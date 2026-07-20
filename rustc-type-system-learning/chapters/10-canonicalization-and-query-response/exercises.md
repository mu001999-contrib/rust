---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "10"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-08-17
---

# 10. 习题

## 作答说明

本轮共四题，每题 2 分，共 8 分。题目中的 canonical variables 用 `^0`、`^1` 表示；region canonical variable 写作 `'^N`。请同时写出 canonical value、变量表或应用 response 后的调用方状态。

## 题目

### E01. Canonical input 与重复变量

调用方 `InferCtxt` 中有：

```text
?T7@U0 = Unknown
?T8@U0 = Unknown
'?R3@U0 = Unknown
```

需要 canonicalize 的值为：

```text
(?T7, Vec<?T8>, ?T7, &'?R3 ?T8)
```

假设遍历首次遇到变量的顺序是 `?T7`、`?T8`、`'?R3`。回答：

1. canonical value 是什么？
2. `var_kinds` 是什么？
3. `OriginalQueryValues.var_values` 是什么？
4. 为什么第三个 tuple element 必须仍使用 `^0`，而不能分配 `^2`？

### E02. Existential、placeholder 与 universe

一个 canonical input 为：

```text
Canonical {
  value: Goal(^0, '^1, ^2),
  var_kinds: [
    Ty { ui: U0, sub_root: ^0 },
    PlaceholderRegion(P@U1),
    Const(U0),
  ],
  max_universe: U1,
}
```

回答：

1. `^0`、`'^1`、`^2` 中哪些是 existential canonical vars，哪些是 universal placeholder？
2. 在 query-local `InferCtxt` 中实例化时，三者分别变成什么种类的值？
3. 为什么要根据 `max_universe = U1` 创建/映射 universe 层级？
4. placeholder 为什么不能像 existential type/const variable 一样被任意统一成具体值？

### E03. Canonical response 回放

调用方最初有：

```text
?T0@U0 = Unknown
OriginalQueryValues.var_values = [?T0]
```

canonical query 内部将 input `^0` 实例化为 `?Q0`，求解后得到 `?Q0 = u32`。响应概念化为：

```text
response.var_values = [u32]
certainty = Yes / Proven
region constraints = []
```

回答：

1. `response.var_values[0] = u32` 表示什么？
2. response 应用到调用方时会建立什么等式？
3. 应用后调用方的 `?T0` 状态是什么？
4. 如果查询没有约束 `?Q0`，response 为什么需要用自己的 canonical var 表示它，而不能把 query-local `?Q0` 直接返回？

### E04. Region constraints 与 response 字段

调用方 canonicalize 前有：

```text
OriginalQueryValues:
  '^0 -> 'a
  '^1 -> 'b
```

查询响应携带：

```text
region constraint: '^0: '^1
certainty: Yes
```

回答：

1. response 实例化回调用方后得到什么 outlives constraint？
2. 为什么这个信息不能只放在 `response.var_values` 中？
3. old `QueryResponse` 除 `var_values` 外还包含哪四类核心信息？
4. new solver 的 `ExternalConstraintsData` 主要包含哪三类外部结果？

## 学习者答案

### E01

> 1. (^0, Vec<^1>, ^0, &'^2 ^1); 2. Ty(U0), Ty(U0), Region(U0); 3. ?T7@U0, ?T8@U0, '?R3@U0; 4. 因为和第一个是同一个类型。

### E02

> 1. ^0, ^2 是 existential，^1 是 universal；2. ?Q0@U0, ?Q0@U1, ?Q0@U0; 3. 因为要区分 existential 和 universal；4. 因为是 universal placeholder，需要对 existential variable 都成立。

### E03

> 1. 表示求解得到的 ?T0 的解为 u32; 2. ?T0 == u32; 3. 状态是 solved；4. 因为 query-local ?Q0 是内部表示。

### E04

> 1. 得到 'a: 'b; 2. var_values 只记录对变量的求解，不记录 constraint；3. region_constraints, certainty, opaque_types, value；4. region_constraints, opaque_types, normalization_nested_goals.
