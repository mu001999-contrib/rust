---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "05"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-07-27
---

# 05. 习题

## 作答说明

本轮覆盖变量创建、union-find 统一、occurs check、snapshot 和解析层级。每题 2 分，共 8 分。

可以使用 `?T0@U0`、`Unknown(U0)`、`Known(u32)` 等简写。请同时写出 Type IR 表示与 `InferCtxt` 中的状态变化。

## 题目

### E01. 创建推理变量

当前 universe 是 U0，依次调用：

```text
next_ty_var_with_origin(...)
next_const_var_with_origin(...)
next_region_var(...)
```

回答：

1. 三个返回值分别使用什么 Type IR variant 和 ID？
2. 每个 ID 对应的可变状态主要存放在哪里，初始 universe 是多少？
3. `FreshTy(0)` 是否表示上述 `InferCtxt` 中另一个等待求解的 live type variable？说明用途。

### E02. 合并、实例化与 occurs check

初始状态：

```text
?T0@U1 -> Unknown(U1)
?T1@U0 -> Unknown(U0)
?T2@U0 -> Unknown(U0)
```

依次处理：

```text
?T0 == ?T1
?T1 == u32
?T2 == Vec<?T2>
```

回答：

1. 第一条关系后，`?T0` 与 `?T1` 的等价类和值是什么？合并后的 universe 是什么？
2. 第二条关系后，两个变量各自解析为什么？
3. 第三条关系能否写入 `Known(Vec<?T2>)`？哪个检查负责维护这里的不变量？

### E03. Snapshot

进入操作前：

```text
current universe = U0
?T0 -> Unknown(U0)
```

执行：

```text
probe {
    ?T0 == u32
    创建 ?T1
    创建并进入 U1
    返回 true
}
```

回答：

1. 闭包内部能观察到哪些 binding、变量和 universe？
2. `probe` 返回后，`?T0`、`?T1` 和 current universe 分别是什么状态？
3. 若把一组 binding 放入 `commit_if_ok`，闭包返回 `Ok` 与 `Err` 时分别发生什么？

### E04. 解析层级

已知：

```text
?T0 -> Known(u32)
?T1 -> Unknown(U0)
?r0 是尚待 region solving 的 ReVar
value = (Vec<?T0>, ?T1, &'?r0 u8)
```

回答：

1. 对整个 tuple type 调用 `shallow_resolve`，结果是什么？为什么？
2. 调用 `resolve_vars_if_possible`，结果是什么？
3. 此时调用 `fully_resolve` 会遇到哪些前置条件或失败点？分别说明 `?T1` 与 `?r0`。

## 学习者答案

### E01

> 练习一：1. InferTy::TyVar, ?T0@U0; InferConst::Var, ?C0@U0; RegionKind::ReVar, ?R0@U0; 2. InferCtxtInner, U0; 3. 不表示，用途是用于轻量缓存。

### E02

> 练习二：1. 等价类是 (?T0, ?T1)，值是 Unknown(U0)，合并后的 universe 是 U0；2. 各自解析为 u32；3. 不能，occurs check 检查递归。

### E03

> 练习三：1. 能观察到 ?T0@U0, ?T0 -> u32, ?T1@U0；2. 返回后 ?T0 == u32, ?T1@U0 == Unknown(U0), current universe 为 U1；3. Ok 时 commit，Err 时回滚。

### E04

> 练习四：1. 结果仍然是原 tuple type，因为 shallow_resolve 只会 resolve 最外层为 infer ty 的情况；2. (Vec<u32>, ?T1, &'?r0 u8); 3. ?T1 和 ?r0 会失败，?T1  resolve 失败，而 ?r0 缺少 region resolve 阶段。

## 提交记录

| 日期 | 轮次 | 说明 |
|---|---:|---|
| 2026-07-27 | 1 | 提交 E01–E04；答案原文如上。 |
| 2026-07-27 | 2 | 提交 snapshot 定向复核题；答案原文如下。 |

## 学习者修正答案

### E03. Snapshot

> 1. (u32, 存在, U1); 2. Unknown(U0); 3. 不存在; 4. U0。

此答案修正 E03 中 `probe` 返回后的状态：闭包内观察结果为 `(u32, 存在, U1)`；退出后 `?T0 -> Unknown(U0)`，`?T1` 不存在，current universe 为 U0。第一轮答案仍按原文保留。
