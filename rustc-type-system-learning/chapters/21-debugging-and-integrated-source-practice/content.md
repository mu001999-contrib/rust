---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "21"
document: content
status: completed
updated_at: 2026-09-25
---

# 21. 调试方法与综合源码实践

## 学习目标

- 从一个可复现的类型系统现象，写出具体的待验证假设，而非只搜索错误文本。
- 按问题所在阶段选择编译诊断、`RUSTC_LOG`、MIR dump、局部测试与源码断点。
- 区分 trait goal 的证明、关联类型归一化、MIR 检查、mono item 收集与代码生成各自的输入输出。
- 能沿着当前 rustc 的函数入口追踪一条具体案例，并说明证据能证明什么、仍缺什么。

## 前置知识

第 07–13 章的 `ParamEnv`、goal、候选与 projection，第 18–19 章的 MIR/NLL，第 20 章的 `Instance` 与 mono item 图。本章把这些概念变成可执行的定位流程；第 17 章仍保留为后续专项。

## 核心心智模型

调试 rustc 时，把“类型是否成立”和“如何生成代码”分开观察：

```text
现象与最小复现
    ↓
定型/trait goal/normalization 的判断
    ↓ 通过后
MIR 构建与类型检查、borrowck、优化
    ↓
Instance 解析 → mono item 收集与 CGU 分配 → codegen
```

每一步都问四件事：**输入是什么、谁作判断、产物是什么、该产物在哪里被消费**。编译错误一般意味着流程在某一步没有通过；若前端定型失败，不能用缺少后端 mono item 来解释它。日志、dump 和测试是不同观察窗：日志记录源码中的事件，MIR dump 给出某时刻的 IR，测试固定可观察行为；它们不相互替代。

## 源码地图

| 仓库相对路径 | 关键符号 | 观察意义 |
|---|---|---|
| `compiler/rustc_log/src/lib.rs` | `LoggerConfig::from_env`、`init_logger_with_additional_layer` | `RUSTC_LOG` 的过滤器与输出层 |
| `compiler/rustc_middle/src/mir/pretty.rs` | `MirDumper::new`、`dump_mir`、`dump_path` | `-Zdump-mir` 过滤与文件写入 |
| `compiler/rustc_mir_transform/src/pass_manager.rs` | `run_passes`、`dump_mir_for_phase_change` | MIR pass 前后生成 dump 的入口 |
| `compiler/rustc_next_trait_solver/src/solve/assembly/mod.rs` | `EvalCtxt::assemble_and_merge_candidates` | trait/projection 候选与响应合并的入口之一 |
| `compiler/rustc_monomorphize/src/collector.rs` | `MirUsedCollector`、`collect_crate_mono_items` | MIR 中的使用关系如何成为 mono item 依赖 |
| `compiler/rustc_codegen_ssa/src/mir/mod.rs` | `codegen_mir` | 按 `Instance` 翻译 MIR |
| `src/tools/compiletest/src/runtest.rs` | `compare_output` | UI 测试怎样比较实际与预期输出 |
| `src/doc/rustc-dev-guide/src/tracing.md`、`mir/debugging.md`、`tests/running.md` | 调试文档 | 当前仓库中命令与限制的说明 |

路径和符号名用于导航；源码行号随版本变化。`src/tools/compiletest` 这里只阅读其实现，不在该外部维护工具目录中改动。

## 源码精读

### 1. 日志先建立过滤器，再接收事件

`compiler/rustc_log/src/lib.rs::init_logger_with_additional_layer` 的主干摘录（颜色、文件输出与 layer 构造省略）：

```rust
let filter = match cfg.filter {
    Ok(env) => EnvFilter::new(env),
    _ => EnvFilter::default().add_directive(Directive::from(LevelFilter::WARN)),
};
```

`LoggerConfig::from_env("RUSTC_LOG")` 读取环境变量。没提供过滤器时默认只看 warning 级别；写 `RUSTC_LOG=rustc_next_trait_solver=debug` 才请求对应 target 的 debug 事件。还必须确认被观察的求解器确实在运行：当前 `NextSolverConfig` 的一般默认值是仅在 coherence 启用 next solver，部分构建可通过编译配置改变默认值；追踪普通类型检查中的 next solver 时可显式使用 `-Znext-solver=globally`。另一个独立条件是编译器构建配置：`debug!` / `trace!` 在默认 `rust.debug-logging=false` 的 bootstrap 构建中可能未编入。看到空日志时，分别检查执行路径、求解器模式、target/level 和构建配置。

### 2. MIR dump 的过滤发生在写文件之前

`compiler/rustc_middle/src/mir/pretty.rs::MirDumper::new` 的主干摘录（`node_path` 构造与 dumper 初始化省略）：

```rust
let dump_enabled = if let Some(ref filters) = tcx.sess.opts.unstable_opts.dump_mir {
    filters.split('|').any(|or_filter| {
        or_filter.split('&').all(|and_filter| {
            let term = and_filter.trim();
            term == "all" || pass_name.contains(term) || node_path.contains(term)
        })
    })
} else {
    false
};
```

