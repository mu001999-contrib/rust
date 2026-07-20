---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "03"
document: grading
status: graded
exercise_version: 1
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-07-21
---

# 03. 评分与反馈

## 总评

得分 `8 / 8`（100%），四题全部正确。学习者能够展平 parent chain、按绝对 index 对齐完整 `GenericArgs`，并正确区分 `EarlyBinder` 实例化与内部真正 `Binder` 的 shift 行为。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 正确 | `Access` 的四个 own slots、`fetch` 的 parent prefix 和 own slot，以及完整 identity args 均正确。答案中的 `C` 对应题目中的 `V`。 |
| E02 | 2 | 2 | 正确 | 具体签名完全正确；外层 early params 被 args 替换，内层 `Binder` 保留，遍历其 value 时 `binders_passed` 增加。 |
| E03 | 2 | 2 | 正确 | binder 外的 replacement 保持 D0；插入内部 binder 后 shift 为 D1，以继续指向原 binder。 |
| E04 | 2 | 2 | 正确 | `extend_to` 得到完整 parent + child args；`rebase_onto` 正确替换来源前缀并保留 child 后缀。 |

## 逐题解释

### E01：parent prefix 与绝对 index

`Access` 没有 parent，因此：

```text
parent_count = 0
own_params   = [Self#0, 'env#1, K#2, CAP#3]
```

`fetch` 继承四个 parent slots，只新增一个 method slot：

```text
parent_count = 4
own_params   = [V#4]
identity     = [Self#0, 'env#1, K#2, CAP#3, V#4]
```

答案中的 `C` 与题目里的 `V` 承担同一个 index 4 slot，不影响参数布局判断。

### E02：两层 wrapper 的不同处理

完整 args 按 `[Self, 'env, K, CAP, V]` 对齐：

```text
[Store, 's, String, 8, bool]
```

实例化结果为：

```text
Binder<fn(&'s Store, String, bool) -> (bool, [String; 8])>
```

外层 `EarlyBinder` 被指定 args discharge。内部 `Binder` 仍是函数签名的 late-bound 边界；`ArgFolder` 进入其 value 时令 `binders_passed += 1`。本题 replacements 不含 escaping bound vars，所以无需实际 shift。

### E03：replacement 所在位置决定 shift

同一个 `&'a u32` replacement 被放入两个不同深度的位置：

```text
fn(&'a[D0] u32, Binder<fn(&'a[D1] u32)>)
```

第二处位于一层新 binder 内，只有从 D0 shift 到 D1 才能继续指向原来的外层 binder。

### E04：两个 args 变换方向

`extend_to` 复用已有的 parent slots，并生成缺失的 method slot：

```text
[Store, 's, String, 8]
  -> [Store, 's, String, 8, ?V]
```

`rebase_onto` 用 impl 的 `[U]` 替换 trait `X` 的 `[Self, S]` 前缀，同时保留 method 后缀 `[T]`：

```text
[Self, S, T] -> [U, T]
```

## 已掌握概念

- `Generics::parent_count` 与 `own_params` 共同描述完整参数布局。
- `GenericParamDef::index` 是 parent chain 展平后的绝对 slot。
- identity 和具体 `GenericArgs` 使用同一 slot 顺序。
- `EarlyBinder::instantiate` 按 index 替换 item params，同时保留内部真正的 `Binder`。
- `binders_passed` 只在 replacement 含 escaping bound vars 时引发实际 shift。
- `extend_to` 补齐 descendant 后缀；`rebase_onto` 替换 ancestor 前缀。

## 后续复核重点

后续保持区分：`binders_passed` 增加表示 replacement 所在深度发生变化；只有 replacement 含 escaping bound vars 时才需要实际 shift。

## 补充练习或复习动作

第 04 章将把 early-bound region 与 late-bound、placeholder、universe 放在同一张 region 表示图中继续复核。

## 完成判定

- 四题全部正确，成绩 100%。
- 能从 parent chain 手算完整 args、identity args 与具体实例化。
- 能解释 item substitution 和内部 binder shift 的协作关系。
- 能正确选择 `extend_to` 与 `rebase_onto`。

结论：第 03 章 `completed`，`mastery: mastered`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-07-21 | 完成 E01–E04 综合评估 | `8 / 8`，`mastered`，章节完成。 |
