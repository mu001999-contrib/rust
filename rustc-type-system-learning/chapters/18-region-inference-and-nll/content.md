---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "18"
document: content
status: completed
updated_at: 2026-09-25
---

# 18. Region Inference 与 NLL

## 学习目标

- 把 NLL region 看成一组“借用必须有效的 MIR 位置”，并区分它与源码里的 `'a` 名字。
- 沿当前 rustc 源码追踪：MIR region 重新编号 → 生成 liveness/outlives 约束 → SCC 传播 → 合法性检查。
- 对 `R1: R2` 正确写出值包含方向，并手算一个小型约束图。
- 解释 universal region（函数签名的生命周期）与 existential region（函数体内待求解的生命周期）的区别。
- 说明为何“算出一个足够大的 region 集合”仍不等于类型检查成功。

## 前置知识

第 04 章的 region 种类、第 05 章的推理变量，以及第 06 章的 `&'a T <: &'b T` 需要 `'a: 'b`。本章只使用这些基础，不要求先完成第 17 章的高阶量词专项。

## 核心心智模型

先看最熟悉的例子：

```rust
let mut n = 10;
let r = &n;
println!("{r}"); // r 最后一次使用
n = 20;          // 此时可再次修改 n
```

借用 `&n` 的 region 不必延续到词法块结尾，但必须覆盖 `r` 被使用所需的 MIR 位置。这是 NLL（non-lexical lifetimes）最直观的一面；真正的结束位置由 liveness、类型关系和其他约束共同决定，不能总是机械地取源码“最后一次出现”。

把 region `R` 的解想成集合 `Values(R)`。当前实现中，元素主要有三类：MIR `Location`、代表根 universe 中 universal region 的标记、以及 placeholder 标记。给 `R` 一条“在位置 `p` 活跃”的约束，就必须令 `p ∈ Values(R)`。给出 `R1: R2`，则必须令 `Values(R1) ⊇ Values(R2)`。因此约束边的方向是“长的/上界 `R1` 吸收短的/下界 `R2` 的元素”。

```text
MIR 中的 region 变量
    ├─ liveness：先给各变量放入必需的位置
    └─ outlives：R1: R2，要求 Values(R1) 包含 Values(R2)
                ↓
        合并成 SCC，按 DAG 顺序传播
                ↓
      检查 type tests 和 universal region 边界
```

## 源码地图

| 仓库路径 | 符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_borrowck/src/nll.rs` | `replace_regions_in_mir`、`compute_regions` | MIR region 重新编号与 NLL 主流程 |
| `compiler/rustc_borrowck/src/universal_regions.rs` | `UniversalRegions` | 记录 `'static`、当前函数体和签名中的 universal regions |
| `compiler/rustc_borrowck/src/renumber.rs` | `RegionRenumberer::renumber_regions` | 为 MIR 内的 region occurrence 建立 NLL 推理变量 |
| `compiler/rustc_borrowck/src/type_check/liveness/mod.rs` | `generate` | 从变量使用、drop 等生成 liveness |
| `compiler/rustc_borrowck/src/constraints/mod.rs` | `OutlivesConstraint` | `sup: sub` 及来源位置 |
| `compiler/rustc_borrowck/src/region_infer/values.rs` | `RegionElement`、`LivenessValues`、`RegionValues::add_region` | region 值的元素与并集操作 |
| `compiler/rustc_borrowck/src/region_infer/region_context.rs` | `UnsolvedRegionInferenceContext::new/solve`、`propagate_constraints`、`check_universal_region_relation` | 初始化、SCC 传播和边界检查 |
| `compiler/rustc_borrowck/src/type_check/free_region_relations.rs` | `UniversalRegionRelations` | 记录签名/where-clause 已知的 universal outlives 关系 |

