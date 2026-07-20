---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "02"
document: exercises
status: submitted
exercise_version: 2
updated_at: 2026-07-21
---

# 02. 习题

## 作答说明

本章包含两轮练习：

1. visitor/folder 调用骨架、结构替换与快速路径；
2. Binder/de Bruijn、实例化、capture avoidance 与稳定映射。

题号为 Binder E01–E04、visitor/folder E05–E08；正文按实际学习顺序展示 visitor/folder，再展示 Binder。每题 1 分，共 8 分。

## 题目

### 第一轮：Visitor/Folder（时间上先完成）

### E05. Visitor callback 调用路径

给定：

```text
Vec<Option<&'a T>>
```

一个只覆盖 `visit_ty` 和 `visit_region` 的 visitor，按顺序大致会收到哪些 callback？哪些节点需要调用 `super_visit_with` 才能继续向内？

### E06. 找出错误 visitor

以下 visitor 有什么问题？

```rust
fn visit_ty(&mut self, ty: Ty<'tcx>) {
    if let ty::Infer(vid) = *ty.kind() {
        self.vars.push(vid);
    }

    ty.visit_with(self);
}
```

应该如何修改？

### E07. Folder 替换结果

Folder 将 `Param(T)` 替换为 `u32`，但不做 normalization。判断输出：

```text
A. Vec<T>

B. <T as Iterator>::Item

C. fn(T) -> Option<T>

D. Binder<for<'a> fn(&'a T) -> &'a T>
```

### E08. `has_param()` 快速路径

为什么下面的优化既正确又重要？

```rust
if !ty.has_param() {
    return ty;
}
```

它能否写成：

```rust
if ty.kind() != TyKind::Param(...) {
    return ty;
}
```

说明两者的区别。

### 第二轮：Binder/de Bruijn（本章收尾）

### E01. 嵌套 binder 中的 de Bruijn index

对于：

```rust
for<'a> fn(
    &'a u32,
    for<'b> fn(&'a u32, &'b u32),
)
```

按三个 lifetime occurrence 的出现顺序，写出它们的 de Bruijn index。

### E02. 只实例化外层 binder

对 E01 的类型只实例化外层 `'a`，将它替换成 fresh inference region `'?0`。写出结果，并保留未被实例化的内层 `'b` binder。

### E03. 穿过两层新 binder 的 shift

某个 bound occurrence 原来以 `D0` 指向其 binder。将包含该 occurrence 的 value 移入两层新 binder，同时要求它继续指向原 binder。新的 de Bruijn index 是什么？为什么？

### E04. 重复 occurrence 的稳定实例化

实例化：

```rust
for<'a> fn(&'a u32, &'a u32)
```

两处相同 bound region 应得到什么结果？说明 fresh-variable mapping 必须保持的关系。

## 学习者答案

### 第一轮：Visitor/Folder

### E05

> 练习一：visit_ty -> visit_ty -> visit_ty -> visit_region -> visit_ty，除了最后两个都需要调用 super_visit_with 才能继续向内。

### E06

> 练习二：应该调用 super_visit_with。

### E07

> 练习三：A. Vec<u32>; B. <u32 as Iterator>::item; C. fn(u32) -> Option<u32>; D. Binder<for<'a> fn(&'a u32) -> &'a u32>。

### E08

> 练习四：不能，has_param 包含递归的情况，并且是快速路径，第二种只能判断当前顶层类型。

### 第二轮：Binder/de Bruijn

### E01

> 练习一：D0, D1, D0。

### E02

> 练习二：fn(&'?0 u32, for<'b> fn(&'?0 u32, &'b u32))，因为只实例化 'a。

### E03

> 练习三：D0 -> D2，因为中间增加了两层。

### E04

> 练习四：相同 bound 实例化后应该也相同，fn(&'?0 u32, &'?0 u32)。

## 提交记录

| 日期 | 轮次 | 说明 |
|---|---:|---|
| 2026-07-21 | 1 | 提交 visitor/folder 四题。 |
| 2026-07-21 | 2 | 提交 Binder/de Bruijn 四题；四题答案如上。 |
