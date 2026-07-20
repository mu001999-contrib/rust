---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "19"
document: grading
status: completed
exercise_version: 2
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-09-25
---

# 19. 评分与反馈

## 总评

E05 第二次复核通过后，E02.2 按修正答案更新为满分；E01–E04 当前成绩为 7.5/8（93.75%），判定 `mastered`。借用数据流与访问冲突的主线准确，MIR typeck 约束产物、NLL 求解结果和 universe 来源信息也已区分清楚。

## 待复核小问（未满分）

| 小问 | 状态 | 当前分 / 满分 | 需要补齐 | 复核位置 |
|---|---|---:|---|---|
| E02.4 | 部分通过 | 0.25 / 0.5 | 说明 `check_user_type_annotations` 仍检查用户注解，并把 `'static` 要求重新转成约束。 | 本文件「E02 讲评」 |
| E03.2 | 部分通过 | 0.25 / 0.5 | 明确 `kill_loans_out_of_scope_at_location` 读取预计算的 `borrows_out_of_scope_at_location`，移除对应 BorrowIndex。 | 本文件「E03 讲评」 |

## 分题评分

每题 2 分，每小问 0.5 分；部分正确的小问记 0.25 分。E05 是原题定向复核，不另计入 8 分。

| 题号 | 第 1 问 | 第 2 问 | 第 3 问 | 第 4 问 | 合计 |
|---|---:|---:|---:|---:|---:|
| E01 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E02 | 0.5 | 0.5 | 0.5 | 0.25 | 1.75 |
| E03 | 0.5 | 0.25 | 0.5 | 0.5 | 1.75 |
| E04 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 已掌握 | 两段代码的类型不变；region 解决定借用在写入位置是否仍活跃。 |
| E02 | 1.75 | 2 | 已掌握主线 | E05 第二次复核确认了约束收集与 region 求解分工；用户注解的具体检查入口仍可后续复查。 |
| E03 | 1.75 | 2 | 基本掌握 | BorrowData 字段、`first_non_contained_inclusive` 和冲突维度正确；补上失活预计算表的名称与作用。 |
| E04 | 2 | 2 | 已掌握 | member constraint、临时 region 图、Polonius loan liveness 与诊断来源均正确。 |

### E01 讲评

四小问正确。`assigned_place` 是保存引用值的 MIR Place（本例通常是 `r`），不是创建借用的 `reserve_location`。A 中 region 不再覆盖写入位置时，借用数据流移除对应活跃位；B 中写入与仍活跃的共享借用 `&n` 冲突。仅看 `BorrowSet` 是否记录 `&n` 不足以判断。

### E02 讲评

1. 返回 `&'a u32` 作为 `&'b u32` 需要 `'a: 'b`，进入 `outlives_constraints`，正确。
2. `type_check::type_check` 负责收集/检查 MIR 关系，返回 `MirTypeckRegionConstraints` 和已知的 `universal_region_relations`；它尚未传播出 region 最终解。后续 `nll::compute_regions` 建立 region inference 上下文并调用 `UnsolvedRegionInferenceContext::solve`；后者运行 `propagate_constraints`、`check_type_tests`、`check_universal_regions`（legacy Polonius 路径另有 subset 检查），产出 `regioncx`、可能向外传播的 closure requirements 与错误。`universe_causes` 记录新 universe 的来源，帮助诊断，不是最终 region 值。
3. `T: 'r` 进入 `type_tests`，在 `solve` 传播后由 `check_type_tests` 核验，正确。
4. “`'static` 要求仍须成立”方向正确，但还要说明来源：`check_user_type_annotations` 会检查用户写下的类型注解，在 MIR region 重新编号后的表示中重建相应约束，不能因重编号丢失该要求。

### E03 讲评

1. `borrowed_place = n`，`assigned_place = r`，`region` 是这笔借用对应的 MIR `RegionVid`，正确。
2. `BorrowSet` 是全体借用的目录，常规路径的 `borrows_in_scope` 从当前数据流状态 `state.borrows` 取用于冲突检查的活跃位，这一层区分正确。失活时 `kill_loans_out_of_scope_at_location` 查预计算表 `borrows_out_of_scope_at_location[location]`，对列出的 BorrowIndex 执行 `kill_all`；“根据终止借用”尚未说清这个输入。
3. `regioncx.first_non_contained_inclusive` 找到 region 首次不包含的 MIR 位置，再登记到上述预计算表，正确。
4. 还需比较访问 Place 与借用 Place 是否重叠，以及读/写/保留/激活的访问种类与借用种类是否冲突；回答覆盖了主要维度。

### E04 讲评

