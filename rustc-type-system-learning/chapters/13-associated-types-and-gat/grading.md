---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "13"
document: grading
status: completed
exercise_version: 2
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-09-06
---

# 13. 评分与反馈

## 总评

结合 E05 复核，E01–E04 当前成绩为 8 / 8（100%），`mastered`，第 13 章完成。具体参数替换、GAT 使用前提、AliasWellFormed 与 expected-term 隔离均已复核。

每题 2 分，每小问 0.5 分；包含两个判断点或部分替换正确的小问可计 0.25 分。原答保存在 `exercises.md`。第 2 版保留 E01–E04 原题，新增 E05；E05 不另加总分，用于更新对应小问的当前评分。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 参数分层已掌握 | 各小问 0.5；E05.1 复核本次 own args 为 ['x, bool, 4]。 |
| E02 | 2 | 2 | rebase、RHS 与使用条件已掌握 | 各小问 0.5；E05.1/3 复核 RHS 中 P = u32 和 AliasWellFormed。 |
| E03 | 2 | 2 | 前提与输出保证已掌握 | 各小问 0.5；E05.2 复核使用前提及 Cell<T>: 'a → T: 'a → 引用良构。 |
| E04 | 2 | 2 | normalization 边界已掌握 | 各小问 0.5；E05.4 复核隔离 expected term 对候选选择的影响。 |

### E01. own 参数声明与本次 own args

1. 完整 args 为 `[Wrap<u16>, u32, 'x, bool, 4]`。
2. trait-ref 为 `Wrap<u16>: Family<u32>`。
3. 本次 own args 是 `['x, bool, 4]`；`['a, Q, N]` 是声明端的参数名称。结合 E05.1 与原答对 `trait_ref()` 不保留 own args 的判断，本问计 0.5。
4. projection 关联项标识指向 trait 的 Out 声明；实际提供 RHS 的 impl Out 定义在 candidate 求值时另外找到。

源码：`compiler/rustc_type_ir/src/ty_kind.rs::AliasTy::trait_ref_and_own_args` 和 `trait_ref`。这里拆的是当前 projection 已携带的 args，而不是重新取出泛型声明。

### E02. RHS 中每个位置使用自己的参数槽

1. impl parent args 为 `[u16, u32]`。
2. impl 关联项完整 args 为 `[u16, u32, 'x, bool, 4]`。
3. RHS 为 `(&'x u16, &'x u32, [bool; 4])`。第二项来自 `&'a P`，其中 P = u32。Q = bool 只用于 `[Q; N]`。E05.1 已复核参数对应，本问计 0.5；其中 `[bool, 4]` 按数组记号笔误理解，规范的 Rust 数组类型写法是 `[bool; 4]`，原答保留原样。
4. 两条 outlives 条件是 `Wrap<u16>: 'x` 和 `u32: 'x`；E05.3 已明确登记它们时使用 `AliasWellFormed`，本问计 0.5。

逐项替换关系：

```text
T → u16，P → u32，'a → 'x，Q → bool，N → 4
&'a T → &'x u16
&'a P → &'x u32
[Q; N] → [bool; 4]
```

`item_bounds` 是输出性质的查询概念；本题检查的是 GAT 声明的 own predicates，即使用该 alias 所需的条件。源码 `compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs::consider_impl_candidate` 中对应：

```rust,ignore
ecx.add_goals(
    GoalSource::AliasWellFormed,
    cx.own_predicates_of(alias_def_id.into())
        .iter_instantiated(cx, goal.predicate.alias.args)
        .map(Unnormalized::skip_norm_wip)
        .map(|pred| goal.with(cx, pred)),
)?;
```

### E03. 使用前提、输出保证、具体定义

1. `where Self: 'a` 是使用 GAT 的前提；E05.2 已确认，本问计 0.5。具体类型由 impl 中的 `type Item<'a> = ...` 给出，或由环境中的 projection equality 约束。
2. `: Clone + 'a` 约束 Item 投影，原答正确。
3. 返回类型是本次 lifetime 实例化的 `L::Item<'a>`，trait 声明保证它实现 Clone，因此即使 RHS 抽象也可以调用 clone。原答包含关键的 Clone 保证，计满分；这里无需知道它一定是引用。
4. 不是要求 `'static`。E05.2 补全了 `Cell<T>: 'a → T: 'a → &'a T 良构`，本问计 0.5。书写时可把 `WF(&'a T)` 与类型 outlives 断言 `&'a T: 'a` 分开；本题所问的是前一条良构性链。

可用三行区分职责：

```text
where Self: 'a       → 允许使用这份 GAT 实例的前提
: Clone + 'a        → 合法 Item 实例的输出保证
type Item<'a> = ... → impl 提供的具体定义
```

源码：`compiler/rustc_hir_analysis/src/check/wfcheck.rs::check_gat_where_clauses`；`compiler/rustc_hir_analysis/src/collect/item_bounds.rs::associated_type_bounds`；讲义正文第 3–4 节。

### E04. 独立输出变量是在隔离 expected term

1. `Trait(T: Iterator)` 与 `Projection(<T as Iterator>::Item == u32)`；第二条直接约束输出。原答正确。
2. 仅有 T: Iterator 不能推出 Item = u32，投影可以保持抽象。原答正确。
3. 核心目的是让 normalization 的候选选择不受外部 expected term 影响。E05.4 已确认，本问计 0.5；即使已经知道候选 impl，也仍需要保持这种输入/输出边界。原答中的 expected item 在此按题设的 expected term 理解。
4. 非单射判断正确：不同 X 都可映射到 u8，因此只凭这个输出无法唯一反推出 X。

源码：`compiler/rustc_next_trait_solver/src/solve/project_goals/mod.rs::normalize_associated_term`。概念流程为：

```text
原要求：Projection(Alias == Expected)
内部：  NormalizesTo(Alias, ?FreshOutput)
随后：  将归一化结果与 Expected 做 relation
```

例如 Alias 的合法归一化结果是 u8，而 Expected 是 u32，后续 relation 应处理这种不一致；不能为了迎合 Expected 而改选一个把同一 alias 展开成 u32 的途径。独立输出变量确保第一次求内部 NormalizesTo 时，Expected 不先约束它。返回的 nested goals 及后续 relation 仍可以参与推理，并不意味着完全禁止任何后续推理反馈。

## 已掌握概念

- projection 完整 args、trait-ref 与 trait 关联项标识。
- impl parent args 与 rebase 后的完整 args。
- 实例化 GAT 的两条 outlives 条件。
- Item 投影上的 Clone 保证及 generic body 的使用。
- 环境中的 trait/projection clause、抽象投影和 GAT 非单射性。
- 具体 own args、RHS 参数逐项替换与 AliasWellFormed。
- GAT 使用前提与输出保证、引用良构的 outlives 推导。
- 独立 normalization 输出变量与 expected term 隔离。

## 后续复核重点

第 14 章复用 item bounds 与 normalization 的分工；第 17 章继续研究 GAT 与高阶 binder；第 18–19 章连接具体 region inference 与引用良构检查。

## 补充练习或复习动作

本章 E05 已完成，分别用于更新 E01.3、E02.3–4、E03.1/4、E04.3，总分仍为 8。第 12 章 E05 继续保留在原章节，本次不改变第 12 章成绩。

## 完成判定

当前 8 / 8，状态 `completed`，`mastery: mastered`。本章关键概念已通过 E01–E05 复核；课程后续断点以 `STATE.md` 为准。

## 复核记录

2026-09-06：E05 四项概念复核通过，对应六个原题小问均更新为 0.5 分，E01–E04 各 2 分，总成绩 8/8。原始回答均保留；数组类型统一用 `[bool; 4]` 作为讲评记号。
