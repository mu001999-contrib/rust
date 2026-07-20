---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "19"
document: exercises
status: completed
exercise_version: 2
updated_at: 2026-09-25
---

# 19. 习题

## 作答说明

E01–E04 每题 2 分，每小问 0.5 分，共 8 分。优先回答“哪个阶段、输入是什么、输出给谁”，不要求记住所有函数参数。第 17 章的高阶量词知识不是本章作答前提。Polonius 与 member constraints 小问只按本章当前源码路径回答，不需推演全部实验特性细节。

## 题目

### E01. 两段代码经过哪几关

```rust
// A
let mut n = 1;
let r = &n;
println!("{r}");
n = 2;

// B（放在另一个函数/作用域中）
let mut n = 1;
let r = &n;
n = 2;
println!("{r}");
```

1. A 与 B 的 `n`、`r` 基本类型是否因语句顺序而不同？MIR typeck 在已定型 MIR 上主要为了收集什么？
2. 两段各自的 `&n` 会在 `BorrowSet` 中记录哪些关键事实？任选三个 `BorrowData` 字段说明。
3. A 中在写入 `n = 2` 前，借用为何可以不再活跃？写出 region 解与借用数据流之间的联系。
4. B 中写入时为何报冲突？只说“`BorrowSet` 中有 `&n`”是否充分？

### E02. MIR typeck 的产物与第 18 章的接口

```rust
fn take<'a, 'b>(x: &'a u32) -> &'b u32 { x }
```

1. 返回表达式使 MIR typeck 产生哪条 region-outlives 要求？它应进入 `MirTypeckRegionConstraints` 的哪个集合？
2. 哪个阶段会传播该要求并检查它是否由已知 universal 关系支持？从该阶段可得到什么主要结果？
3. 概念上的 `T: 'r` 属于 `outlives_constraints` 中的两-region 边，还是 `type_tests`？哪一步核验？
4. 若用户显式写 `let y: &'static u32 = x;`，为什么 region 重新编号之后 MIR typeck 仍需检查这项用户注解，而不能把 `'static` 的要求丢掉？

### E03. 从借用目录到位置冲突

1. 对 `let r = &n`，`BorrowData.borrowed_place` 与 `assigned_place` 分别指向什么？`region` 保存什么？
2. `BorrowSet` 与某位置的 `borrows_in_scope` 有什么区别？`Borrows::kill_loans_out_of_scope_at_location` 根据哪类预计算结果移除借用？
3. 常规路径的 `Borrows` 如何借助 `regioncx` 找到借用离开作用域的位置？写出关键查询名或“首次不包含的 MIR 位置”。
4. `check_access_for_conflict` 除了活跃借用索引，还必须比较什么，才能判定一次写入与借用冲突？至少写出两个维度。

### E04. 旁支约束、Polonius 与诊断

1. 概念化的 member constraint `'m ∈ {'a, 'b}` 是要求 `'a: 'b`、`T: 'm`，还是要求 `'m` 从可捕获的候选 region 中选择等价者？当前源码主要在什么对象的定义性使用处理中应用它？
2. 当前 `compute_definition_site_hidden_types` 应用 member constraints 时，是否直接改写常规 `compute_regions` 的最终 region 图？简述它使用的上下文。
3. 当前 `-Zpolonius=next` 开启时，`PoloniusContext` 主要为哪种信息提供另一条计算路径？不启用时能否据此说 borrowck 不运行？
4. `OutlivesConstraint` 除 `sup/sub` 外有哪些能帮助诊断“要求从何而来”的字段？任选两个，并说明这些字段是求解前提还是诊断来源信息。

### E05. 定向复核：约束从哪里求解

不计入原始 E01–E04 的 8 分总分；复答用于确认 E02.2。

1. `type_check::type_check` 对 region 约束做什么，它是否已求出最终的 `Values(R)`？
2. 哪个入口将 MIR typeck 的约束交给 region inference？哪个方法负责传播并检查 `type_tests` 与 universal 边界？
3. 该入口的主要输出是什么？`universe_causes` 在其中属于求解结果还是 universe 来源信息？

## 学习者答案

### E01

> 练习一：1. 否，为了收集 region constraints；2. 记录 region NLL 变量，reserve\_location 借用开始处，borrowed\_place 被借用的对象，assigned\_place 保存一用的 MIR 位置；3. 因为 r 已经经过最后一次 use，region r 从 let r = &n 存活到 println，然后失活；4. 因为写入时借用仍存活，不充分，因为 BorrowSet 不是借用活跃集合。

### E02

> 练习二：1. 'a: 'b，进入 outlives\_constraints；2. type\_check 阶段会传播和检查，得到 universe\_causes；3. 属于 type\_tests，后续再检查；4. 因为即使重新编号后，'static 的要求仍然需要成立。

### E03

> 练习三：1. n 和 r，region 存 r 的 mir region vid 编号；2. BorrowSet 保存所有借用的记录，borrows\_in\_scope 只获取当前位置的需要的借用，根据终止借用；3. 通过 regioncx.first\_non\_contained\_inclusive；4. 还有 Place、读写种类和借用种类。

### E04

> 练习四：1. 要求 'm 从可捕获的候选 region 中选择等价者，opaque type；2. 不会改写，用的是临时图，后续会再重新计算；3. w为 lone\_liveness 的计算，不能；4. span，from\_closure，都是诊断来源信息。

### E05

> 复核：1. 收集/检查 MIR 关系，求解 constraints 和 universal region relations，已求出；2. compute\_regions，check\_type\_tests 和 check\_universal\_regions；3. 产出 regioncx，属于求解结果

> 复核：1. 收集/检查 MIR 关系，返回待求解的 constraints 和已知的 universal region relations，尚未求出最终 Values(R)，typeck 后的 region inference 才会得到最终 Values(R)；2. compute\_regions 调用 solve，check\\\_type\\\_tests 和 check\\\_universal\\\_regions；3. 产出 regioncx，属于来源信息，记录 universe 的来源。
