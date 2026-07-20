---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "10"
document: grading
status: completed
exercise_version: 1
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-08-17
---

# 10. 评分与反馈

## 当前状态

E01–E04 已评分：7.5 / 8，掌握度 `mastered`。可以进入第 11 章。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 正确 | canonical value、按首次出现顺序建立的变量表、`OriginalQueryValues` 和重复变量去重全部正确；`Ty(U0)` 是省略 `sub_root` 的可接受简写。 |
| E02 | 1.5 | 2 | 基本正确 | existential/universal 分类和 placeholder 的刚性语义正确。`^0` 实例化为 type inference variable，`^2` 实例化为 const inference variable；`'^1` 应实例化为 `RePlaceholder(P@U1)`，不成为 `ReVar`。`max_universe` 用于在 query-local `InferCtxt` 中重建 universe 层级，保持 nameability 与 leak-check 语义。 |
| E03 | 2 | 2 | 正确 | 准确理解 `response.var_values[0]` 是 input slot 0 的求解结果，并能追踪等式建立、调用方变量解析和 query-local 身份隔离。 |
| E04 | 2 | 2 | 正确 | outlives constraint 的回放、赋值与关系约束的区别、old response 字段和 new solver external constraints 全部正确。 |

## 已掌握概念

- 按首次出现顺序 canonicalize inference variables，并保持重复变量的同一性。
- `OriginalQueryValues` 与 canonical input slot 的调用方映射。
- existential canonical variables 与 universal placeholders 的逻辑区别。
- type、region、const canonical slots 与各自本地 `Vid` 编号空间的区别。
- response `var_values` 的位置含义及其向调用方的回放。
- query-local inference variable 不能直接进入缓存或返回调用方。
- `var_values` 与 region/external constraints 的职责边界。
- old `QueryResponse` 与 new solver `ExternalConstraintsData` 的字段组织。

## 后续复核重点

- `PlaceholderRegion(P@U1)` 实例化后仍是 `RePlaceholder`；只有 existential `Region(Ui)` 才实例化为 `ReVar`。
- `max_universe` 不只标记变量类别，而是让目标 `InferCtxt` 重建 universe 偏序，从而保持“哪个变量能够命名哪个 placeholder”的约束。

## 补充练习或复习动作

第 11 章在追踪 `EvalCtxt` 与 goal decomposition 时，继续观察 canonical input 如何通过 `enter_canonical` 变成 query-local goal；第 17 章再次复核 placeholder 与 universe nameability。

## 完成判定

当前章节状态：`completed`。综合评分 7.5 / 8，达到 `mastered` 标准。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-08-12 | 完成第 10 章讲授、当前 rustc 源码精读并发布 E01–E04 | 等待提交。 |
| 2026-08-17 | 提交 E01–E04 并完成评分 | 综合评分 7.5 / 8，掌握度 `mastered`。 |