补充阅读：[rustc dev guide：Region inference](https://rustc-dev-guide.rust-lang.org/borrow-check/region-inference.html)、[Constraint propagation](https://rustc-dev-guide.rust-lang.org/borrow-check/region-inference/constraint-propagation.html)、[Lifetime parameters](https://rustc-dev-guide.rust-lang.org/borrow-check/region-inference/lifetime-parameters.html)。部分指南仍以 `end('a)` 记 universal 标记；下文对当前 IR 名称以本仓库源码为准。

## 源码精读

### 1. 入口：保留签名边界，重新编号 MIR 内 region

来自 `compiler/rustc_borrowck/src/nll.rs` 的 `replace_regions_in_mir`（省略日志、dump）：

```rust
let universal_regions = UniversalRegions::new(infcx, def);
renumber::renumber_mir(infcx, body, promoted);
universal_regions
```

第一行建立函数签名中 universal region 的信息；第二行把 MIR 中还需求解的 region occurrence 换成 NLL 变量。这解释了为什么源码中的 `'a` 和之后看到的 `RegionVid` 不是两个彼此无关的概念：后者是 MIR 求解使用的编号，`UniversalRegions` 保留从签名边界到这些编号的联系。`renumber.rs` 中的 `renumber_regions` 通过 `fold_regions` 与 `next_nll_region_var` 创建变量。

### 2. 约束的方向写在字段名里

来自 `compiler/rustc_borrowck/src/constraints/mod.rs` 的 `OutlivesConstraint`（省略诊断字段）：

```rust
pub struct OutlivesConstraint<'tcx> {
    /// The region SUP must outlive SUB...
    pub sup: RegionVid,
    /// Region that must be outlived.
    pub sub: RegionVid,
    pub locations: Locations,
    // span、category、variance_info、from_closure 省略
}
```

`sup: sub` 就是 `Values(sup) ⊇ Values(sub)`。`locations` 记录约束来自某个 `Single(Location)`，还是必须全局成立的 `All(Span)`；它还服务于诊断与约束来源。不要仅凭 `Single` 就把本章的 `RegionValues::add_region` 理解成“只并入一个位置”——这个函数并入的是另一个 region 的整组值。

### 3. liveness 先放入必须覆盖的位置

来自 `compiler/rustc_borrowck/src/type_check/liveness/mod.rs` 的 `generate`（只展示 universal 初始化；其余局部变量分析和 drop 分支省略）：

```rust
// Universal regions are live at every point.
for region in typeck.universal_regions.universal_regions_iter() {
    typeck.constraints.liveness_constraints.add_all_points(region);
}
```

函数签名里的 universal region 属于调用方环境，所以在当前函数体的所有点都活跃；普通函数体内 region 则由值的实际使用、drop 等逐步约束。`LivenessValues` 用稀疏位集保存这些点。注意 `liveness` 是 region 必须覆盖的**下界**，不是一个已经完成并验证的最终解。

### 4. region 解是三类元素的集合

来自 `compiler/rustc_borrowck/src/region_infer/values.rs` 的 `RegionElement`：

```rust
pub(crate) enum RegionElement<'tcx> {
    Location(Location),
    RootUniversalRegion(RegionVid),
    PlaceholderRegion(ty::PlaceholderRegion<'tcx>),
}
```

MIR 位置回答“函数体内何处必须有效”；`RootUniversalRegion` 是“必须至少达到该 universal region”的符号标记，讲义里写作 `end('a)`；placeholder 标记用于第 17 章那类高阶约束，本章不必继续展开。它们不是“运行时计时器”，而是编译器用来比较 region 范围的抽象元素。

### 5. 初始化、SCC 传播与最后的检查

来自 `compiler/rustc_borrowck/src/region_infer/region_context.rs` 的 `UnsolvedRegionInferenceContext::new`（省略其他来源分支与结构构造）：

```rust
let scc = constraint_sccs.scc(region);
match definition.origin {
    NllRegionVariableOrigin::FreeRegion => {
        scc_values.add_free_region(scc, region);
    }
    NllRegionVariableOrigin::Placeholder(placeholder) => {
        scc_values.add_placeholder(scc, placeholder);
    }
    NllRegionVariableOrigin::Existential { .. } => {}
}
if let Some(liveness) = liveness_constraints.point_liveness(region) {
    scc_values.merge_liveness(scc, liveness)
}
```

即：同一 SCC 的变量共享一个值；先并入自身的 universal/placeholder 标记与已有 liveness。然后 `propagate_constraints` 对 SCC DAG 逐边执行（来自同文件；省略日志）：

```rust
for scc_a in self.constraint_sccs.all_sccs() {
    for &scc_b in self.data.constraint_sccs.successors(scc_a) {
        self.data.scc_values.add_region(scc_a, scc_b);
    }
}
```

若有边 `A: B`，先算 B，再把 B 的值并入 A。`RegionValues::add_region` 同时合并 points、free-region 标记和 placeholders。`UnsolvedRegionInferenceContext::solve` 随后先 `propagate_constraints`，再 `check_type_tests`，通常再 `check_universal_regions`（legacy Polonius 模式有对应的 subset-error 分支）。因此“传播完成”与“所有约束都合法”是分开的阶段。

## 正文

### 一、同叫 region variable，角色可以不同

在 `fn f<'a>(x: &'a u32)` 中，`'a` 由调用方选择；检查 `f` 的函数体时，不能擅自把它缩到只覆盖某几条语句。`UniversalRegions` 为这类签名生命周期（以及 `'static`、函数体特殊 region）建立边界；当前 `UniversalRegions` 源码有 `fr_static` 与 `fr_fn_body` 字段。它们在 NLL 内部也用 `RegionVid` 表示，但语义上是必须尊重的 universal 边界。

相对地，`let r = &n` 新借用需要的范围由函数体内事实约束，是 existential region。它初始可很小，再根据 liveness 与 outlives 增长。源代码里不一定有对应的显式生命周期名字。不要把 “NLL 推理变量”直接等同于“用户写的生命周期参数”。

### 二、从“使用”到 liveness

在开头的 `r` 例子里，打印需要 `r` 中引用保持有效，因此相应 region 必须包含这段使用涉及的 MIR 位置。`liveness::generate` 不只看字符串层面的最后一次出现；它结合 MIR 上的 use、drop 和类型里的 region，生成 `LivenessValues`。它也可能为某些 rvalue、调用等记录 region 的活跃位置。这个集合是最小要求的起点。

例如用示意位置表示：

```text
p0: 创建 &n
p1: 使用 r
p2: 修改 n
```

对借用 region `R`，若 liveness 和其他约束最后只要求覆盖 `p0、p1`，那么 `p2` 就不必在 `Values(R)` 内，修改可以发生。这是示意，不是对实际 MIR `Location` 编号、每条语句位置数量或最终值的逐项断言。

### 三、从类型关系到 outlives

共享引用的子类型规则给出：

```text
&'long T <: &'short T   需要   'long: 'short
```

MIR type checker 在赋值、参数传递、返回、显式类型注解等场景检查类型关系时，会把这类要求加入 `OutlivesConstraintSet`。如果得到 `Rlong: Rshort`，图上是 `Rlong → Rshort`，传播时将 `Values(Rshort)` 并入 `Values(Rlong)`。于是 `Rlong` 必须覆盖 `Rshort` 已有的一切点和标记。

一个纸面计算：

```text
初值：Values(R0) = {p0}
      Values(R1) = {p1}
      Values(R2) = {p2}
约束：R0: R1, R1: R2
结果：Values(R2) = {p2}
      Values(R1) = {p1, p2}
      Values(R0) = {p0, p1, p2}
```

实现通过 SCC DAG 的依赖顺序一次传播，不是照这段文字反复扫边。如果再加 `R1: R0`，R0 与 R1 形成 SCC、共享同一个结果。

### 四、为何 universal region 还要单独检查

```rust
fn take<'a, 'b>(x: &'a u32) -> &'b u32 {
    x
}
```

返回值要求 `&'a u32 <: &'b u32`，所以约束是 `'a: 'b`。传播会使 `Values('a)` 含有 `'b` 的 universal 标记（可记作 `end('b)`）。集合运算本身总能把它加进去；但函数签名并未承诺所有调用方选出的 `'a` 都比 `'b` 长，所以不能因此宣布成功。

`check_universal_region` 遍历最终值中的 universal 标记；`check_universal_region_relation` 再询问 `universal_region_relations.outlives(longer_fr, shorter_fr)`。后者的已知关系来自签名、where-clause、implied bounds 等，而不是本次传播“想要什么就创造什么”。如果写成 `fn take<'a: 'b, 'b>(...)`（或等价的 `where 'a: 'b`），该关系成为调用方须满足的契约；在函数体里可以据此返回。

同理，`check_type_tests` 检查待满足的 `T: 'r` 类条件。它与纯 region 间的 `R1: R2` 不是同一类约束。第 19 章会深入看 MIR type checking 如何产生这些检查。

### 五、SCC 是求解加速，不是额外语义

有 `R0: R1` 且 `R1: R0`，两组值互相包含，故相等。求解器把这种循环压缩成强连通分量（SCC），为每个 SCC 存一个 `scc_values`。压缩后的图没有环，`propagate_constraints` 可先算被依赖的后继，再并回前驱。注意 SCC 相等只表示约束要求这些 region 拥有同一个解，不会自动使 universal 边界合法；合法性仍需后续检查。

### 六、这一章与第 17、19 章的接口

第 17 章关心“高阶 binder 打开后 placeholder 能否逃逸”；本章只需知道 placeholder 是 region 值可能含有的一类标记，当前 `check_universal_regions` 也区分 `FreeRegion` 与 `Placeholder`。完整的高阶规则留待专项。

本章解决的是“给定 MIR 类型关系和活跃位置，region 约束如何求解”。第 19 章会回到这些约束从哪条 MIR 语句、哪次类型检查、哪次借用冲突判断中产生，以及求得的 region 值怎样供借用检查使用。因而不要把 NLL 简化为“把生命周期都缩短”，更准确的是求满足约束的最小值，再检查 universal/type 边界，之后借用检查利用结果。

## 常见误区

1. `R1: R2` 不是把 R1 塞进 R2；它使 `Values(R1)` 包含 `Values(R2)`。
2. `RegionVid` 不一定是函数体内 existential；签名的 universal region 也有编号。
3. liveness 不是最终解：outlives 传播可能继续扩大 region。
4. 传播出一个集合不代表程序成立：还要检查 type tests、universal 边界等。
5. SCC 不是“把两个源码生命周期名字永远改成同一个”；它是在本次约束求解中共享值。
6. `Locations::Single` 记录约束来源/生效位置；不要把它误读为 `add_region` 只合并一个 MIR 点。

## 本章小结

NLL 在 MIR 上为 region 求一个由位置、universal 标记和 placeholder 标记组成的解。liveness 给下界，`sup: sub` 令 `sup` 吸收 `sub` 的值；SCC 将互相包含的变量合并，再按 DAG 顺序传播。最后依据已知的 universal 关系和 type tests 判断结果能否成立。抓住“`R1: R2` → `Values(R1) ⊇ Values(R2)` → 传播后检查边界”这条主线，就能继续阅读第 19 章的 MIR borrowck 输入与输出。
