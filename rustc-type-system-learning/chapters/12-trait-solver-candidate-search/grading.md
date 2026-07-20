---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "12"
document: grading
status: graded
exercise_version: 2
earned_points: 7
max_points: 8
mastery: pending
updated_at: 2026-09-05
---

# 12. 评分与反馈

## 总评

E01–E04 已评分：7 / 8（87.5%）。每题 2 分，每小问 0.5 分。候选来源、header 实例化、probe 隔离与不同约束的 ambiguity 已形成清晰认识；下一步通过 E05 复核匹配步骤、ParamEnv 继承及相同 response 的合并，再确认本章完成。

原答保存在 `exercises.md`，对应发布时的第 1 版 E01–E04。第 2 版保留这四题不变，增加 E05 作为定向复核，不另加总分。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 1.75 | 2 | 来源与索引已掌握 | 四小问依次为 0.5、0.5、0.5、0.25；第 4 问明确了筛查边界，后续步骤在下文具体化。 |
| E02 | 1.75 | 2 | 实例化与响应已掌握 | 四小问依次为 0.5、0.5、0.25、0.5；第 3 问的 predicate 为 String: Clone，环境继承当前 goal.param_env。 |
| E03 | 2 | 2 | 隔离与不同约束的合并已掌握 | 四小问各 0.5；“互不干扰”按候选隔离的概括计分，具体机制是 probe。 |
| E04 | 1.5 | 2 | 合并规则已讲评 | 四小问依次为 0.5、0、0.5、0.5；完全相同的 response 可走 EqualResponse，保留 Yes 与相同约束。 |

### E01. 候选来源与索引

1. 使用 ParamEnv candidate，匹配环境中的 `T: Clone` assumption。
2. 可以作为 impl candidate，且 Self 的外层构造器为 `Store`，可按它索引 non-blanket bucket；K/T 等内部泛型参数随后再实例化和匹配。
3. 使用 builtin auto-trait candidate，按字段类型产生 Send 要求。
4. `args_may_unify` 是快速筛查。原答正确说明它只排除明显不匹配的情况，计 0.25；完整后续步骤是：检查 polarity 等条件，进入 probe，fresh args 实例化 impl header，对完整 trait-ref 做 eq，实例化 where-clauses，求值这些条件及 relation 产生的 nested goals，导出 canonical response。只要求表述关键的实例化、匹配与子条件求值，不要求背出所有辅助检查。

源码：`compiler/rustc_next_trait_solver/src/solve/trait_goals.rs::consider_impl_candidate`；`compiler/rustc_middle/src/ty/trait_def.rs::for_each_relevant_impl`。

### E02. Predicate 来源与 ParamEnv 继承是两件事

1. header 为 `Store<?K>: Convert<?K>`。
2. 约束为 `?X = ?K = String`，所以两个变量都解析为 String。
3. predicate 是 `String: Clone`（计 0.25）；其 ParamEnv 是外层当前 goal 的 `goal.param_env`。原答“从 impl candidates”描述了要求的产生来源；题目所问的环境由 `goal.with(cx, pred)` 继承，而不是根据 impl predicates 新建一个环境。
4. 用 `CanonicalResponse` 带出解。probe 返回 Ok 仍不直接提交 inference 赋值；调用方在合并响应后通过实例化响应接收约束。

可把第 3 问拆成三个独立字段：

```text
predicate 的来源：impl 的 predicates_of，经 impl_args 实例化
nested goal.param_env：当前 goal.param_env
GoalSource：ImplWhereBound
```

例如当前环境 `P = [T: Clone]`，外层 goal 为 `Goal(P, Store<T>: Convert<T>)`，使用本题 impl 匹配出 K = T 后，子目标就是 `Goal(P, T: Clone)`。环境里的已有 assumption 可以帮助证明它，但产生这个要求的动作不会自动把要求变成 assumption。

