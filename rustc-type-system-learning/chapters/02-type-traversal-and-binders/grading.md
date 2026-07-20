---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "02"
document: grading
status: graded
exercise_version: 2
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-07-21
---

# 02. 评分与反馈

## 总评

得分 `8 / 8`（100%），两轮共八题全部正确：

- visitor/folder：`4 / 4`；
- Binder/de Bruijn：`4 / 4`。

## 分题评分

题目按实际学习时间排列：visitor/folder E05–E08 在前，Binder/de Bruijn E01–E04 在后。

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E05 | 1 | 1 | 正确 | callback 顺序正确；复合 `Ty` 需用 `super_visit_with` 进入子节点，`Region` 非递归，末尾 `Param(T)` 是叶节点。 |
| E06 | 1 | 1 | 正确 | 应把对当前 `Ty` 的 `visit_with` 改成 `super_visit_with`，否则重新 dispatch 到同一 `visit_ty` 并无限递归。 |
| E07 | 1 | 1 | 正确 | 四个 substitution 结果都正确；alias 不自动 normalization，binder 中的 `'a` 保留。原答的 `item` 指语义上的关联类型 `Item`。 |
| E08 | 1 | 1 | 正确 | `has_param()` 查询整棵子树且利用缓存；只匹配根 `TyKind::Param` 无法覆盖 `Vec<T>` 等嵌套参数。 |
| E01 | 1 | 1 | 正确 | 外层位置的 `'a` 为 D0；进入内层 binder 后，外层 `'a` 为 D1；内层 `'b` 为 D0。 |
| E02 | 1 | 1 | 正确 | 只 discharge 外层 `'a`；它在内外两个位置都替换为同一 `'?0`，内层 `'b` 仍由 `for<'b>` 绑定。 |
| E03 | 1 | 1 | 正确 | occurrence 被移入两层新 binder 后需 `shifted_in(2)`，从 D0 变为 D2，才能继续指向原 binder。 |
| E04 | 1 | 1 | 正确 | 两个 occurrence 指向同一个 binder slot，必须复用同一个 fresh inference region。 |

## 逐题解释

### E05：Visitor callback 路径

对于：

```text
Vec<Option<&'a T>>
```

callback 路径大致是：

```text
visit_ty(Vec<Option<&'a T>>)
  visit_ty(Option<&'a T>)
    visit_ty(&'a T)
      visit_region('a)
      visit_ty(T)
```

外层 `Vec`、`Option` 和 `Ref` 三个复合 `Ty` 必须调用 `super_visit_with` 才能递归。`Region` 在 Type IR traversal 中是非递归节点，没有对应的 `super_visit_with`；`Param(T)` 是叶节点，调用 `super_visit_with` 也不会再产生子节点。

学习者给出的顺序和“最后两个不需要继续向内”的判断都正确。

### E06：`visit_with` 与 `super_visit_with`

在 `visit_ty` hook 中对同一 `ty` 再调用：

```rust
ty.visit_with(self)
```

会再次 dispatch 回当前 `visit_ty`：

```text
visit_ty
  → ty.visit_with
    → visit_ty
      → ...
```

正确修改是：处理当前节点后，对非终止分支调用：

```rust
ty.super_visit_with(self)
```

学习者直接指出“应该调用 `super_visit_with`”，准确识别了修复点。

### E07：Param substitution 不等于 normalization

四个结果分别是：

```text
Vec<T>
→ Vec<u32>

<T as Iterator>::Item
→ <u32 as Iterator>::Item

fn(T) -> Option<T>
→ fn(u32) -> Option<u32>

for<'a> fn(&'a T) -> &'a T
→ for<'a> fn(&'a u32) -> &'a u32
```

Folder 只替换 `Param(T)`：

- projection 仍是 `Alias`，不会自动求得关联类型；
- `'a` 是该 `Binder` 引入的 `ReBound`，不是目标 `Param(T)`，所以保持不变。

### E08：递归属性与根节点匹配

```rust
!ty.has_param()
```

表示整个 Type IR 子树不包含 parameter，并可利用 intern 时缓存的 `TypeFlags` 快速判断。因此 folder 可以安全复用整棵子树。

而根节点检查只回答：

