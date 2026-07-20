---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "07"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-08-02
---

# 07. 习题

## 作答说明

本轮共四题，每题 2 分，共 8 分。请同时写出结论和数据所在层次，例如 `ClauseKind`、`GenericPredicates`、`ParamEnv` 或 outlives environment。

## 题目

### E01. Predicate 与 Clause 分类

分别判断以下 IR 应属于哪个 variant，并说明它能否作为普通 `ParamEnv::caller_bounds` 中的 `Clause`：

1. `T: Iterator<Item = U>`；注意它会拆成哪些逻辑事实。
2. `'a: 'b`。
3. `T: 'a`。
4. type relation 暂时无法处理而产生的 `?T0 <: ?T1`。

### E02. Parent predicates 与 elaboration

考虑：

```rust,ignore
trait Base {}
trait Child: Base {}

struct Wrapper<T>(T);

impl<T: Child> Wrapper<T> {
    fn run<U: Clone>(&self, value: U) {}
}
```

回答：

1. `run` 的 `GenericPredicates` 中，impl 层和 method own 层各自主要保存哪些 clauses？包括默认 `Sized`。
2. `tcx.predicates_of(run).instantiate_identity(tcx)` 为什么会同时得到 `T` 与 `U` 的约束？
3. 完成 elaboration 后，`ParamEnv` 还会增加哪条由 supertrait 得来的 clause？
4. 在 `run` 内证明 `T: Base` 时，最终可使用哪类 candidate？

### E03. 三类隐含信息

考虑：

```rust,ignore
use std::fmt::Debug;

struct Ref<'a, T>(&'a T);

fn use_ref<'a, T>(x: &'a T) {
    requires_outlives::<'a, T>();
}

fn requires_outlives<'a, T: 'a>() {}

struct NeedsDebug<T: Debug>(T);
fn use_debug<T>(x: NeedsDebug<T>) {}
```

回答：

1. `Ref<'a, T>` 所需的 `T: 'a` 通过什么机制得到，是否会进入它的 `predicates_of`？
2. `use_ref` 函数体为何可以使用 `T: 'a`？它是否通常直接存于该函数的 `ParamEnv::caller_bounds`？最终进入哪个环境？
3. `use_debug` 是否能省略 `T: Debug`？
4. 上述 lifetime implied bound 与 `Child: Base` 带来的 `T: Base` 在实现路径上有何区别？

### E04. 一次 goal 的证明链

考虑：

```rust,ignore
fn duplicate<T: Clone>(x: Vec<T>) -> Vec<T> {
    x.clone()
}
```

回答：

1. `x.clone()` 的核心 trait goal 是什么？它属于 `PredicateKind` 的哪一类？
2. 第一层可以选择什么 candidate，并产生什么 nested goal？
3. nested goal 如何利用 `ParamEnv` 完成？
4. 为什么缓存或 query key 不能只记录 predicate，而通常还必须带上 `ParamEnv`？

## 学习者答案

### E01

> 1. Trait(T: Iterator), Projection(<T as Iterator>::Item == U), 可以; 2. RegionOutlives('a: 'b), 可以; 3. TypeOutlives(T: 'a), 可以; 4. PredicateKind::SubType, 不可以。

### E02

> 1. impl: T: Sized, T: Child; method: U: Sized, U: Clone; 2. 因为会递归 parent 的；3. T: Base; 4. 使用 ParamEnv candidate。

### E03

> 1. 通过 implies outlives，字段的 WF-check，会进入 predicates_of；2. 因为参数 x: &'a T 的 wf 要求 T: 'a，通过额外的环境；3. 不能；4. 后者是通过 \*\*elaboration，并且在 predicates\_of 中，前者则不在。

### E04

> 1. Vec<T>: Clone, 属于 Trait；2. 第一层选择 blanket impl, 产生 T: Clone；3. ParamEnv 包含 T: Clone，证明完成；4. 因为 predicate 依赖 ParamEnv，ParamEnv 不同，结果不同。\*\*
