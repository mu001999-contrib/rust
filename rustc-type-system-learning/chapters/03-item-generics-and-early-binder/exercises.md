---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "03"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-07-21
---

# 03. 习题

## 作答说明

本轮练习覆盖 item 参数的 parent chain、identity/具体实例化、内部 binder 下的 shift，以及 `extend_to` / `rebase_onto`。每题 2 分，共 8 分。

可以继续使用近似 Type IR 记法；关键是 slot 顺序、替换边界和参数坐标系准确。

## 题目

### E01. 展平 parent chain

给定：

```rust
trait Access<'env, K, const CAP: usize> {
    fn fetch<V>(&'env self, key: K, fallback: V) -> (V, [K; CAP]);
}
```

写出：

1. `Access::Generics` 的 `parent_count`、`own_params` 及各参数 index；
2. `fetch::Generics` 的 `parent_count`、`own_params` 及各参数 index；
3. `GenericArgs::identity_for_item(tcx, fetch_def_id)` 的 slot 顺序。

### E02. EarlyBinder 与具体实例化

`tcx.fn_sig(fetch_def_id)` 的结构近似为：

```text
EarlyBinder<Binder<FnSig>>
```

使用完整 args：

```text
[Store, 's, String, 8, bool]
```

写出实例化后的函数签名，并说明外层 `EarlyBinder` 与内层 `Binder` 分别发生了什么。

### E03. Early substitution 与内部 binder

给定定义端值：

```text
EarlyBinder<fn(Param(A), Binder<fn(Param(A))>)>
```

用一个含 escaping bound region 的实参替换 `A`：

```text
A := &'a u32
```

假设在实参自身位置，`'a` 表示为 `ReBound(D0)`。写出两个 `A` occurrence 被替换后的 de Bruijn index，并解释差异。

### E04. `extend_to` 与 `rebase_onto`

回答两个小问：

1. 已有 `Access` 的具体 args `[Store, 's, String, 8]`，要扩展为 method `fetch<V>` 的完整 args，并为 `V` 创建 `?V`。应使用哪个操作，结果是什么？
2. 对：

   ```rust
   trait X<S> { fn f<T>(); }
   impl<U> X<U> for U { fn f<V>() {} }
   ```

   已知 trait method args 为 `[Self, S, T]`，`source_ancestor = X`，impl 一侧 `target_args = [U]`。调用相应操作后的结果是什么？

## 学习者答案

### E01

> 练习一：1. 0、[Type(Self), Lifetime(env), Type(K), Const(CAP)]、0, 1, 2, 3；2. 4、[Type(C)]、4；3. [Self#0, env#1, K#2, CAP#3, C#4]。

### E02

> 练习二：fn(&'s Store, String, bool) -> (bool, [String; 8])，外层替换 Param 为 Arg，进入 Binder binders_passed 会 +1。

### E03

> 练习三：替换后第一个为 D0，第二个为 D1，因为第二个增加了一层 Binder。

### E04

> 练习四：1. 用 extend_to，结果是 [Store, 's, String, 8, ?V]；2. 调用 rebase_onto 的结果是 [U, T]。

## 提交记录

| 日期 | 轮次 | 说明 |
|---|---:|---|
| 2026-07-21 | 1 | 提交 E01–E04；答案原文如上。 |
