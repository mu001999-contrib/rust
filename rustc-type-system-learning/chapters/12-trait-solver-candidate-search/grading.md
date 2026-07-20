---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "12"
document: grading
status: completed
exercise_version: 2
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-09-25
---

# 12. 评分与反馈

## 总评

E05 三项复核通过，作为原小问的修正答案计分；E01–E04 当前为 8/8（100%）。候选匹配、impl where-clause 的环境继承与相同 canonical response 的合并已确认。E05 不独立增加总分。

E01–E04 原答与 E05 复答均保存在 `exercises.md`。

## 待复核小问（未满分）

无。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 已掌握 | E05.1 补齐 fast reject 后的实例化、匹配与条件求值。 |
| E02 | 2 | 2 | 已掌握 | E05.2 明确 `String: Clone`、`goal.param_env` 与 `ImplWhereBound`。 |
| E03 | 2 | 2 | 隔离与不同约束的合并已掌握 | 四小问各 0.5；“互不干扰”按候选隔离的概括计分，具体机制是 probe。 |
| E04 | 2 | 2 | 已掌握 | E05.3 确认完全相同的 canonical response 可合并，无需唯一确定候选来源。 |

### E01. 候选来源与索引

1. 使用 ParamEnv candidate，匹配环境中的 `T: Clone` assumption。
2. 可以作为 impl candidate，且 Self 的外层构造器为 `Store`，可按它索引 non-blanket bucket；K/T 等内部泛型参数随后再实例化和匹配。
3. 使用 builtin auto-trait candidate，按字段类型产生 Send 要求。
4. `args_may_unify` 是快速筛查。后续检查 polarity 等条件，进入 probe，fresh args 实例化 impl header，对完整 trait-ref 做 eq，实例化 where-clauses，求值这些条件及 relation 产生的 nested goals，导出 canonical response。E05.1 已补齐关键的实例化、匹配与子条件求值，更新为 0.5。

源码：`compiler/rustc_next_trait_solver/src/solve/trait_goals.rs::consider_impl_candidate`；`compiler/rustc_middle/src/ty/trait_def.rs::for_each_relevant_impl`。

### E02. Predicate 来源与 ParamEnv 继承是两件事

1. header 为 `Store<?K>: Convert<?K>`。
2. 约束为 `?X = ?K = String`，所以两个变量都解析为 String。
3. predicate 是 `String: Clone`；其 ParamEnv 是外层当前 goal 的 `goal.param_env`，GoalSource 是 `ImplWhereBound`。E05.2 将三者分别答出，更新为 0.5。
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
2. 完全相同的 canonical responses 可以合并，结果仍为 `Yes, ?A = u32`。因为每条保留途径给调用方相同的答案，此时无需先唯一确定 candidate 的来源。E05.3 已确认此点，更新为 0.5；以下源码是对应的 EqualResponse 分支。
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

### E05. 定向复核与原题映射

| 复核题 | 对应原小问 | 结果 |
|---|---|---|
| E05.1 | E01.4 | 实例化 → header 匹配 → 条件求值，更新为 0.5。 |
| E05.2 | E02.3 | `String: Clone`、`goal.param_env`、`ImplWhereBound` 均正确，更新为 0.5。 |
| E05.3 | E04.2 | 相同 canonical response 可合并且无需唯一来源；coherence 是另一项检查，更新为 0.5。 |

## 已掌握概念

- ParamEnv、impl 与 builtin auto-trait 来源。
- Self 外层构造器与 non-blanket 索引。
- header 实例化与共享 impl args 的变量约束。
- CanonicalResponse 带出结果，probe 不直接提交 inference 赋值。
- 候选隔离、不同约束的 ambiguity，以及输入确定后的候选筛选。
- 空候选集合的 NoSolution 与无约束 Yes 的 AlwaysApplicable 分支。
- E05 确认了 fast reject 之后的匹配、impl where-clause 的环境继承与 EqualResponse 合并。

## 后续复核重点

无；后续章节可在实际候选搜索例子中继续运用 probe、ParamEnv 与 response 合并规则。

## 补充练习或复习动作

第 12 章无需补充练习。

## 完成判定

当前为 `completed`，8/8，`mastery: mastered`。课程当前断点仍以 `STATE.md` 为准。

## 复核记录

2026-09-05：已保存 E01–E04 原答并完成源码对照讲评；E05 已发布，待作答。

2026-09-25：E05 三项复答已记录并通过，E01.4、E02.3、E04.2 更新为满分，总成绩 8/8，判定 `mastered`。
