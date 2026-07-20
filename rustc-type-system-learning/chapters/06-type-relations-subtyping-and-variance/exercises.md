---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "06"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-08-01
---

# 06. 习题

## 作答说明

本轮共四题，每题 2 分，共 8 分。请写出关系方向或 variance 推导过程，不只给最终结论。

约定：

```text
A <: B
```

表示 `A` 可以用于需要 `B` 的位置；`'long: 'short` 表示 `'long` outlives `'short`。

## 题目

### E01. `sub/sup/eq` 与 region 约束

回答：

1. `at.sub(E, A)`、`at.sup(E, A)`、`at.eq(E, A)` 分别建立什么关系？其中哪个适合检查实际参数 `A` 能否传给形参类型 `E`？
2. 已知 `'long: 'short`，判断 `&'long u32` 与 `&'short u32` 的 subtype 方向。
3. 调用 `sub(&'a u32, &'b u32)` 时，需要什么 outlives 关系？底层 `make_subregion(sub, sup)` 的两个 region 参数按什么顺序传入？

### E02. 嵌套 variance

分别计算 `T` 在以下完整类型中的最终 variance，并写出 `Variance::xform` 链：

1. `fn(*const Vec<T>)`
2. `fn(fn(T))`
3. `fn() -> *mut Vec<T>`
4. `&'a mut fn(T)`

同时回答：第 4 项中的 `'a` 是什么 variance？

### E03. ADT 参数关系与推理变量

定义：

```rust,ignore
struct Packet<A, B, C> {
    output: A,
    consume: fn(B),
    cell: std::cell::Cell<C>,
}
```

回答：

1. `A`、`B`、`C` 的 definition-site variance 分别是什么？
2. 要证明 `Packet<A1, B1, C1> <: Packet<A2, B2, C2>`，三个参数分别需要什么关系？
3. 对 `Vec<?T0> <: Vec<?T1>`，旧 `TypeRelating` 应直接 equate 两个变量，还是保留有方向的 subtype obligation？对 `Cell<?T0> <: Cell<?T1>` 呢？

### E04. Coercion 与 lattice

回答：

1. `&mut i32 -> &i32`、`&[u8; 3] -> &[u8]`、fn item `f -> fn(i32)` 为什么属于 coercion，而不能全部简化成纯 subtype relation？
2. 已知 `A <: B`，写出 `LUB(A, B)` 与 `GLB(A, B)`。
3. 已知 `'long: 'short`，写出：

   ```text
   LUB(&'long T, &'short T)
   GLB(&'long T, &'short T)
   ```

4. 已知 `A <: B`，简化 `LUB(fn(A) -> R, fn(B) -> R)`，并说明为什么函数输入处使用相反的 lattice operation。

## 学习者答案

### E01

> 练习一：1. E <: A, A <: E, E == A; at.sup; 2. &'long u32 <: &'short u32; 3. 'a : 'b, 'a 'b。

### E02

> 练习二：1. - xform + xform + = -; 2. - xform - = +; 3. + xform o xform + = o; 4. o xform - = o, 'a 是 +。

### E03

> 练习三：1. A +, B -, C o; 2. A1 <: A2, B2 <: B1, C1 == C2; 3. subtype obligation, equate。

### E04

> 练习四：1. 因为需要表达式转换；2. LUB(A, B) = B, GLB(A, B) = A; 3. LUB = &'long T, GLB = &'short T; 4. LUB = fn(A) -> R, 因为函数参数是逆变。

## 学习者修正答案

### Region constraint 与引用 LUB/GLB

> 1. X <: Y; 2. LUB(X, Y) = Y; 3. GLB(X, Y) = X; 4. 'outer: 'inner, sub = 'inner, sup = 'outer

## 提交记录

| 日期 | 轮次 | 说明 |
|---|---:|---|
| 2026-08-01 | 1 | 提交 E01–E04；答案原文如上。 |
| 2026-08-01 | 2 | 提交 region constraint 与引用 LUB/GLB 顺序合并复核；作为 E01、E04 的修正答案。 |