四小问正确。`'m ∈ {'a, 'b}` 是 opaque hidden type 的候选 region 归属；当前 `compute_definition_site_hidden_types` 使用临时 `RegionCtxt`，不直接改写常规 `compute_regions` 的 region 图。`-Zpolonius=next` 的另一条路径计算 loan liveness，未开启并不代表 borrowck 停止运行。`span` 与 `from_closure` 是约束归因/诊断信息，不是额外的 outlives 求解前提。

### E05 首次复核讲评

| 小问 | 结论 | 具体反馈 |
|---|---|---|
| E05.1 | 部分通过 | “收集/检查 MIR 关系”正确；但 `type_check` 返回待求解的 `constraints` 与已知的 `universal_region_relations`，尚未求出最终 `Values(R)`。这里构造已知 universal 关系，不等于传播本次 MIR outlives 约束。 |
| E05.2 | 部分通过 | `compute_regions` 是正确入口；其中创建 `UnsolvedRegionInferenceContext` 并调用其 `solve`，由 `solve` 依次执行 `propagate_constraints`、`check_type_tests` 和常规路径的 `check_universal_regions`。回答给出了两个检查名，但尚缺统领它们的 `solve` 与传播步骤。 |
| E05.3 | 部分通过 | `regioncx` 是主要产物之一，正确；`universe_causes` 则在 MIR typeck 的 `MirTypeckRegionConstraints` 中记录 universe 的来源，供后续诊断使用，不是 region 解。 |

按当前源码，最短流程是：

```text
type_check::type_check
    -> MirTypeckResults { constraints, universal_region_relations, ... }
    -> nll::compute_regions
    -> UnsolvedRegionInferenceContext::solve
         propagate_constraints -> check_type_tests -> check_universal_regions
    -> NllOutput { regioncx, nll_errors, ... }
```

其中 `universal_region_relations` 是已知前提关系，`universe_causes` 是 universe 来源记录，`regioncx` 才承载传播后的 region 解；三个名称不指同一结果。源码定位：`compiler/rustc_borrowck/src/type_check/mod.rs::type_check`、`compiler/rustc_borrowck/src/nll.rs::compute_regions`、`compiler/rustc_borrowck/src/region_infer/region_context.rs::UnsolvedRegionInferenceContext::solve`。

### E05 第二次复核讲评

三项均通过。E05.1 明确 `type_check` 返回待求解的 `constraints` 和已知的 `universal_region_relations`，而非最终 `Values(R)`；E05.2 指出 `compute_regions` 调用 `solve`，检查 type tests 与 universal 边界；E05.3 正确区分 `regioncx` 是求解产物、`universe_causes` 是 universe 来源信息。按复核规则，E02.2 从 0/0.5 更新为 0.5/0.5；首次与第二次原答均保留在 `exercises.md`。

## 已掌握概念

- `BorrowSet` 保存借用身份，当前位置的 `borrows_in_scope` 才供冲突检查筛选。
- `first_non_contained_inclusive` 将 region 解连接到借用数据流的失活位置。
- `TypeTest`、opaque member constraint 与普通 region outlives 边的用途不同。
- 冲突判断还要综合 Place 与访问/借用种类；Polonius next 是可选 loan-liveness 路径。
- MIR typeck 产出待求解的 region 约束与已知 universal 关系；`compute_regions`/`solve` 求出 region 解，`universe_causes` 记录来源。

## 后续复核重点

E02.4 的用户注解检查入口 `check_user_type_annotations`、E03.2 的借用失活预计算表 `borrows_out_of_scope_at_location` 可在后续源码跟踪时复查。

## 补充练习或复习动作

[E05 定向复核](exercises.md) 已通过，不另计分，本章无必做补充题。E02.4、E03.2 的补充解释已在上方，后续源码跟踪时再复查即可。

## 完成判定

E05 第二次复核确认本章关键阶段分工；当前 7.5/8（93.75%），达到掌握标准，判定第 19 章 `completed`。E02.4 与 E03.2 的两个源码细节保留在待复核清单，不妨碍进入第 20 章。

## 复核记录

2026-09-25：发布第 19 章讲义和 E01–E04。

2026-09-25：原样记录 E01–E04 答案；按当前 rustc 源码评分 7/8，发布 E05 定向复核。

2026-09-25：原样记录 E05 首次复核答案；确认 `compute_regions` 入口，仍需区分 `type_check` 的约束产物、`solve` 的 region 解和 `universe_causes` 的来源信息；原题成绩保持 7/8。

2026-09-25：原样记录 E05 第二次复核答案，三项通过；E02.2 更新为满分，总成绩更新为 7.5/8，判定 `mastered`。
