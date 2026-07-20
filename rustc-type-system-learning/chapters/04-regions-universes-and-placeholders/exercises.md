---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "04"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-07-26
---

# 04. 习题

## 作答说明

本轮练习覆盖 region 表示的阶段转换、universe nameability、`ReLateParam` / `RePlaceholder` 的边界，以及 placeholder escape。每题 2 分，共 8 分。

可以使用 `D0`、`U0`、`P1@U1` 等简写；重点是变量身份、可见性方向和转换阶段。

## 题目

### E01. Region 表示分类

分别写出下列位置的主要 region variant：

1. `struct Hold<'a>(&'a u32)` 定义内部的 `'a`；
2. `fn borrow<'a>(x: &'a u32) -> &'a u32` 的 poly signature 中、尚未打开 binder 的 `'a`；
3. 检查 `borrow` 函数体、执行 `liberate_late_bound_regions` 后的 `'a`；
4. 调用点为省略的 lifetime 创建的 region inference variable；
5. 用 `enter_forall` 打开 `for<'a> fn(&'a u32)` 后的 `'a`；
6. codegen 阶段已经擦除 identity 的 region。

### E02. Universe nameability

按以下顺序创建值：

```text
?r0 创建于 U0
P1 创建于 U1
?r1 创建于 U1
P2 创建于 U2
?r2 创建于 U2
```

回答：

1. `?r0`、`?r1`、`?r2` 分别能否采用 `P1` 和 `P2` 作为解？
2. `U1.can_name(U0)`、`U1.can_name(U2)` 分别是什么？
3. universe index 的大小能否直接说明 region 的 outlives 长短？

### E03. Liberate 与 enter_forall

比较两个操作：

1. 检查：

   ```rust
   fn id<'a>(x: &'a u32) -> &'a u32
   ```

   从 poly signature 进入函数体时，`'a` 从什么表示变成什么表示？新表示的身份由哪些字段确定？
2. 在 U0 中临时打开：

   ```text
   for<'b> fn(&'b u32)
   ```

   `'b` 从什么表示变成什么表示？新表示的身份由哪些字段确定？

说明为什么这两个操作不能互换。

### E04. Placeholder escape

外层有 `?r0@U0`。进入 `forall<'a>` 后，`'a` 被替换为 `P1@U1`，检查过程产生：

```text
?r0@U0 = P1@U1
```

回答：

1. `?r0` 能否把 `P1` 作为解？为什么？
2. 这条约束试图证明的量词含义有什么问题？
3. 若另一个 inference variable `?r1` 是进入 U1 后才创建的，它在 nameability 上能否引用 `P1`？

## 学习者答案

### E01

> 练习一：1.  ReEarlyParam；2. ReBound；3. ReLateParam；4. ReVar；5. RePlaceHolder；6. ReErased。

### E02

> 练习二：1. ?r0 都不能，?r1 可以用 P1，?r2 两个都可以；2. true, false；3. 不能。

### E03

> 练习三：1. 从 ReBound 到 ReLateParam，scope + kind；2. 从 ReBound 到 RePlaceHolder，universe 和 bound identify；不能互换是因为后者表示对所有，前者是对函数体内的可以反复引用的 region。

### E04

> 练习四：1. 不能，因为 P1 是 U1 产生的，U1 中的 name 不能在 U0 中使用；2. 不知道；3. 可以。

## 提交记录

| 日期 | 轮次 | 说明 |
|---|---:|---|
| 2026-07-26 | 1 | 提交 E01–E04；答案原文如上。 |