因此 `main & SimplifyCfg` 要求同一候选 dump 的函数路径或 pass 名满足两项；`|` 表示任一组满足。`compiler/rustc_mir_transform/src/pass_manager.rs::run_passes` 在 pass 执行前/后分别调用 `dumper.dump_mir(body)`，`MirDumper::dump_path` 使用 `-Zdump-mir-dir` 指定目录。dump 文件名包含函数、pass、`before`/`after`，应先比较同一 pass 两侧，而不是混看不同阶段的 MIR。NLL 还有其专门的 MIR 注释/dump 路径。

### 3. UI 测试比较的是可观察输出

`src/tools/compiletest/src/runtest.rs::compare_output` 的主干摘录（规范化、compare mode 与差异展示省略）：

```rust
let expected_path = expected_output_path(/* 测试路径、revision、compare mode、stream */);
let are_different = expected != actual; // 实际源码还有 SVG/逐行比较分支
if !are_different {
    return CompareOutcome::Same;
}
// 不同则记录 actual；只有 bless 模式才更新 expected。
```

这里的源码片段是**控制流示意**，不是可逐字编译的完整实现。`tests/ui` 的 `.stderr` / `.stdout` 是期望结果；一般定位时先运行单个测试，确认观察到的行为，再决定需要哪个测试类别。`tests/mir-opt` 检查 MIR 变换，`tests/codegen-*` 检查后端产物。不能把“某次日志里出现某事件”直接当成稳定的 UI 测试断言。

### 4. 后端观察必须接上实例收集

`compiler/rustc_monomorphize/src/collector.rs::MirUsedCollector::visit_terminator` 的关键链（特殊调用分支省略）：

```rust
let callee_ty = func.ty(self.body, tcx);
let callee_ty = self.monomorphize(callee_ty);
visit_fn_use(self.tcx, callee_ty, /* direct call? */, source, &mut self.used_items);
```

这说明“源码写了调用”与“实际收集到哪个函数实例”之间还有当前 `Instance` 的参数实例化及 `visit_fn_use` 的解析。构造 trait object vtable 的 `Unsize` 分支、取函数地址的 `ReifyFnPointer` 分支也会形成依赖。一个成功通过类型检查的案例，才有继续追问 mono item 与 codegen 的基础。

## 正文

### 一、从一个综合案例出发

```rust
trait Render {
    type Out;
    fn render(self) -> Self::Out;
}

struct Wrap<T>(T);

impl<T: Clone> Render for Wrap<T> {
    type Out = Vec<T>;
    fn render(self) -> Self::Out { vec![self.0] }
}

fn consume<R: Render<Out = Vec<String>>>(x: R) -> Vec<String> {
    x.render()
}

fn main() {
    let _ = consume(Wrap(String::from("ok")));
}
```

先做纸面追踪：调用处 `R = Wrap<String>`；目标要求 `Wrap<String>: Render`，并要求 `<Wrap<String> as Render>::Out = Vec<String>`。候选 `impl<T: Clone> Render for Wrap<T>` 将 `T` 匹配为 `String`，产生/证明 `String: Clone`，关联类型归一化得到 `Vec<String>`。`consume` 的定义处环境则是抽象前提 `R: Render<Out = Vec<String>>`，不能把调用处的 `String` 偷渡进泛型函数的定义检查。

如果将调用改成 `consume(Wrap(7_u32))`，`Wrap<u32>: Render` 与 `u32: Clone` 仍可成立；变化的是关联类型得到 `Vec<u32>`，无法满足要求的 `Vec<String>`。定位时先拆开 trait 可实现性与 projection equality；不要把所有带 `Render` 的错误都归因为“找不到 impl”。

### 二、最小复现与假设

最小复现应保留能触发现象的最短关系链：trait 声明、impl 条件、关联类型、调用处期望。每删去一个部分，重新检查现象是否还在；只有现象仍在，删除才是有效缩减。记录三个事实：期望结果、实际结果、改动一个条件后的差异。例如上例唯一改动 `String → u32`，便把怀疑范围收窄到 `Out` 归一化/相等性，而非是否存在 `Render` impl。

诊断假设要可证伪，例如：“`T=u32` 的 impl candidate 存在，但其 projection response 给出 `Vec<u32>`，与期望 `Vec<String>` 不等。”随后选择观察入口验证，而不是先加全量日志。

### 三、选择正确的观察窗

