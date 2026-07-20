---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "07"
document: grading
status: completed
exercise_version: 1
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-08-02
---

# 07. 评分与反馈

## 当前状态

E01–E04 已评分：7.5 / 8，掌握度 `mastered`。可进入下一章。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 正确 | 能正确区分 `ClauseKind::Trait`、`ClauseKind::Projection`、outlives clauses 与非 clause 的 subtype predicate；`Subtype` 拼写后续统一为 rustc variant 名。 |
| E02 | 2 | 2 | 正确 | parent predicates、默认 `Sized`、supertrait elaboration 与 `ParamEnv` candidate 的证明路径都正确。 |
| E03 | 2 | 2 | 正确 | 能区分 ADT inferred outlives、函数签名 assumed-WF implied outlives、以及 trait bound 不会自动 implied 的边界。 |
| E04 | 1.5 | 2 | 基本正确 | 证明链、nested goal 与 `ParamEnv` 影响 query key 的理解正确；`Vec<T>: Clone` 的 impl 在 rustc impl 索引语境中是 `non_blanket_impls[Adt(Vec)]` 里的泛型 impl，不是 blanket impl。 |

## 已掌握概念

- `PredicateKind::Clause(ClauseKind::Trait/Projection/Outlives)` 与 `PredicateKind::Subtype` 的边界。
- `GenericPredicates::parent` 带来的父层 predicates 递归实例化。
- supertrait elaboration 将 `T: Child` 推出 `T: Base`，并可作为 `ParamEnv` candidate 使用。
- ADT inferred outlives 与函数体 assumed-WF implied outlives 的存储路径区别。
- impl candidate 通过 head matching 产生 nested goals，nested goals 再由 `ParamEnv` 证明。
- query/cache key 需要携带 `ParamEnv`，因为同一 predicate 在不同环境下证明结果不同。

## 后续复核重点

- 区分“泛型 impl”和 rustc impl 索引中的 `blanket_impls`：`impl<T> Clone for Vec<T>` 是泛型 impl，但按 self type 可简化为 `Adt(Vec)`，因此属于 non-blanket bucket。
- 表述 `PredicateKind` 层级时，`Trait` 是 `ClauseKind::Trait`，完整外层是 `PredicateKind::Clause(...)`。

## 补充练习或复习动作

进入第 08 章前，可用一个小例子口头复核：`impl<T> Trait for Vec<T>` 与 `impl<T> Trait for T where T: Copy` 在 `trait_impls_of` 中分别落入哪个 bucket。

## 完成判定

当前章节状态：`completed`。达到 80% 阈值并形成稳定心智模型，判定 `mastered`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-08-02 | 完成讲授、当前 rustc 源码精读并发布 E01–E04 | 等待提交。 |
| 2026-08-02 | 提交 E01–E04 并完成评分 | 7.5 / 8；第 07 章 `mastered`，可进入下一章。 |