```text
当前最外层是不是 Param？
```

`Vec<T>` 的根是 `Adt(Vec, ...)`，内部仍有 `Param(T)`。以递归属性作为返回条件才能覆盖这些嵌套参数。

### E01：相对 de Bruijn index

de Bruijn index 是 occurrence 到目标 binder 的相对距离，不是 lifetime 的全局 ID：

```text
outer occurrence: 当前最内层就是 for<'a> → D0
inner occurrence: 先跨过 for<'b> 才到 for<'a> → D1
inner 'b':        当前最内层就是 for<'b> → D0
```

这与 `compiler/rustc_type_ir/src/lib.rs` 中 `DebruijnIndex` 的嵌套函数示例一致。

### E02：只实例化目标 binder

实例化外层 binder 时，replacer 的 target depth 初始为 D0；进入内部 `for<'b>` 后 target depth 变成 D1。因此内层位置的 outer `'a` 虽然写作 D1，仍匹配目标 binder；inner `'b` 是该位置的 D0，不匹配，必须保留。

替换出的 `'?0` 是自由 inference region，不由 de Bruijn index 定位，所以它在内部 binder 中仍是同一个 `'?0`。

对应实现位于 `compiler/rustc_middle/src/ty/fold.rs` 的 `BoundVarReplacer::fold_binder` 和 `fold_region`。

### E03：Capture-avoiding shift

引入两层不应捕获原变量的新 binder，相当于：

```text
D0.shifted_in(2) = D2
```

若仍保留 D0，它会改为指向新引入的最内层 binder。`compiler/rustc_type_ir/src/fold.rs` 的 `Shifter` 只 shift 对当前子结构 escaping 的 bound vars，从而保持内部 binder 自己绑定的变量不变。

### E04：按 binder slot 稳定映射

两处 `'a` 是同一个 `BoundVar` slot 的两个 occurrence。正确算法为每个 slot 创建一次 fresh arg，再按 slot index 复用：

```text
保持同一 slot：fn(&'?0 u32, &'?0 u32)
分别创建变量：fn(&'?0 u32, &'?1 u32)
```

同一 slot 复用同一变量，才能保留“两个 region 相同”的原约束。`compiler/rustc_infer/src/infer/mod.rs` 的 `instantiate_binder_with_fresh_vars` 先建立 `args` 数组，再通过 bound-var index 读取 replacement。

## 已掌握概念

- `TypeVisitable`/`TypeVisitor` 与 `TypeFoldable`/`TypeFolder` 的分工。
- `visit_with`/`fold_with` 进入自定义 hook，`super_*_with` 执行默认子结构递归。
- cached `TypeFlags`、提前终止和递归属性快速路径。
- substitution 只做结构替换，不隐式 normalization。
- 普通 `Param` substitution 会穿过 binder，但不替换 binder 自己引入的 variables。
- `Binder<T>`、`BoundVar` 与 de Bruijn index 的组合定位模型。
- nested binder 下的相对 index 计算和 escaping bound variables。
- capture-avoiding shift 与指定 binder 的实例化。
- 同一 binder slot 到 fresh variable 的稳定映射。

## 后续复核重点

E07 原答中的小写 `item` 对应源码符号 `Item`；substitution 结论正确。

后续仍需保持：

- inference region 是自由变量，不带 de Bruijn depth；
- placeholder 与 inference variable 的可赋值性不同；
- 第 03 章的 `EarlyBinder` 不引入普通 de Bruijn binder。

## 补充练习或复习动作

第 03 章将把本章的真正 `Binder<T>` 与 item-level `EarlyBinder<T>` 对照，并把 substitution 扩展到 parent generic args。

## 完成判定

- 两轮八题全部正确，完整成绩 100%。
- 能正确解释 visitor dispatch、默认递归、folder replacement 和缓存快速路径。
- 能手算嵌套 binder、目标 binder 实例化和两层 shift。
- 能清晰区分自由 inference variable 与 bound variable。
- 能说明 stable mapping 保留相等约束的原因。

结论：第 02 章 `completed`，`mastery: mastered`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-07-21 | 完成两轮八题的综合评估 | `8 / 8`，`mastered`，章节完成。 |
