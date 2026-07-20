---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "01"
document: grading
status: graded
exercise_version: 2
earned_points: 26
max_points: 31
mastery: mastered
updated_at: 2026-07-21
---

# 01. 评分与反馈

## 总评

完整得分 `26 / 31`（约 83.9%），达到本课程通常的 80% 掌握阈值。学习者已经能够识别 Type IR 核心节点、区分表示相等与类型关系，并理解 immutable interned IR、缓存摘要与外部 inference state 的边界。

通过作答和主动追问，学习者已经形成阶段边界与变量转换的清晰模型；第 02、04、10 章将继续深化相关内容。因此本章判定为 `mastered`，总状态为 `completed`。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 8 | 8 | 正确 | `Matrix` 字段与实例化表示正确；三组比较正确区分了统一、归一化与表示相等；修订答案已指出 HIR 中保留歧义、HIR type lowering 读取 generic param kind、后续 inference 求具体值。 |
| E02 | 6.5 | 9 | 核心已掌握，后续深化 | `Param`、`ReBound` 和四个判断的结论正确；完整模型还包括调用点先实例化、`enter_forall` 产生 `RePlaceholder`、canonical bound variable，以及位于 `InferCtxt` 的 inference mapping。 |
| E03 | 3.5 | 6 | 核心 flags 已掌握 | 六项的直接 flag 判断均抓住核心；完整集合还包括 B–F 的 `HAS_FREE_LOCAL_NAMES`，以及 F 的 binder 元数据和内部 `T` parameter flags。 |
| E04 | 4 | 4 | 正确 | 正确判断 1、3 可立即返回，并以“无 infer vars”“无 alias”解释；也正确排除了 2、4。 |
| E05 | 4 | 4 | 正确 | binder 内无 escaping vars 且 exclusive binder 为 0；`skip_binder` 后存在 escaping var 且为 1。 |

E02 的独立判断点明细：

| 判断点 | 得分 | 满分 | 说明 |
|---|---:|---:|---|
| 泛型函数体中的 `T` | 1 | 1 | 正确：`Param`。 |
| 调用点的参数与实参 | 0.5 | 1 | 定义端为 `Param`、实参涉及 infer var；调用点的完整流程是先把 formal `Param(T)` 实例化成 fresh infer var。 |
| 未打开 binder 的 `'a` | 1 | 1 | 正确：`ReBound`。 |
| `enter_forall` 后的 `'a` | 0 | 1 | 精确表示为新 universe 中的 `RePlaceholder`；`ReLateParam` 对应 liberate 后的函数体环境。 |
| canonicalization 前后 | 0.5 | 1 | 前半 `Infer` 正确；后半由 `BoundVarIndexKind::Canonical` 管理 canonical bound variable。 |
| `Param` 能否直接统一 | 1 | 1 | 判断正确：必须先 substitution/instantiation。 |
| placeholder 能否赋值 | 1 | 1 | 判断正确；精确表述是“任意但固定、刚性”，而不是“所有值本身”。 |
| inference state 的位置 | 0.5 | 1 | interned `Ty` 保持不可变；映射位于 `InferCtxt` 的 unification tables，临时 `TypeckResults` 保存含 infer ID 的 Type IR。 |
| `Bound` 能否脱离 binder | 1 | 1 | 正确：否则成为 escaping bound variable。 |

E03 的独立判断点明细：

| 类型 | 得分 | 满分 | 说明 |
|---|---:|---:|---|
| `Vec<u32>` | 1 | 1 | 正确：在题目列出的主要 flags 中没有命中项。 |
| `Vec<T>` | 0.5 | 1 | `HAS_PARAM` 正确；完整集合还包含派生的 `HAS_FREE_LOCAL_NAMES`。 |
| `Vec<?0t>` | 0.5 | 1 | `HAS_INFER` 正确；完整集合还包含 `HAS_FREE_LOCAL_NAMES`。 |
| `Vec<<T as Iterator>::Item>` | 0.5 | 1 | `HAS_PARAM`、`HAS_ALIAS` 正确；完整集合还包含 `HAS_FREE_LOCAL_NAMES`。 |
| `&'?0r Vec<?1t>` | 0.5 | 1 | `HAS_INFER`、`HAS_FREE_REGIONS` 正确；完整集合还包含 `HAS_FREE_LOCAL_NAMES`。 |
| `Binder<for<'a> fn(&'a T)>` | 0.5 | 1 | `HAS_BOUND_VARS` 正确；完整集合还包含 `HAS_BINDER_VARS`、内部 `T` 的 `HAS_PARAM` 与 `HAS_FREE_LOCAL_NAMES`。 |