源码：`compiler/rustc_next_trait_solver/src/solve/trait_goals.rs::consider_impl_candidate` 中的 `.map(|pred| goal.with(cx, pred))` 与 `add_goals(GoalSource::ImplWhereBound, ...)`。

### E03. 每份 Yes 都连同自己的约束一起理解

1. 两份响应分别为 `Yes, ?A = u32` 和 `Yes, ?A = bool`；各自在自己的条件下可以成功。
2. 两个候选互不污染。结合 E02 第 4 问对 probe 不提交的说明，这里按隔离概念计满分。实现机制是每个候选在 probe 中试算，保留 canonical response，恢复 inference 状态，再尝试其他候选；仅仅“条件不同”本身不会自动产生隔离。
3. 两份响应不同且没有题设外的偏好依据，保留 `Maybe(Ambiguity)`，不任意选择 u32。
4. 外部确定 `?A = u32` 后，u32 impl 保留，bool impl 的 trait-ref 参数不匹配，被排除。

源码：`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/probe.rs::ProbeCtxt::enter_inner`；`compiler/rustc_next_trait_solver/src/solve/mod.rs::try_merge_candidates`、`bail_with_ambiguity`。

### E04. 不同证明来源可以给出同一个确定答案

1. 题设普通路径的候选集合为空，`flounder` 返回 `NoSolution`。
2. 完全相同的 canonical responses 可以合并，结果仍为 `Yes, ?A = u32`。因为每条保留途径给调用方相同的答案，此时无需先唯一确定 candidate 的来源。本问计 0；以下源码就是对应的 EqualResponse 分支。
3. u32 与 bool 两种不同响应保留 Maybe，不把两份等式同时应用到调用方。
4. 使用无 inference/external constraints 的 Yes response。它不要求调用方额外选择变量值或接受外部约束，就足以证明当前 goal，对应 AlwaysApplicable 分支。

位置：`compiler/rustc_next_trait_solver/src/solve/mod.rs::try_merge_candidates`，以下省略先检查 AlwaysApplicable 的分支：

```rust,ignore
let one: CanonicalResponse<I> = candidates[0].result;
if candidates[1..].iter().all(|candidate| candidate.result == one) {
    return Some((one, MergeCandidateInfo::EqualResponse));
}
```

比较对象是完整 canonical response；题设已确保 certainty、变量值、外部约束等都相同，不只是两个候选都说 Yes。

```text
两个相同答案：Yes, ?A = u32 / Yes, ?A = u32 → Yes, ?A = u32
两个不同答案：Yes, ?A = u32 / Yes, ?A = bool → Maybe
```

这仍是 goal 求值层面的合并，和 codegen 时选择具体 impl instance 是不同任务。

## 已掌握概念

- ParamEnv、impl 与 builtin auto-trait 来源。
- Self 外层构造器与 non-blanket 索引。
- header 实例化与共享 impl args 的变量约束。
- CanonicalResponse 带出结果，probe 不直接提交 inference 赋值。
- 候选隔离、不同约束的 ambiguity，以及输入确定后的候选筛选。
- 空候选集合的 NoSolution 与无约束 Yes 的 AlwaysApplicable 分支。

## 后续复核重点

1. fast reject 之后的实例化、header matching 和 nested-goal 求值。
2. impl predicate 的来源、`GoalSource` 与继承的 `ParamEnv`。
3. EqualResponse 合并：多个来源、同一个答案。

## 补充练习或复习动作

完成 `exercises.md` 的 E05（三个短问）。它不独立加分，分别作为 E01 第 4 问、E02 第 3 问与 E04 第 2 问的复核依据；复核后更新对应当前分数，总分仍为 8。

## 完成判定

当前为 `graded`，7 / 8，`mastery: pending`。总分达到课程参考阈值；E05 留作后续复核，以确认掌握与章节完成。学习者已要求继续下一章，课程当前断点以 `STATE.md` 为准。

## 复核记录

2026-09-05：已保存 E01–E04 原答并完成源码对照讲评；E05 已发布，待作答。
