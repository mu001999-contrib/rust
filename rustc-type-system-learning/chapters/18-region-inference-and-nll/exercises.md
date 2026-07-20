---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "18"
document: exercises
status: completed
exercise_version: 1
updated_at: 2026-09-25
---

# 18. 习题

## 作答说明

E01–E04 每题 2 分，每小问 0.5 分，共 8 分。先只用本章“位置集合 → outlives 并集 → 边界检查”的主线；不需要第 17 章的高阶 binder 规则。题中的 `p0`、`p1` 等是示意 MIR 位置，不要求猜测 rustc 的实际 `Location` 编号。

## 题目

### E01. 从最后一次使用理解 NLL

```rust
let mut n = 10;
let r = &n;
println!("{r}");
n = 20;
```

1. 借用 `&n` 的 region 必须覆盖哪一类 MIR 位置？为什么不能只说“覆盖到词法块结束”？
2. 若把创建、打印、修改三个示意位置记为 `p0`、`p1`、`p2`，在没有其他延长约束的简化模型里，`p2` 是否必须属于这个借用 region 的值？
3. `LivenessValues` 保存的是 region 的最终解、必需位置的下界，还是 universal 关系表？
4. 为什么“源码里 `r` 最后出现的位置”只是直觉起点，而不能替代 MIR liveness/type/drop 约束分析？

### E02. Outlives 方向与 universal 边界

```rust
fn take<'a, 'b>(x: &'a u32) -> &'b u32 {
    x
}
```

1. 从返回表达式的引用子类型关系，写出需要的 `'a` / `'b` outlives 方向。
2. 若该关系以 `OutlivesConstraint { sup, sub, .. }` 表示，`sup` 和 `sub` 分别对应哪个 region？传播时谁吸收谁的值？
3. 如果传播把代表 `'b` 的 universal 标记加入 `Values('a)`，为何函数仍可能报错？要从哪里查这条关系是否已知？
4. 为签名加什么前提可让这个返回有据可依？该前提是函数体求解出来的，还是要求调用方满足的契约？

### E03. 手算 SCC 和传播

以下全部是概念化 region 值；无 universal 标记和 type tests：

```text
初值：Values(R0) = {p0}
      Values(R1) = {p1}
      Values(R2) = {p2}
约束：R0: R1
      R1: R0
      R1: R2
```

1. 哪些 region 落入同一个 SCC？`R2` 是否也属于这个 SCC？
2. SCC 压缩后，长的 SCC 到短的 SCC 的边朝哪个方向？用 `R0/R1` 与 `R2` 描述即可。
3. 传播后的 `Values(R0)`、`Values(R1)`、`Values(R2)` 各是什么？
4. 为什么这个例子可以按 SCC DAG 的依赖顺序一次传播，而无需在 `R0` 与 `R1` 之间无限循环？

### E04. 找到源码中的四个阶段

1. `replace_regions_in_mir` 先建立哪种 regions 的信息，然后对 MIR 做什么？写出相应的两个源码 API 名称。
2. 当前 `RegionElement` 有哪三类元素？指南里的 `end('a)` 在本章表示哪类概念？
3. `UnsolvedRegionInferenceContext::new` 如何把 liveness 并入 `scc_values`？`propagate_constraints` 遇到 `A: B` 时调用 `add_region(A, B)` 意味着什么？
4. `solve` 在传播之后还进行哪两类主要检查？为什么这些检查不能直接由“并集传播”代替？

## 学习者答案

### E01

> 1. 覆盖 use、drop mir，因为是 NLL，非词法；2. p2 不属于；3. 必需位置的下界；4. 因为有隐式位置，比如说 drop 就是非源码 r 最后出现的位置。

### E02

> 1. 'a: 'b；2. sup = 'a, sub = 'b，a 吸收 b；3. 因为这是传播出来的，函数签名没有要求 'a: 'b，要从函数签名、where bounds、implicit bounds 查；4. 加 'a: 'b，要求调用方满足的。

### E03

> 1. R0 和 R1，不属于；2. R0/R1 到 R2；3. {p0, p1, p2}, {p0, p1, p2}, {p2}；4. 因为 R0 == R1，是一个 SCC，一次就收敛了。

### E04

> 1. 先建立手写 regions 到 mir regions 的映射，然后 renumber；2. Location，RootUniversalRegion，PlaceholderRegion，end 表示 RootUniversalRegion；3. 通过 merge\_liveness，意味着把 Values(B) 加入 Values(A)；4. check\_type\_tests 和 check\_universal\_regions，分别检查 T: 'r 和类型签名中的 'a: 'b。