| 要问的问题 | 首选观察窗 | 证据边界 |
|---|---|---|
| 为什么某个 trait/projection goal 成立或失败？ | 最小复现、确认 solver 模式后定向 `RUSTC_LOG`、候选组装/归一化源码 | 日志展示求解路径，不代表最终代码实体 |
| MIR 某个 pass 改了什么？ | `-Zdump-mir` 的同一 pass `before`/`after` | dump 是特定阶段快照，不直接说明 trait candidate |
| NLL 的 region/loan 何时还活跃？ | NLL 注释 dump、borrowck 源码与相关约束 | 源码 lifetime 名字不等于最终 MIR region 解 |
| 一个实例有没有被本地收集、在哪个 CGU？ | `collector.rs`、partitioning 源码及适当 codegen 观察 | 概念上的 `Instance` 不保证最终二进制符号 |
| 怎样固定一个编译器行为？ | 最小化的 `tests/ui`、`mir-opt`、`codegen-*` 测试 | 应选能直接观察目标行为的套件 |

命令示意（需要已构建并注册 `+stage1` 的本地工具链；使用当前仓库对应的 stage1 rustc，`-Z` 选项要求 nightly/dev 编译器）：

```bash
RUSTC_LOG=rustc_next_trait_solver=debug rustc +stage1 -Znext-solver=globally example.rs
rustc +stage1 -Zdump-mir='consume & SimplifyCfg' -Zdump-mir-dir=/tmp/rustc-study-mir example.rs
./x test tests/ui/traits/next-solver/example.rs
```

上面的 `example.rs` 是自行准备的个人最小复现，最后一行的测试路径也只是命令形状示例，不表示仓库中存在该文件。真实运行时应换成已经存在的精确测试路径。`-Znext-solver=globally` 明确切换求解器，适合研究 next solver 自身；如果原始现象只在默认旧求解器路径出现，应保留原始编译模式，改查 `rustc_trait_selection` 等对应目标，不要让调试选项改变待复现现象。为避免无关输出，可先使用 `rg -n '目标符号' compiler/` 找到函数/模块，再把日志 target 缩到相应模块。若默认构建看不到 debug/trace 事件，可依据 `src/doc/rustc-dev-guide/src/tracing.md` 检查 bootstrap 的 `rust.debug-logging`；不要把空日志当成目标代码没有执行的证据。

### 四、把观察结果串成一条可复查的源码链

对成功案例，记录为：

1. `consume` 定义检查使用 `ParamEnv` 中的 `R: Render<Out = Vec<String>>`；调用处实例化 `R=Wrap<String>`。
2. `Wrap<String>: Render` 选择 `impl<T: Clone>`，`T=String`；确认 `String: Clone`。
3. `<Wrap<String> as Render>::Out` 归一化为 `Vec<String>`；类型关系检查通过。
4. MIR 中调用 `consume`；收集器在已实例化的 `callee_ty` 上解析 `Instance`，再遍历相关函数使用关系。
5. codegen 对被收集的具体实例读取 MIR 并按 `instance.args` 代入。实际内联和最终符号要另行观察，不能从第 4 步直接推断。

对失败案例，只写到确实到达的阶段。`Wrap<u32>` 的 `Out` 不满足 `Vec<String>` 时，先报告 projection/类型关系的证据；不虚构它之后的 mono item 或 codegen 结果。

### 五、测试与复现的选择

`./x test tests/ui/.../具体文件.rs` 是聚焦某个现有 UI 测试的形式；UI 测试主要固定成功/失败与诊断输出。对 MIR pass 形状用 `mir-opt`；对生成的 IR/汇编用 `codegen-*` 或相关套件。运行测试时先看原有期望与实际差异；`--bless` 会更新期望文件，不是观察失败的第一步。课程示例是个人学习材料，不添加到 rust-lang/rust 的正式测试树。

### 六、完成一次源码定位的验收标准

一份合格的定位笔记至少能说清：最小复现、具体 goal/类型关系、最终分歧发生的阶段、负责判断的源码函数、观察证据，以及尚未验证的推断。若问题跨越多个阶段，就按每个阶段的输入/产物逐段建立证据链，不把“存在某个 candidate”“生成了某个 `Instance`”“最终有某个符号”混成同一句话。

## 常见误区

1. “`RUSTC_LOG` 没输出，所以没进 solver”：也可能是 target/level 不匹配，或构建未包含 debug/trace 事件。
2. “MIR dump 中没看到源码表达式，所以类型检查没处理它”：dump 可能位于优化之后，也可能筛选到另一个 pass。
3. “有 trait impl candidate 就证明了 projection equality”：候选的嵌套条件和关联类型值还须检查。
4. “UI 测试失败就直接 `--bless`”：先确认是预期变化还是回归，及差异所处阶段。
5. “收集到 `MonoItem` 就一定有最终独立符号”：CGU、内联、跨 crate 复用与链接仍会影响产物。

## 本章小结

最有效的 rustc 调试方法不是一次打开所有日志，而是先把现象翻译成一个具体 goal、类型关系或 MIR 变换问题，再选正好能观察该阶段的工具。用“输入 → 判断函数 → 产物 → 消费者”追踪源码，最终就能把课程中分散的 `ParamEnv`、projection、NLL、`Instance` 与代码生成连成一条可验证的链。
