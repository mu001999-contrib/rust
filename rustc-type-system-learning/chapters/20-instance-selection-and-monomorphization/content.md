---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "20"
document: content
status: exercises_assigned
updated_at: 2026-09-25
---

# 20. 实例选择与 Monomorphization

## 学习目标

- 从泛型定义与一次具体调用推导 `Instance { def, args }`，并区分“选中哪个实现”与“代入哪些实参”。
- 追踪 trait 方法从 trait item 的 `(DefId, args)` 到 impl 方法、默认方法或虚调用的解析。
- 解释 mono item 收集如何从根节点扫描 MIR，为什么函数值、drop glue 和 vtable 方法也能形成依赖边。
- 说明当前 rustc 在代码生成时按需实例化 MIR 中的类型，而不是预先复制一份完整的单态化 MIR。
- 辨认课程规划中的 `polymorphization` 是历史术语；当前源码没有对应的活动开关或收集 pass。

## 前置知识

第 03 章的 `GenericArgs` 与 `rebase_onto`，第 07–12 章的 trait candidate/impl selection，第 15 章的 dyn/vtable，以及第 19 章“检查通过后的 MIR”是本章的连接点。第 17 章仍留作高阶约束专项，不影响本章。

## 核心心智模型

泛型 MIR 通常仍以定义处参数表达。具体调用先确定实参，再把“应运行哪段代码”解析为 `Instance`；收集阶段据此形成要生成代码的 `MonoItem` 图；代码生成读取原 MIR，按这个 `Instance` 在使用处实例化。

```text
泛型定义 fn id<T>(T) -> T     调用 id::<u32>(7)
             │                         │
             └──── 已定型调用 + 具体 args ──┘
                              ↓
                 Instance { def: Item(id), args: [u32] }
                              ↓
      roots → MIR 依赖边 → MonoItem::Fn(instance) 集合
                              ↓
                    分配 codegen units
                              ↓
         取 generic MIR，按 instance.args 即时实例化/归一化
```

三个身份不要混淆：

| 身份 | 例子 | 回答的问题 |
|---|---|---|
| `DefId` | `id` 的定义 ID | 源码定义是哪一个？ |
| `GenericArgs` | `[u32]` | 该定义中的参数这次变成什么？ |
| `Instance` | `Item(id) + [u32]` | 当前应执行/生成哪一种具体调用实体？ |

同一个定义可以对应多个 `Instance`；trait item 的 `DefId` 还可能先解析到某个 impl item，或解析成 `Virtual` / compiler shim。`Instance` 是中间层的“可调用实体”，不能简单等同于“一定单独生成一个最终机器码符号”：是否本地 codegen、是否内联、CGU 分配、跨 crate 复用等还会影响实际产物。

## 源码地图

| 仓库相对路径 | 关键符号 | 用途 |
|---|---|---|
| `compiler/rustc_middle/src/ty/instance.rs` | `Instance`、`InstanceKind`、`try_resolve`、`instantiate_mir_and_normalize_erasing_regions` | 实例身份与按需代入 |
| `compiler/rustc_ty_utils/src/instance.rs` | `resolve_instance_raw`、`resolve_associated_item` | 从 trait item 找到 impl/default/virtual 实体 |
| `compiler/rustc_middle/src/mono.rs` | `MonoItem`、`InstantiationMode` | 要收集的代码实体及放置模式 |
| `compiler/rustc_monomorphize/src/collector.rs` | `collect_roots`、`collect_crate_mono_items`、`MirUsedCollector`、`visit_fn_use` | 根节点、MIR 扫描、依赖图 |
| `compiler/rustc_monomorphize/src/partitioning.rs` | `collect_and_partition_mono_items` | 收集后分配 codegen units |
| `compiler/rustc_codegen_ssa/src/mir/mod.rs` | `FunctionCx::monomorphize`、`codegen_mir` | 后端翻译 MIR 时按实例代入 |
| `RELEASES.md` | 1.85.0 Compiler 项 | `-Zpolymorphize` 移除记录 |

