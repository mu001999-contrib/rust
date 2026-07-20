---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "05"
document: grading
status: completed
exercise_version: 1
earned_points: 7.5
max_points: 8
mastery: mastered
updated_at: 2026-07-27
---

# 05. 评分与反馈

## 总评

修正后得分 `7.5 / 8`（93.75%）。E03 的 snapshot 答案经定向复核修正后计为满分；推理变量表示、union-find 合并、universe 收紧、occurs check、snapshot rollback，以及 shallow/opportunistic/full resolution 的边界均已掌握。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 1.5 | 2 | 基本正确 | 三类 IR 表示、ID、U0 与 `FreshTy` 用途正确；可变状态的回答停留在共同容器 `InferCtxtInner`，还需区分三种具体存储。 |
| E02 | 2 | 2 | 正确 | 两个 unknown class 合并后取 U0，随后共同解析为 `u32`；循环实例化由 generalization 中的 occurs check 阻止。 |
| E03 | 2 | 2 | 修正后正确 | 第一轮已正确判断闭包内观察与 `commit_if_ok`；定向复核进一步正确给出 `probe` 退出后旧 binding、新变量和 current universe 的回滚状态。 |
| E04 | 2 | 2 | 正确 | 整体 tuple 的 shallow resolve、深层 opportunistic resolve，以及 type/region full-resolution 前置条件判断正确。 |

## 逐题解释

### E01：IR handle 与具体状态存储

返回表示正确：

```text
type   -> Ty::Infer(InferTy::TyVar(TyVid))
const  -> ConstKind::Infer(InferConst::Var(ConstVid))
region -> RegionKind::ReVar(RegionVid)
```

三者都由 `InferCtxtInner` 持有，但具体落点分别是：

```text
TyVid
  -> TypeVariableStorage::eq_relations
  -> Unknown(U0) / Known(Ty)

ConstVid
  -> const_unification_storage
  -> Unknown { universe: U0, origin } / Known(Const)

RegionVid
  -> region_constraint_storage
  -> var info { universe: U0, origin } + region constraints
```

`FreshTy(0)` 的判断正确：它是 `TypeFreshener` 生成的轻量缓存占位符，没有 live table entry。

### E02：统一与 occurs check

答案完整正确：

```text
?T0@U1 == ?T1@U0
  -> class { ?T0, ?T1 } = Unknown(U0)

?T1 == u32
  -> class { ?T0, ?T1 } = Known(u32)

resolve(?T0) = resolve(?T1) = u32
```

`?T2 == Vec<?T2>` 不能直接写成 `Known(Vec<?T2>)`。`instantiate_ty_var` 经过 generalization traversal 执行 occurs check，维护 Type IR 的无循环展开不变量。

### E03：`probe` 返回值与推理状态是两件事

闭包内部的观察应为：

```text
?T0 -> Known(u32)
?T1 -> Unknown(U0)
current universe = U1
```

但 `probe` 的实现顺序是：

```text
start_snapshot
  -> 执行闭包并保存返回值
  -> rollback_to(snapshot)
  -> 返回闭包计算出的值
```

因此退出后的状态为：

```text
?T0 -> Unknown(U0)       // Known(u32) binding 被撤销
?T1 -> 不再存在          // snapshot 内新建的 table entry 被截去
current universe = U0    // 恢复 snapshot 保存的 universe
```

`probe` 即使返回 `true`，也只保留这个返回值，不保留闭包对 `InferCtxt` 的副作用。

`commit_if_ok` 的判断正确：

```text
Ok(_)  -> commit_from(snapshot)
Err(_) -> rollback_to(snapshot)
```

### E04：三种解析深度

对整个 tuple 调用 `shallow_resolve` 时，最外层是 tuple 而不是 `Infer`，结果保持：

```text
(Vec<?T0>, ?T1, &'?r0 u8)
```

`resolve_vars_if_possible` 深入 type/const 结构但不处理 region：

```text
(Vec<u32>, ?T1, &'?r0 u8)
```

`fully_resolve` 遍历到 `?T1` 时会返回 `FixupError`。若先解决 `?T1`，遍历到 `?r0` 时还要求 lexical region resolution 已经运行并写入 `lexical_region_resolutions`。一次实际调用通常在遇到首个失败点时停止，而不是同时返回两类错误。

## 已掌握概念

- Type/const/region inference handles 及其 universe。
- `eq_relations` 中 unknown class 的合并和 concrete instantiation。
- 不同 universe 合并时取共同可命名范围 U0。
- occurs check 阻止循环推理类型。
- `shallow_resolve` 与 `resolve_vars_if_possible` 的遍历深度。
- `fully_resolve` 对 type var 和 lexical region resolution 的不同要求。

## 后续复核重点

已通过定向复核确认：`probe` 始终回滚推理副作用，已有变量恢复旧值、snapshot 内创建的变量被移除、current universe 恢复进入 snapshot 时的值。后续可在涉及 speculative relation checking 时继续巩固。

## 补充练习或复习动作

已完成以下 snapshot 复核题：

```text
初始：
  current universe = U0
  ?T0 -> Unknown(U0)

let observed = probe {
    ?T0 == u32
    创建 ?T1@U0
    创建并进入 U1
    返回 (resolve(?T0), ?T1 是否存在, current universe)
}
```

学习者回答：

```text
1. (u32, 存在, U1)
2. Unknown(U0)
3. 不存在
4. U0
```

四项全部正确：

```text
observed = (u32, true, U1)
probe 后 ?T0 -> Unknown(U0)
?T1 的 table entry 被回滚移除
current universe = U0
```

## 完成判定

- 修正后总分为 `7.5 / 8`（93.75%），达到完成标准。
- Snapshot 回滚这一核心概念已通过定向复核。
- 当前章节状态：`completed`；掌握度：`mastered`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-07-26 | 完成讲授、源码精读并发布 E01–E04 | 等待提交。 |
| 2026-07-27 | 评阅 E01–E04 | `6.5 / 8`；统一与解析已掌握，安排 snapshot 定向复核。 |
| 2026-07-27 | 评阅 snapshot 定向复核题 | 四项全部正确；E03 更新为 2/2，总成绩更新为 7.5/8，章节判定 `mastered`。 |
