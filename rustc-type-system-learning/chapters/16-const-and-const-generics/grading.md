---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "16"
document: grading
status: completed
exercise_version: 3
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-09-25
---

# 16. 评分与反馈

## 总评

E05、E06 已复核并作为原小问的修正答案计分，当前 E01–E04 为 8/8（100%）。类型级 const 的 HIR/Type IR 阶段、GCE/GCA 路径与 next solver rigid-alias 回退均已确认；E05、E06 不独立增加总分。

## 待复核小问（未满分）

无。E06 三项均已补齐；答案中的 `Anno` 按 `Anon` 的笔误理解，原文保留于 `exercises.md`。

## 分题评分

每小问满分 0.5；包含多个判断点的小问可计 0.25。当前逐小问得分：

| 题号 | 第 1 问 | 第 2 问 | 第 3 问 | 第 4 问 | 合计 |
|---|---:|---:|---:|---:|---:|
| E01 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E02 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E03 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E04 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 已掌握 | GenericArgs、Param/Infer 与 ValTree 分工明确。 |
| E02 | 2 | 2 | 已掌握 | E06 明确区分 HIR `AnonConst`、Type IR `Alias(Anon)`、`Value`，以及两条关系分支。 |
| E03 | 2 | 2 | 已掌握 | 复答准确区分 GCE 可求值前提与 GCA 具名 alias 的延迟求值。 |
| E04 | 2 | 2 | 已掌握 | `Expr`、`TooGeneric` 与 `IsRigid::Yes` 回退均已确认。 |

### E05–E06. 定向复核与原题映射

| 复核题 | 对应原小问 | 结果 |
|---|---|---|
| E05.1、E06.1 | E02.3 | E06 已给出匿名常量的 HIR `AnonConst` → Type IR `Alias(Anon)` → `Value` 三阶段，更新为 0.5。 |
| E05.2、E06.2 | E02.4 | E06 已区分 next solver 的 `ProjectionClause` 与旧 GCE 的 `ConstEquate`，更新为 0.5。 |
| E05.3 | E03.3 | 可求值前提与 lowering 求值已区分，原小问更新为 0.5。 |
| E05.4、E06.3 | E04.2、E04.3 | E05 给出两种 certainty 和 `Expr` 的 GCE 归属；E06 补出原始 alias 的 `IsRigid::Yes`，两小问均为 0.5。 |
| E05.5 | E04.4 | `TooGeneric` 变体已答出，原小问更新为 0.5。 |

### E01. Const 与泛型参数

1. `Buffer` 的 args 为 Type、Const；字段类型可写 `TyKind::Array(T, ConstKind::Param(N))`。概念判断正确，0.5。
2. 签名内是 `Param(N)`；`keep::<3>` 为该位置提供具体值，0.5。
3. 调用处推断使用 `ConstKind::Infer(InferConst::Var(_))`，而签名里的 N 是泛型参数，0.5。
4. `Value` 同时携带类型和 `ValTree`；`AllocId` 的分配身份不适合作为类型级值相等依据，0.5。

### E02. Alias 与 const relation

1. `<Fixed<3> as Capacity>::CAP` 对应 `AliasConstKind::Projection`。DefId 标识 trait 中的 CAP，args 包含本次 Self = `Fixed<3>`，用于匹配和实例化；原答抓住两项用途，0.5。
2. `ConstArgHasType(3_usize, bool)` 检查类型匹配，此处不满足，0.5。
3. 具体 `{ 1 + 2 }` 可先作为 HIR `ConstArgKind::Anon`，进入 Type IR 后待求值形态是 `ConstKind::Alias(AliasConstKind::Anon, args)`；求值成功后是 `Value(3_usize)`。E06 按这三个阶段复答，0.5。
4. next solver 对非 rigid const alias 注册 `ProjectionClause { projection_term: alias, term: other_const }`；`ConstEquate` 是旧 `generic_const_exprs` 分支的目标，并非 next solver 通用入口。E06 已正确区分，0.5。

### E03. GCA 的具名 const

1. 稳定 `[u8; N]` 使用独立 const 参数；`N + 1` 放入具名 ADD1 后，抽象 N 下的长度可保持 `Alias(Free(ADD1), [N])`，0.5。
2. DefId 指向 ADD1，args 携带 N；`gca!` 要求直接表示，避免该实参被包装成匿名 const。原答正确；在有具体实参时仍可能随后求出 `Value`，0.5。
3. GCA 的具名 alias 可以在抽象 N 下保持未求值，因此不需 GCE 的 `where [(); N + 1]:`。该 where 条件提供调用方需满足的可求值前提，**不是** lowering 时已经求出数值；E05 复答已明确此点，更新为 0.5。
4. 同一 ADD1 定义和同一实参可按定义身份建立关系；不同定义即使右侧同为 `N + 1`，也不能在抽象 N 下直接推出相等，0.5。

### E04. 求值与支持边界

1. 类型、可求值性和相等性三个目标区分正确，0.5。
2. 仍有未解非 region 推理变量时返回 `Certainty::AMBIGUOUS`；没有此类变量时，GCA 分支与原始 `IsRigid::Yes` alias 建立关系，若关系和新增目标成立则返回 `Certainty::Yes`。E05、E06 合并复答完整，0.5。
3. `ConstKind::Expr` 属于 `generic_const_exprs` 的非 item 表达式路径；GCA 使用具名 const item，含泛型参数的匿名 `{ N + 1 }` 不是这一主机制。E05 复答正确区分，更新为 0.5。
4. 入口是 `const_eval_resolve`；仍太泛化的具体返回是 `Err(ErrorHandled::TooGeneric(...))`。E05 复答给出 `TooGeneric` 变体，更新为 0.5。

## 已掌握概念

- const 参数、推理变量、数组长度与泛型实参的 IR 层次。
- 具名 const item 的 DefId/args、直接表示与 GCA 定义相等。
- 常量类型、可求值性、相等性三类问题的区分。
- GCE 可求值前提与 GCA 抽象 alias 的不同检查时机；`Expr` 与 `TooGeneric` 的归属。
- 匿名 const 的 HIR/Type IR/求值三阶段、`ProjectionClause` 与 `ConstEquate` 的分支，以及 `IsRigid::Yes` 回退。

## 后续复核重点

无；后续章节可在实际源码例子中继续运用 const alias、WF 与求值时机的区分。

## 补充练习或复习动作

第 16 章无需补充练习；第 12 章 E05 保留于原章节。

## 完成判定

当前 8/8，`mastered`，第 16 章 `completed`；下一章为第 17 章。

## 复核记录

2026-09-25：记录 E01–E04 原答并按当前源码评阅，成绩 6/8；发布 E05 定向复核，原答保留于 `exercises.md`。

2026-09-25：E05 原答已记录；E02.4、E03.3、E04.3、E04.4 更新得分，总成绩 7.25/8；发布 E06 三项简短复核。

2026-09-25：E06 三项通过，E02.3、E02.4、E04.2 更新为满分，总成绩 8/8，判定 `mastered`。