辅助阅读：[rustc dev guide：编译器总览](https://rustc-dev-guide.rust-lang.org/overview.html)、[Lowering MIR to a Codegen IR](https://rustc-dev-guide.rust-lang.org/backend/lowering-mir.html)、[当前 rustc 的 `Instance` API](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/ty/struct.Instance.html)。本文的具体分支和名称以当前仓库源码为准。

## 源码精读

### 1. `Instance` 是“代码种类 + 实参”

`compiler/rustc_middle/src/ty/instance.rs` 的定义摘录（`ShimKind` 内部种类省略）：

```rust
pub struct Instance<'tcx> {
    pub def: InstanceKind<'tcx>,
    pub args: GenericArgsRef<'tcx>,
}

pub enum InstanceKind<'tcx> {
    Item(DefId),
    Intrinsic(DefId),
    LlvmIntrinsic(DefId),
    Virtual(DefId, usize),
    Shim(ShimKind<'tcx>),
}
```

`Item` 通常对应可执行的用户定义函数、闭包或 coroutine；`Virtual` 表示调用经 vtable 槽位间接分派；`Shim` 是编译器生成的适配代码，例如部分 drop glue 或函数指针调用 shim。`def` 不总是简单 `DefId`，这正是 `InstanceKind` 存在的原因。`args` 包含此实例要用的完整泛型实参，可能含类型与 const 实参；在 `try_resolve` 查询边界 region 会被 erase/anonymize，不能把具体源码 lifetime 当作生成不同运行时代码的区分符。

### 2. 先选 impl，再转换参数坐标

`compiler/rustc_middle/src/ty/instance.rs::Instance::try_resolve` 将 `(def_id, args)` 交给 `tcx.resolve_instance_raw`。`compiler/rustc_ty_utils/src/instance.rs::resolve_associated_item` 的主干摘录（错误、specialization 和 builtin 分支已省略）：

```rust
let trait_ref = ty::TraitRef::from_assoc(tcx, trait_id, rcvr_args);
let input = typing_env.as_query_input(trait_ref);
let vtbl = match tcx.codegen_select_candidate(input) {
    // 具体分支省略
};

// UserDefined impl 分支中的参数转换：
let args = rcvr_args.rebase_onto(tcx, trait_def_id, impl_data.args);
let args = translate_args(
    &infcx, param_env, impl_data.impl_def_id, args, leaf_def.defining_node,
);
// region 擦除、兼容性检查等省略
Some(ty::Instance::new_raw(leaf_def.item.def_id, args))
```

`codegen_select_candidate` 决定哪种 impl 来源；`leaf_def` 决定实际关联方法定义（包括可能使用 trait 默认方法的情形）。`rebase_onto` 把 trait item 的参数后缀接到 impl 参数前缀，`translate_args` 处理 specialization 祖先链上的参数转换。因而调用侧的 trait item `DefId` 与最后 `Instance` 的 `def_id()` 可以不同。对于 `dyn Trait`，相应 builtin object 分支可能给出 `InstanceKind::Virtual`，并非选出唯一具体类型的 impl 方法。

### 3. 根节点与 MIR 依赖边形成 `MonoItem` 图

`compiler/rustc_middle/src/mono.rs`：

```rust
pub enum MonoItem<'tcx> {
    Fn(Instance<'tcx>),
    Static(DefId),
    GlobalAsm(ItemId),
}
```

`compiler/rustc_monomorphize/src/collector.rs::collect_crate_mono_items` 的步骤是 `collect_roots`，再对根节点运行 `collect_items_root` / `collect_items_rec`。后者借 `items_of_instance` 扫描该实例的 MIR，记录它使用的其他 `MonoItem`，然后递归遍历；`visited` 防止同一 mono item 无限重复收集。根节点既有适合直接输出的非泛型入口，也受到可见性、收集策略、静态对象等条件影响，不等于“所有源码函数”。

在 `MirUsedCollector::visit_terminator` 中，调用先取 `callee_ty`，经 `self.monomorphize(callee_ty)` 把当前实例的实参带入，再交给 `visit_fn_use`。后者对 `FnDef(def_id, args)` 调用 `Instance::expect_resolve`，最后由 `visit_instance_use` 判断是否需要本地代码实体。取函数地址（即使并未立即调用）、drop glue、构造 trait-object vtable 等也能引入依赖；`Virtual` 调用本身通过 vtable，不会被误当成某个直接调用的具体 impl item。

### 4. 不预先保存一套“单态化 MIR”

`compiler/rustc_codegen_ssa/src/mir/mod.rs::FunctionCx::monomorphize` 的主干：

```rust
self.instance.instantiate_mir_and_normalize_erasing_regions(
    self.cx.tcx(),
    self.cx.typing_env(),
    ty::EarlyBinder::bind(self.cx.tcx(), value),
)
```

同文件 `codegen_mir` 先取 `tcx.instance_mir(instance.def)`；翻译具体 MIR 值时，用这个 `Instance` 的实参做 instantiate、normalize 和 erase regions。`Instance::args_for_mir_body` 还区分泛型 MIR 与已按实例生成的 shim MIR：只有前者需要再用 `self.args` 替换定义处参数。某些特性分支可能临时克隆已实例化 MIR 做额外优化，但一般模型是“按需实例化”，不是为每个 `Instance` 预先持久生成一份完整 MIR。

## 正文

### 一、从一个泛型调用得到实例

```rust
fn id<T>(x: T) -> T { x }

fn main() {
    let _: u32 = id::<u32>(7);
    let _: bool = id::<bool>(true);
}
```

`id` 只有一个定义 `DefId`，却有两组实参 `[u32]` 与 `[bool]`。对普通自由函数，解析后的概念实例分别是 `Item(id) + [u32]`、`Item(id) + [bool]`。收集器从非泛型 `main` 这一根出发；如果优化后的 MIR 仍保留这两次函数使用，就会形成对应的 `MonoItem::Fn` 节点。若 MIR 已内联或消除了调用，实际收集结果可不同，更不能断言最终二进制保留两个独立符号。泛型 `id` 的定义本身也不因为“写在源码里”就自动拥有全部可能 `T` 的代码。

### 二、trait 方法的 `args` 要换坐标

```rust
trait Show {
    fn show<U>(&self, value: U);
}
struct Wrap<T>(T);
impl<T> Show for Wrap<T> {
    fn show<U>(&self, _: U) {}
}
fn main() {
    Wrap(3_u32).show(true);
}
```

把调用视作 trait item 时，参数坐标可写成 `[Self = Wrap<u32>, U = bool]`；选中 `impl<T> Show for Wrap<T>` 后，具体 impl 方法的坐标是 `[T = u32, U = bool]`。`Self = Wrap<u32>` 不是原封不动塞进 impl 方法的 `T` 位置。`resolve_associated_item` 的 `rebase_onto` / `translate_args` 就在完成这类坐标转换；与第 03 章的参数替换模型直接相连。

这也区别于第 12 章的普通 trait goal 求证。源程序 typeck 需要证明“此调用合法吗”；到了选择可执行 `Instance` 时，还要知道“具体应运行哪段方法体”。`try_resolve` 在仍带参数的上下文可能返回 `Ok(None)`（尚不能唯一确定代码），而已通过检查的单态化 codegen 上下文通常应得到具体实例；`Err` 则表示先前错误阻止了解析，不等于普通歧义。

### 三、收集、放置、翻译是三个动作

`collect_crate_mono_items` 建图并去重；`collect_and_partition_mono_items` 先调用收集器，再将节点分到 codegen units。`MonoItem::Fn(Instance)` 是节点身份，而 `InstantiationMode` 描述节点在 CGU 中是全局共享还是允许局部副本；“一个 `MonoItem`”不等于“整个最终程序严格一个函数副本”。后端随后通过 `codegen_instance` / `codegen_mir` 翻译 MIR。这三步不可缩成“trait solver 一选出 impl，机器码就已经生成了”。

跨 crate 的泛型定义也可能在使用方 crate 被实例化；若上游已有可链接的共享版本，`should_codegen_locally` / `upstream_monomorphization` 等逻辑可能选择复用。常量、vtable 等有按需局部生成或求值路径，不能一概当作 `MonoItem::Fn`。收集器还跟踪 `mentioned_items`：即使优化移除某个代码使用，仍需在恰当场景检查原先提及的常量是否求值出错；这和实际需要生成代码的 `used_items` 不完全相同。

### 四、`polymorphization` 在当前源码中的位置

课程总览沿用了旧资料中的 `polymorphization` 术语：历史上它试图识别对代码生成无影响的泛型实参，从而减少等价实例。当前仓库的 `RELEASES.md` 明确记载 Rust 1.85.0 移除了实验性 `-Zpolymorphize`；在当前 `rustc_monomorphize` 和 `rustc_middle/src/ty/instance.rs` 中没有应被当作本章主线的活动 polymorphization pass。因此本章只要求理解这个历史目标，以及它**不能替代**当下 `Instance`、参数实例化、mono item 收集与后端优化的实际流程。也不要把“region 在代码生成前被擦除”直接称作该旧 pass。

## 常见误区

1. “`Instance` 就是 `DefId`”：还要有 `InstanceKind` 与完整 `args`。
2. “trait item 的 `Self` 就是 impl item 的第一个参数”：需要 impl 选择和参数坐标转换；默认方法、specialization 还可能增加路径。
3. “单态化先复制整份 MIR，收集器再读复制件”：一般是用 `Instance` 按需实例化 generic MIR 中的值。
4. “`MonoItem` 只来自显式调用”：函数值、drop glue、vtable、静态数据等也会引入使用关系。
5. “一个 mono item 必有一个最终机器码函数”：收集、CGU 放置、内联、跨 crate 共享和链接是不同层。
6. “`-Zpolymorphize` 仍是当前编译器的必经步骤”：当前版本已移除该实验开关。

## 本章小结

从泛型调用走到 codegen，应依次问：`def_id + args` 是什么、`InstanceKind` 解析成什么、该实例是否进入本地 `MonoItem` 图、被放入哪个 codegen unit、翻译 MIR 时如何使用 `instance.args`。这条链把前面学过的 `GenericArgs`、trait selection、dyn dispatch 和 MIR 串到实际生成代码的入口。
