---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "08"
document: grading
status: completed
exercise_version: 1
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-08-02
---

# 08. 评分与反馈

## 当前状态

E01–E04 已评分：7.5 / 8，掌握度 `mastered`。可以进入下一章。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 正确 | 能区分核心 predicate、`Goal = ParamEnv + Predicate`，以及 `Obligation` 额外携带的 `cause` 与 `recursion_depth`。字段名记录为 `recursion_depth`。 |
| E02 | 2 | 2 | 正确 | `Changed(children)`、nested obligation、调用点 `ParamEnv` 继承与 depth `+1` 都掌握到位。 |
| E03 | 2 | 2 | 正确 | ambiguity 暂停、`stalled_on` 记录推理变量、变量变化后重新处理的模型清晰。 |
| E04 | 1.5 | 2 | 基本正确 | 根 obligation、child obligation 与最终失败点判断正确；复核点是诊断链主要由 `cause` / derived cause / forest backtrace 表达，`recursion_depth` 主要服务递归深度与 overflow 控制。 |

## 已掌握概念

- `Predicate`、`Goal`、`Obligation` 的层次边界。
- `FulfillmentContext` / `ObligationForest` 中 `Unchanged`、`Changed(children)` 与重新处理条件。
- impl 选择后由 impl where-clauses 派生 nested obligations。
- nested obligation 继承调用点 `ParamEnv`，同时递增 `recursion_depth`。
- ambiguity 通过 `stalled_on` 依赖推理变量变化进行调度。
- 错误诊断需要保留从根 obligation 到派生 child obligation 的上下文链。

## 后续复核重点

- 精确区分 `ObligationCause` / derived cause / forest backtrace 与 `recursion_depth`：前者承载诊断上下文，后者承载递归深度控制。
- 统一使用字段名 `recursion_depth`。

## 补充练习或复习动作

进入第 09 章时，继续把 `T: Clone` 这类 nested obligation 的生成接到 selection confirmation 与 query 结果缓存。

## 完成判定

当前章节状态：`completed`。第 08 章达到 `mastered` 标准。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-08-02 | 完成第 08 章讲授、当前 rustc 源码精读并发布 E01–E04 | 等待提交。 |
| 2026-08-02 | 提交 E01–E04 并完成评分 | 综合评分 7.5 / 8，掌握度 `mastered`。 |
