---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "11"
document: grading
status: completed
exercise_version: 1
earned_points: 7.75
max_points: 8
mastery: mastered
updated_at: 2026-09-05
---

# 11. 评分与反馈

## 总评

E01–E04 已评分：7.75 / 8（96.875%），掌握度 `mastered`。第 11 章完成，第 12 章尚未开始。

每题 2 分，四个小问各 0.5 分。E02 第 4 小问包含 G3、G4 两个判断，各占 0.25 分；本次 G3 部分得 0.25 分。其余小问均得 0.5 分。原始回答保存在 `exercises.md`，以下是评分及对应解释。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 已掌握 | Goal 字段、外层调度信息与 typed goal 的用途正确；两个环境形成不同逻辑查询，缓存表仍可共用，区分在 key。 |
| E02 | 1.75 | 2 | 分派与 WF 分解已掌握 | 四个分派、trait assembly 与 region constraint 来源正确；G3 可产生 `T: Sized`。G4 经 type relation 也可产生 subtype、projection 或 WF nested goals。 |
| E03 | 2 | 2 | 已掌握 | `[G1]` 重新入队，`HasChanged::Yes` 驱动下一轮，题设最终为 `Yes`；无进展返回 `Maybe`，必需的 nested goal 无解则当前 candidate 返回 `NoSolution`。 |
| E04 | 2 | 2 | 已掌握 | 三种 path 映射及 Typeck 下的初始 provisional result 正确；将 `NoSolution` 限定在当前 goal/environment，将 `Maybe` 理解为尚未确定的结果。 |

### E01. Goal 与查询上下文

1. `Goal { param_env: [T: Clone], predicate: Wrapper<T>: Ready }`。
2. `cause` 服务于诊断，obligation 的 `recursion_depth` 服务于外层调度和限额控制；solver 自己仍通过 search graph 管理递归深度、cycle 和 fixpoint 限制。
3. `[T: Clone]` 与 `[]` 是不同 assumptions，因此构成不同逻辑查询。原答“不共用”按“不共用该查询结果”计分；实现上两者使用同一个 `TyCtxt` global cache，通过不同 canonical key 区分。
4. `P` 允许外部入口使用 `Goal<I, I::Predicate>`，分派后使用 `Goal<I, TraitPredicate<I>>`、`Goal<I, SubtypePredicate<I>>` 等具体 payload 类型。

源码定位：`compiler/rustc_type_ir/src/solve/mod.rs`，`Goal`；`compiler/rustc_middle/src/ty/context.rs`，`new_solver_evaluation_cache`。

### E02. 分派与关系检查产生的 nested goals

1. G1：`compute_trait_goal`；G2：`compute_region_outlives_goal`；G3：`compute_well_formed_goal`；G4：`compute_subtype_goal`。原答使用函数名中间部分作为简写，含义准确。
2. G1 进入 trait candidate assembly。
3. G2 登记 `'a: 'b` region outlives constraint。
4. G3 经 `well_formed_goals` 获得 Vec 的泛型约束及组成类型的 WF 条件，例如 `T: Sized`。G4 经 `self.sub(...)` 调用 type relation，返回的子任务由 `EvalCtxt::relate` 逐一加入 nested goals。

G4 的具体例子，设 `?X`、`?Y` 是两个不同且尚未确定的类型变量：

```text
Goal(P, (?X, u8) <: (?Y, u8))
  -> 按 tuple 结构比较对应元素
  -> 第一项需要 ?X <: ?Y
  -> relation 返回 nested Goal(P, ?X <: ?Y)
  -> EvalCtxt::relate 以 GoalSource::TypeRelating 登记
```

这类 nested goal 在 fast path 中也可能直接得到 `Maybe` 并记录 `stalled_on`。若关系涉及 projection 或变量 generalization，还可能产生 projection/WF goals；简单已知类型的比较则可以直接完成。

源码定位：

- `compiler/rustc_next_trait_solver/src/solve/mod.rs`，`compute_subtype_goal` / `compute_well_formed_goal`。
- `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`sub` / `relate`：对 relation 返回的 goals 调用 `add_goal`。
- `compiler/rustc_type_ir/src/relate/solver_relating.rs`，`SolverRelating::tys`：协变位置的两个类型变量生成 `SubtypePredicate`。

### E03. nested-goal fixpoint

round 1 后队列是 `[G1]`：`Maybe` 保留，`Yes` 移出。G1 的 `HasChanged::Yes` 表示本轮有 inference progress，因而继续 round 2；题设 round 2 后所有必需 goals 为 `Yes`，在 candidate 自身没有其他不确定性的前提下，最终 certainty 为 `Yes`。

若所有 goals 均无进展且 G1 仍为 `Maybe`，则返回 `Maybe`。若必需的 G1 返回 `NoSolution`，则当前 candidate 求值返回 `NoSolution`；是否还有其他可行 candidate，由外层候选求值与合并决定。

源码定位：`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`evaluate_added_goals_step` / `try_evaluate_added_goals`。

### E04. cycle 的初始结果与最终结果

`TypeRelating -> Inductive`，coinductive trait 的 `ImplWhereBound -> Coinductive`，`Misc -> Unknown`。

在题设 `TypingMode::Typeck` 下，第一次遇到 cycle 时：

```text
Inductive   -> Err(NoSolution)
Unknown     -> 无约束 response，certainty = Certainty::overflow(false)
Coinductive -> 无约束 response，certainty = Certainty::Yes
```

`Certainty::overflow(false)` 属于 `Maybe`，这里来自保守的 cycle 处理；`Maybe` 也可来自 inference 信息不足、候选歧义或求值限制。`NoSolution` 表示当前求值条件下该 goal/candidate 无解。cycle 的初始 `NoSolution` 或 `Yes` 是 provisional result，仍需结合其他前提、候选与 cycle fixpoint 得到最终结果。

源码定位：`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`step_kind_for_source`；`compiler/rustc_next_trait_solver/src/solve/search_graph.rs`，`initial_provisional_result`。

## 已掌握概念

- `Goal` 的 assumptions 与 predicate、typed goal 的字段边界。
- 不同 `ParamEnv` 对逻辑查询及 canonical key 的影响。
- trait、region outlives、WF、subtype 四类 goal 的分派。
- `Maybe` 入队、`HasChanged::Yes` 驱动重试与最终 certainty 合并。
- `GoalSource` 到 `PathKind` 的映射及 Typeck 下的 cycle provisional result。

## 后续复核重点

- 在 impl head matching 时观察 `eq` / `sub` 如何生成新的 subtype、projection、WF goals。
- 继续区分共享 cache 容器与不同 canonical query key。
- 区分单个 candidate 的 `NoSolution`、cycle provisional result 与整个 goal 的最终结果。

## 补充练习或复习动作

第 12 章结合 candidate matching 继续观察 relation 返回的 nested goals；第 17 章复用 provisional response 与 cycle fixpoint 的区别。本次无需额外补交即可进入下一章。

## 完成判定

当前章节状态：`completed`。综合评分 7.75 / 8（96.875%），Goal 建模、分派、fixpoint 和 cycle 基础达到掌握标准。下一章为第 12 章 Trait Solver：候选搜索，状态仍为 `planned`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-08-26 | 完成第 11 章讲授、当前 rustc 源码精读并发布 E01–E04 | 等待提交。 |
| 2026-09-05 | 提交 E01–E04，逐项对照当前源码评分 | 7.75 / 8，`mastered`；讲评补充 subtype nested goals、cache key 和 provisional result 的边界。 |