E04 的四个判断均正确：

```text
resolve_vars_if_possible(Vec<T>)                 → 可返回；没有 Infer
resolve_vars_if_possible(Vec<?0t>)               → 不可返回；需要 resolve
normalize(Vec<u32>)                              → 可返回；没有 Alias
normalize(Vec<<T as Iterator>::Item>)            → 不可返回；存在 projection alias
```

E05 同时正确区分了两个概念：完整 `Binder` 可以包含 bound-variable flags，但它已经捕获 `'a`，所以没有 escaping bound vars；`skip_binder()` 后 binder 边界消失，内部 `'a` 变成 escaping。

## 已掌握概念

- HIR 类型与 Type IR 类型的职责边界。
- `Ty`、`Const`、`Region` 的 interned、不可变表示。
- `GenericArg` 对 lifetime/type/const 实参的统一承载。
- `Param`、`Infer`、`Bound`、`Placeholder`、`Alias` 的基本语义分类。
- Type IR 表示相等不等于可统一或归一化后相等。
- generic argument `_` 在 HIR 中的歧义表示，以及通过 `GenericParamDefKind` lower 成 type/const infer var 的过程。
- `ItemCtxt` 与 `FnCtxt` 对 inference 的不同能力。
- inference 通过外部 `InferCtxt` 更新状态，resolve/writeback 构造新结果，不修改原 interned `Ty`。
- `TypeFlags` 是当前 immutable IR 在 intern 时计算的递归摘要，可支持 resolve/normalize 快速路径。
- `HAS_BOUND_VARS` 与 `has_escaping_bound_vars()` 回答不同问题。
- `outer_exclusive_binder` 能快速表达当前值需要外部多少层 binder 才能捕获其 bound variables。

## 后续复核重点

以下内容在对应后续章节再次验证：

1. 调用泛型函数时，定义端 `Param(T)` 先经 substitution 变成调用端的 fresh inference variable，之后才进行 unification。
2. `enter_forall` 使用 `RePlaceholder`；`ReLateParam` 用于把函数签名中的 late-bound region liberate 到函数体环境。
3. Canonicalization 将 inference variable 改写成由 `Canonical<V>` 管理的 canonical bound variable。
4. `TypeckResults` 可以暂时引用含 infer ID 的 Type IR，但 `?0t → u32` 的可变映射属于 `InferCtxt`。
5. 判断完整 flags 时，同时考虑直接 variant flag、`HAS_FREE_LOCAL_NAMES` 等组合 flag，以及 `Binder` 自身的 bound-variable metadata。

## 补充练习或复习动作

后续按课程路线自然复核：

- 第 02 章：普通 `Binder`、de Bruijn index、escaping bound vars。
- 第 04 章：`ReLateParam`、placeholder、universe 与 leak check。
- 第 10 章：canonical input/response、`CanonicalVarValues` 与响应实例化。

## 完成判定

- 分数达到 80% 以上。
- 已建立 Type IR 基础表示、interning、相等性边界和 lowering 主线的清晰模型。
- 能正确使用 flags 快速路径，并区分 bound-variable flags 与 escaping 状态。
- 讲评内容已形成正确模型，并纳入后续章节继续深化。

结论：第 01 章 `completed`，`mastery: mastered`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-07-21 | 完成 E01–E05 的综合评估 | `26 / 31`（83.9%），`mastered`，章节完成。 |
