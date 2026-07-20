---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "08"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-08-02
---

# 08. 习题

## 作答说明

本轮共四题，每题 2 分，共 8 分。请同时写出“数据结构层次”和“状态变化”，例如 `Predicate`、`Goal`、`Obligation`、`PendingPredicateObligation`、`ObligationForest`。

## 题目

### E01. Predicate、Goal 与 Obligation

考虑：

```rust,ignore
fn f<T: Clone>(x: Vec<T>) {
    x.clone();
}
```

回答：

1. `x.clone()` 产生的核心 `Predicate` 是什么？
2. 对应的 `Goal` 比 `Predicate` 多了什么？
3. 对应的 `Obligation` 又比 `Goal` 多了哪些字段？
4. 为什么 `Obligation::as_goal()` 可以丢掉 `cause` 和 `recursion_depth`？

### E02. `Changed(children)` 的含义

考虑：

```rust,ignore
impl<T: Clone> Clone for Box<T> { ... }

fn g<T: Clone>(x: Box<T>) {
    x.clone();
}
```

回答：

1. 证明 `Box<T>: Clone` 时，select 到 impl 后 `process_trait_obligation` 应返回哪类 `ProcessResult`？
2. nested obligation 是什么？
3. nested obligation 的 `param_env` 是 impl 的 where-clauses，还是调用点的 `ParamEnv`？
4. `recursion_depth` 如何变化？

### E03. Ambiguity 与 `stalled_on`

考虑概念性状态：

```text
pending obligation: ?T: Iterator
```

当前 `?T` 还没有被解析。

回答：

1. `poly_select` 没有足够信息时，`process_trait_obligation` 返回什么？
2. `stalled_on` 会记录什么？
3. 下一次 `try_evaluate_obligations` 为什么可以跳过它？
4. 如果后来 `?T = Vec<u32>`，为什么它又需要被重新处理？

### E04. 错误链与 cause

考虑：

```rust,ignore
fn h<T>(x: Vec<T>) {
    x.clone();
}
```

没有 `T: Clone` bound。

回答：

1. 根 obligation 可以概念化为什么？
2. 选择 `Vec<T>: Clone` 的 impl 后会派生出哪个 child obligation？
3. 最终失败的是哪一个 obligation？
4. 为什么错误诊断还应该回溯到 `x.clone()`？

## 学习者答案

### E01

> 1. Vec<T>: Clone; 2. 多 ParamEnv; 3. 多 cause 和 recursive_depth; 4. 因为 as_goal 是给 trait solver 用的，不需要这些，这些是给提供诊断信息的时候用的。

### E02

> 1. 返回 Changed + [T: Clone]; 2. 是指一个 obligation 证明需要的子 obligation；3. 调用点的；4. +1。

### E03

> 1. 返回 Unchanged；2. 记录 ?T，变化的时候才重新证明；3. 因为没有变化；4. 因为 ?T 改变了，因此证明依赖的条件变化了，因此需要重新证明。

### E04

> 1. Vec<T>: Clone; 2. T: Clone; 3. T: Clone; 4. 因为 cause 和 recursive_dep 会记录证明链。
