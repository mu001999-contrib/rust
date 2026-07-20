---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "09"
document: grading
status: completed
exercise_version: 1
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-08-12
---

# 09. 评分与反馈

## 当前状态

E01–E04 已评分：7.5 / 8，掌握度 `mastered`。可以进入第 10 章。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 正确 | `Projection`、关联项 `def_id`、完整 args、普通 alias eager expansion 与当前 `Free` 命名均正确。 |
| E02 | 2 | 2 | 正确 | 能准确拆分 trait clause 与 projection clause，并识别由 `ParamEnv` candidate 提供关联项值。 |
| E03 | 2 | 2 | 正确 | fresh impl header、`eq` 映射、normalized value 与 impl where-clause nested obligation 全部正确。 |
| E04 | 1.5 | 2 | 基本正确 | old solver 的推理变量占位、deferred projection obligation、后续解析以及隔离 `Expected` 的目的正确；`NormalizesTo.term` 进入候选计算时要求是完全未约束的 type/const inference variable，`normalized` / `ambiguity` 描述的是求解结果或 certainty。 |

## 已掌握概念

- `AliasTyKind` 与完整 `GenericArgs` 的构造。
- 普通 alias eager expansion 与 checked/free alias 的边界。
- trait clause 和 projection clause 的职责区别。
- `ParamEnv` 与 impl projection candidate 的信息来源。
- impl header fresh instantiation、等式映射和 nested obligations。
- ambiguity 时通过输出推理变量与 deferred projection obligation 保留约束。
- new solver 使用内部 `NormalizesTo(alias, ?U)` 隔离 expected term。

## 后续复核重点

- `NormalizesTo.term` 是候选计算的输出槽，进入计算时必须是完全未约束且能够命名相关 universe 中变量的 inference variable。
- `Certainty::Yes` / `Maybe` 描述 goal evaluation 结果，不是 `term` 进入 solver 时的状态。

## 补充练习或复习动作

第 10 章 canonicalization 中复核为什么 solver query 的输出变量必须与输入中的 inference 状态清晰分离。

## 完成判定

当前章节状态：`completed`。综合评分 7.5 / 8，达到 `mastered` 标准。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-08-05 | 完成第 09 章讲授、当前 rustc 源码精读并发布 E01–E04 | 等待提交。 |
| 2026-08-12 | 提交 E01–E04 并完成评分 | 综合评分 7.5 / 8，掌握度 `mastered`。 |
