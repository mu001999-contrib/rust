---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "04"
document: grading
status: graded
exercise_version: 1
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-07-26
---

# 04. 评分与反馈

## 总评

得分 `7.5 / 8`（93.75%）。region variants、universe nameability、liberate 与 `enter_forall` 的边界均已掌握；placeholder escape 的量词顺序已通过讲评补齐。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 正确 | 六个阶段依次对应 `ReEarlyParam`、`ReBound`、`ReLateParam`、`ReVar`、`RePlaceholder`、`ReErased`。 |
| E02 | 2 | 2 | 正确 | 三个 inference vars 的可命名范围、两次 `can_name` 判断和 universe/outlives 边界均正确。 |
| E03 | 2 | 2 | 正确 | 两次转换及其身份字段正确，也准确指出 `RePlaceholder` 表示任意选择的 `forall` 代表，而 `ReLateParam` 是 body 内稳定引用的参数。 |
| E04 | 1.5 | 2 | 核心判断正确 | U0 不能命名 U1、U1 中新变量可以命名 P1；量词顺序的解释由讲评补齐。 |

## 逐题解释

### E01：RegionKind 的阶段转换

答案顺序完全正确：

```text
item GenericArgs        ReEarlyParam
未打开的 Binder         ReBound
liberate 到函数体       ReLateParam
等待 region inference   ReVar
enter_forall             RePlaceholder
identity 已擦除         ReErased
```

原答中的 `RePlaceHolder` 对应源码拼写 `RePlaceholder`，语义判断一致。

### E02：Universe 只控制 nameability

可命名矩阵为：

```text
?r0@U0 -> P1: 否，P2: 否
?r1@U1 -> P1: 是，P2: 否
?r2@U2 -> P1: 是，P2: 是
```

并且：

```text
U1.can_name(U0) = true
U1.can_name(U2) = false
```

Universe 层级不表达 region 长短或 outlives 顺序。

### E03：Body parameter 与 forall placeholder

进入函数自身的 body：

```text
ReBound(D0, 'a)
  -> ReLateParam { scope: id, kind: Named('a) }
```

临时打开 higher-ranked binder：

```text
ReBound(D0, 'b)
  -> RePlaceholder { universe: U1, bound: slot/'b identity }
```

前者属于 body 的 free-parameter 环境；后者是 fresh universe 中 arbitrary-but-fixed 的刚性名字。原答对二者用途的区分准确。

### E04：Placeholder escape 的量词含义

约束背后的量词顺序是：

```text
exists ?r0@U0, forall 'a@U1: ?r0 = 'a
```

`?r0` 位于 `forall<'a>` 外面，必须在 `'a` 被任意选择之前就固定。不存在一个固定的 outer region 能同时等于所有可能的 `'a`。

若允许：

```text
?r0@U0 := P1@U1
```

就等于让较早的 existential variable 依赖较晚的 universal variable，把原问题偷偷改成“对每个 `'a`，另选一个等于它的 `?r0('a)`”。这正是 placeholder escape；`U0.cannot_name(U1)` 在实现中阻止这种非法依赖。

在 U1 中新建的 `?r1` 位于 placeholder 可见的作用域内，因此单从 nameability 看，它可以引用 P1；最终能否作为某个关系的解还要满足该关系的其余约束。

## 已掌握概念

- `RegionKind` 主要 variants 与各编译阶段的对应关系。
- early-bound `ReEarlyParam`、binder 内 `ReBound` 与 body 内 `ReLateParam` 的转换。
- `enter_forall` 把 `ReBound` 换成新 universe 的刚性 `RePlaceholder`。
- `UniverseIndex::can_name` 的方向，以及它与 outlives 长短的独立性。
- inference variable 的创建 universe 限制其可采用的 placeholder。
- outer existential 不能依赖 inner universal 的量词顺序。

## 后续复核重点

第 17 章复核 higher-ranked goals 时，继续使用 `exists outer, forall inner` 的量词顺序判断 placeholder 是否能够逃逸。

## 补充练习或复习动作

进入第 05 章后，把本章的 `ReVar(RegionVid @ Universe)` 接入 `InferCtxt`、unification table 与 snapshot 模型。

## 完成判定

- 总分达到掌握阈值，四类核心 region 表示边界清晰。
- 能准确手算 universe nameability。
- 能区分 liberate 与 `enter_forall`。
- placeholder escape 的实现规则和量词原因已经连通。

结论：第 04 章 `completed`，`mastery: mastered`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-07-26 | 完成 E01–E04 综合评估 | `7.5 / 8`，`mastered`，章节完成。 |
