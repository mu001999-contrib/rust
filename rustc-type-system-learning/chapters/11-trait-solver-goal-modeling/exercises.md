---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "11"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-09-05
---

# 11. 习题

## 作答说明

本轮共四题，每题 2 分，共 8 分。请优先写出 `Goal`、分派函数、nested-goal 队列或 certainty 的变化；可使用概念化 IR，不要求填写真实 `DefId`。

## 题目

### E01. Obligation、Goal 与 ParamEnv

typeck 中有：

```text
Obligation {
  predicate: Wrapper<T>: Ready,
  param_env: [T: Clone],
  cause: CallArgument(span),
  recursion_depth: 3,
}
```

回答：

1. 传给 solver 的 `Goal` 包含哪两个字段及其具体值？
2. `cause` 和 `recursion_depth` 为什么不进入 `Goal`？
3. 若 predicate 相同但 `param_env = []`，它与原 goal 是否是同一个逻辑查询？为什么？
4. `Goal<I, P>` 的 `P` 为什么不固定为 `I::Predicate`？

### E02. `compute_goal` 分派与 decomposition

给出四个 query-local goals：

```text
G1: Goal(P, Vec<T>: Clone)
G2: Goal(P, 'a: 'b)
G3: Goal(P, WF(Vec<T>))
G4: Goal(P, A <: B)
```

回答：

1. G1–G4 分别由 `compute_goal` 分派给哪个 `compute_*_goal`？
2. 哪一个主要进入 trait candidate assembly？
3. 哪一个主要登记 region outlives constraint？
4. G3、G4 分别可能如何产生 nested goals？

### E03. nested-goal fixpoint

某 candidate 添加了 `[G1, G2]` 两个 nested goals。求值轨迹为：

```text
round 1:
  G1 -> Certainty::Maybe(Ambiguity), HasChanged::Yes
  G2 -> Certainty::Yes,              HasChanged::No

round 2:
  G1 -> Certainty::Yes,              HasChanged::No
```

回答：

1. round 1 后哪些 goal 会重新进入 `nested_goals`？
2. 为什么 round 1 后不能立即以 `Maybe` 结束？
3. 该 candidate 最终 certainty 是什么？
4. 如果 round 1 的 G1 改为 `Maybe + HasChanged::No`，且没有其他 goal 产生进展，fixpoint 返回什么？如果 G1 返回 `NoSolution` 又会怎样？

### E04. `GoalSource`、`PathKind` 与 cycle

根据当前 next solver 的规则，回答：

1. `GoalSource::TypeRelating` 对应哪个 `PathKind`？
2. 当前 goal 是 coinductive trait 时，`GoalSource::ImplWhereBound` 对应哪个 `PathKind`？
3. `GoalSource::Misc` 对应哪个 `PathKind`？
4. 假设处于 `TypingMode::Typeck`，遇到 cycle 时，`Inductive`、`Unknown`、`Coinductive` 三者的初始 provisional result 分别是什么？这里的 `Certainty::Maybe` 与 `NoSolution` 有什么区别？

## 学习者答案

### E01

> 1. param\_env 和 predicate，[T: Clone], Wrapper\<T>: Ready; 2. 主要是 fulfillment 在外层用于诊断和控制求解上限的，solver 在求解 goal 时不需要关心；3. 不共用，全局缓存不同；4. 因为有不同类型的 Predicate。

### E02

> 1. trait，region\_outlives，well\_formed，subtype；2. G1；3. G2；4. G3 可能产生 T: Sized，G4 应该不产生。

### E03

> 1. G1；2. 因为有 HasChanged；3. Yes；4. 返回 Maybe，返回 NoSolution。

### E04

> 1. 对应 Inductive；2. Coinductive；3. Unknown；4. NoSolution，Maybe，Yes，区别是 NoSolution 一定无解，Maybe 可能是缺少条件。
