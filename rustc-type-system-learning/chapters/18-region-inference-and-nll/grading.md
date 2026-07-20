---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "18"
document: grading
status: completed
exercise_version: 1
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-09-25
---

# 18. 评分与反馈

## 总评

E01–E04 综合评分为 7.5/8（93.75%），判定 `mastered`。NLL 位置下界、`sup: sub` 传播方向、SCC 合并和 universal 边界检查均已理解。E04 两个小问各有一个源码表述细节待补齐，不影响本章完成。

## 待复核小问（未满分）

| 小问 | 状态 | 当前分 / 满分 | 需要补齐 | 复核位置 |
|---|---|---:|---|---|
| E04.1 | 部分通过 | 0.25 / 0.5 | 写出 `UniversalRegions::new` 与 `renumber::renumber_mir`；前者收集签名里的 universal regions，范围不只显式手写名称。 | 本文件「E04 讲评」 |
| E04.4 | 部分通过 | 0.25 / 0.5 | 说明并集传播只满足值包含关系，不能证明 `T: 'r` 或传播出的 universal outlives 已有合法前提。 | 本文件「E04 讲评」 |

## 分题评分

每题 2 分，每小问 0.5 分；部分正确的小问记 0.25 分。

| 题号 | 第 1 问 | 第 2 问 | 第 3 问 | 第 4 问 | 合计 |
|---|---:|---:|---:|---:|---:|
| E01 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E02 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E03 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E04 | 0.25 | 0.5 | 0.5 | 0.25 | 1.5 |

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 已掌握 | MIR 活跃位置是下界；不机械等同于词法块或源码最后一次出现。 |
| E02 | 2 | 2 | 已掌握 | `'a: 'b` 的方向、传播后的标记及签名边界判断准确。 |
| E03 | 2 | 2 | 已掌握 | SCC 与 DAG 传播的集合计算全部正确。 |
| E04 | 1.5 | 2 | 部分细节待补齐 | 源码节点和传播方向准确；入口 API 名称及传播/合法性区别需更明确。 |

### E01 讲评

四小问均正确。`use` 与可能相关的 `drop` 都属于 MIR liveness 分析会考虑的来源；并不是每个 drop 都必然延长本例 `&u32` 的借用。`p2` 在题设简化条件下无须属于借用 region，`LivenessValues` 是必需位置的下界。

### E02 讲评

四小问均正确。返回 `&'a u32` 作为 `&'b u32` 需要 `'a: 'b`，对应 `sup = 'a`、`sub = 'b`，传播将 `Values('b)` 并入 `Values('a)`。这只是求解出的要求；还须在 `UniversalRegionRelations` 中有签名、where-clause 或 implied bound 支撑。加入 `where 'a: 'b` 是调用方须满足的契约。

### E03 讲评

四小问均正确。`R0` 和 `R1` 构成同一 SCC，`R2` 在下游；边为 `{R0,R1} → {R2}`。传播后前者共享 `{p0,p1,p2}`，后者保留 `{p2}`。压缩环后按 DAG 依赖顺序计算即可。

### E04 讲评

1. 主流程“先处理 universal 边界，再重新编号 MIR”已抓住，记 0.25。精确入口为 `UniversalRegions::new(infcx, def)` 和 `renumber::renumber_mir(infcx, body, promoted)`（见 `compiler/rustc_borrowck/src/nll.rs` 的 `replace_regions_in_mir`）。`UniversalRegions` 包括 `'static`、函数体特殊 region、签名中的具名及匿名 universal regions；不宜只叫“手写 regions 的映射”。
2. `Location`、`RootUniversalRegion`、`PlaceholderRegion` 三类正确。指南记号 `end('a)` 对应这里的 root universal 标记，0.5。
3. `merge_liveness` 把原有活跃位置并入该变量所属 SCC 的值；`add_region(A, B)` 把 B 的 points、universal 标记、placeholder 标记并入 A，0.5。
4. 两个检查函数及对象都正确，记 0.25。还需补上关键理由：并集传播总能把 `Values(sup)` 扩大以满足 `sup: sub`，但它不能自行证明 `T: 'r` 的 type test，也不能凭“传播出了 `'a: 'b`”就当作签名本已承诺该关系。`check_type_tests` 和 `check_universal_regions` 因此仍须单独执行（见 `UnsolvedRegionInferenceContext::solve`）。

## 已掌握概念

- MIR use/drop 等来源形成 region 活跃位置下界。
- `R1: R2` 表示 `Values(R1) ⊇ Values(R2)`，传播方向稳定。
- SCC 环压缩、DAG 顺序及集合值手算。
- 函数签名 universal outlives 必须有已知前提，不能由传播结果反向证明。

## 后续复核重点

第 19 章若再遇到 MIR typeck 入口，可顺带辨认 `UniversalRegions::new` / `renumber_mir`；遇到 `solve` 时再区分“传播要求”与“验证已知边界”。第 17 章仍独立保留为后续专项。

## 补充练习或复习动作

本章无必做补充题。E04.1、E04.4 的准确表述已列在上方；需要时可在第 19 章源码跟踪中复核。

## 完成判定

7.5/8（93.75%）且核心模型清晰，判定 `mastered`，第 18 章 `completed`。第 17 章未完成状态不因此改变。

## 复核记录

2026-09-25：发布第 18 章讲义和 E01–E04；等待学习者作答。

2026-09-25：原样记录 E01–E04 答案；按当前 rustc 源码评分 7.5/8，E04.1 和 E04.4 各为部分通过，判定本章已掌握。
