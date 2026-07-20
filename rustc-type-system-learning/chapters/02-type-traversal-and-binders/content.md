---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "02"
document: content
status: completed
updated_at: 2026-07-21
---

# 02. 遍历、折叠与 Binder

## 学习目标

完成本章后，应当能够：

1. 区分 `TypeVisitable` / `TypeVisitor` 与 `TypeFoldable` / `TypeFolder` 的职责。
2. 解释 `visit_with`、`super_visit_with`、`fold_with`、`super_fold_with` 之间的 dispatch 和递归关系。
3. 使用 `VisitorResult`、`TypeFlags` 和结构缓存理解遍历的提前终止与快速路径。
4. 解释 `Binder<T>`、`BoundVar` 和 `DebruijnIndex` 如何共同表示 higher-ranked variables。
5. 判断 bound variable 对某个子结构而言是否 escaping。
6. 在穿过嵌套 binder、实例化 bound variables 或插入 replacement 时正确执行 shift，避免变量捕获。
7. 说明同一个 bound variable 的多个 occurrence 为什么必须实例化成同一个 fresh variable。

## 前置知识

- 第 01 章中的 interned `Ty`、`Region`、`Const` 与 `TypeFlags`。
- `Param`、`Bound`、`Placeholder`、`Infer` 的基本区别。
- higher-ranked function type 的表面语法，例如 `for<'a> fn(&'a T)`。

## 核心心智模型

本章有两条互相连接的主线：

```text
结构操作
  只读查询：value.visit_with(visitor)
  结构变换：value.fold_with(folder) -> new_value

变量作用域
  Binder(value, bound_vars)
       │
       └── Bound occurrence = (DebruijnIndex, BoundVar)
                               ├── 向外选择哪一层 binder
                               └── 选择该 binder 中的哪个 slot
```

visitor/folder 提供“如何走过 Type IR”的统一机制；binder/de Bruijn 则决定遍历或替换时必须维护的作用域状态。

## 源码地图

| 主题 | 主要路径与符号 |
|---|---|
| visiting traits | `compiler/rustc_type_ir/src/visit.rs`：`TypeVisitable`、`TypeSuperVisitable`、`TypeVisitor` |
| visiting 快速查询 | 同文件：`TypeVisitableExt`、`HasTypeFlagsVisitor`、`HasEscapingVarsVisitor` |
| folding traits | `compiler/rustc_type_ir/src/fold.rs`：`TypeFoldable`、`TypeSuperFoldable`、`TypeFolder`、`FallibleTypeFolder` |
| 通用 shift | 同文件：`Shifter`、`shift_vars`、`shift_region` |
| binder 表示和安全 API | `compiler/rustc_type_ir/src/binder.rs`：`Binder`、`skip_binder`、`no_bound_vars` |
| de Bruijn 定义 | `compiler/rustc_type_ir/src/lib.rs`：`DebruijnIndex`、`shifted_in`、`shifted_out` |
| bound variable replacement | `compiler/rustc_middle/src/ty/fold.rs`：`BoundVarReplacer`、`BoundVarReplacerDelegate` |
| fresh instantiation | `compiler/rustc_infer/src/infer/mod.rs`：`instantiate_binder_with_fresh_vars` |

源码引用以路径和符号名为准；行号随 rustc 演进可能变化。

## 正文

### 1. Visiting：只读遍历

[`TypeVisitable`](../../../compiler/rustc_type_ir/src/visit.rs) 由“能够被遍历的数据”实现，提供入口：

```rust
fn visit_with<V: TypeVisitor<I>>(&self, visitor: &mut V) -> V::Result;
```

[`TypeVisitor`](../../../compiler/rustc_type_ir/src/visit.rs) 由一次具体遍历操作实现，按感兴趣的 Type IR 节点提供 hook：

```text
visit_binder
visit_ty
visit_region
visit_const
visit_predicate
visit_clauses
```

dispatch 方向是：

```text
value.visit_with(visitor)
  ↓ 数据类型识别自身类别
visitor.visit_ty(value) / visit_const(...) / ...
  ↓ visitor 决定停止、替换策略或继续默认递归
value.super_visit_with(visitor)
  ↓ 访问当前节点的子结构
children.visit_with(visitor)
```

`visit_with` 是公开遍历入口；`super_visit_with` 表示“跳过当前节点的自定义 hook，按默认方式访问它的 children”。在 `visit_ty` 内若想继续递归当前 `Ty`，应调用 `ty.super_visit_with(self)`；对同一个节点再次调用 `ty.visit_with(self)` 会重新进入自己的 hook，可能无限递归。

### 2. `VisitorResult` 与提前终止

visitor 的结果类型实现 `VisitorResult`。常见模式是：

- `()`：完整走完，没有需要返回的提前结果。
- `ControlFlow<BreakValue>`：找到目标后立即 `Break`，否则继续。

例如 `HasTypeFlagsVisitor` 在命中目标 flag 后立即停止。它访问 `Ty`、`Const`、`Predicate` 时通常只读取节点缓存的 flags，并不递归内部结构，所以诸如 `has_infer()`、`has_param()` 的结构查询可以很便宜。

但 binder 自己的 `bound_vars` 列表可能包含未在 value 中实际使用的变量；查询 `HAS_BINDER_VARS` 时仍须在 `visit_binder` 中单独检查 binder 元数据。

### 3. Folding：重建而非原地修改

[`TypeFoldable`](../../../compiler/rustc_type_ir/src/fold.rs) 是 `TypeVisitable + Clone` 的子 trait，提供：

```text
try_fold_with  -> 可失败的结构变换
fold_with      -> 不可失败的结构变换
```

对应操作对象为：

```text
FallibleTypeFolder
TypeFolder
```

folder 的典型控制流与 visitor 对称：

```text
value.fold_with(folder)
  ↓
folder.fold_ty(value)
  ↓ 若不替换当前节点，继续默认递归
value.super_fold_with(folder)
  ↓
fold children，构造并 intern 新节点
```

这与第 01 章的 immutable Type IR 一致：folder 不会原地改写 interned `Ty`，而是复用未变化节点，并在需要时构造新结构。

`try_fold_with` 与 `fold_with` 的行为应保持同步；前者用于 normalization、关系检查等可能失败的操作，后者用于 substitution、shift、擦除或解析等不可失败变换。

### 4. 什么时候 visit，什么时候 fold

| 目标 | 机制 |
|---|---|
| 判断是否包含 infer/param/alias/error | `TypeVisitableExt` 或 visitor |
| 收集所有 free regions | visitor |
| 把 `Param(T)` 替换成 generic arg | folder |
| 把 inference vars 解析成已知类型 | folder |
| normalization 并传播错误 | fallible folder |
| 调整 escaping bound vars 的 de Bruijn index | binder-aware folder |

经验法则：只观察或收集使用 visitor；产生新 Type IR 使用 folder。若操作涉及 `Bound`，必须额外回答“当前已经穿过多少层 binder”。

### 5. `Binder<T>` 的实际结构

[`Binder`](../../../compiler/rustc_type_ir/src/binder.rs) 保存：

```rust
pub struct Binder<I: Interner, T> {
    value: T,
    bound_vars: I::BoundVarKinds,
}
```

`bound_vars` 是该 binder 的 slot 列表；每个 slot 说明它绑定 type、region 还是 const，以及必要的名称/kind 信息。value 中的 occurrence 使用 `BoundVar` 指向 slot。

因此一个普通 bound occurrence 的完整身份是：

```text
(DebruijnIndex, BoundVar)
```

- `DebruijnIndex`：从 occurrence 所在位置向外数，选择哪层 binder。
- `BoundVar`：选择那层 binder 内的哪个变量槽位。

`D0` 不是全局编号，也不是变量名；它只表示“当前位置最内层的 binder”。

### 6. de Bruijn index 是相对位置

考虑：

```rust
for<'a> fn(
    &'a u32,
    for<'b> fn(&'a u32, &'b u32),
)
```

三个 region occurrence 按出现顺序表示为：

```text
outer position 的 'a : D0
inner position 的 'a : D1
inner position 的 'b : D0
```

同一个 `'a` 在不同位置可以有不同 de Bruijn index，因为从 occurrence 到定义它的 binder 之间隔着的层数不同。

[`DebruijnIndex::shifted_in(amount)`](../../../compiler/rustc_type_ir/src/lib.rs) 在值被移入新 binder 时增加 index；`shifted_out` 用于反方向调整。若原来指向当前 binder 的 occurrence 为 `D0`，被移入两层不会捕获它的新 binder 后，应成为 `D2`。

### 7. escaping bound variables

对完整类型：

```rust
for<'a> fn(for<'b> fn(&'a u32, &'b u32))
```

`'a` 和 `'b` 的 binder 都包含在整个类型中，因此对整个值而言没有 escaping vars。

若只截取内部函数类型：

```rust
for<'b> fn(&'a u32, &'b u32)
```

它包含 `'b` 的 binder，却不包含 `'a` 的 binder；因此 `'a` 对该子结构而言是 escaping bound variable。

`has_escaping_bound_vars()` 的问题不是“值里有没有 `Bound`”，而是：

> 是否存在某个 bound occurrence，其 binder 不属于当前被检查的值？

`HasEscapingVarsVisitor` 在进入 binder 时把跟踪 index `shift_in(1)`，退出时 `shift_out(1)`，以便相对当前遍历位置判断 escaping。

### 8. Binder 的安全解包

`skip_binder()` 只是拿出 value，并没有实例化变量。若 value 引用该 binder 的 slots，解包后这些 occurrence 立即变成 escaping。

所以源码要求优先使用具有语义的 discharge 操作，例如：

- `no_bound_vars()`：只有 value 不含 escaping bound vars 时才返回内部值。
- 用 fresh inference vars 实例化。
- 用 placeholders 进入 `forall` 检查。
- 其他明确维护 binder 语义的转换。

`skip_binder()` 只适合提取不依赖 bound variables 的数据，或把结果交给明确能够处理 escaping vars 的代码。

### 9. Binder-aware visitor/folder

`Binder<T>::super_visit_with` 和 `super_fold_with` 负责走入 value，但通用实现不会猜测某次算法想如何解释 binder depth。需要作用域状态的 visitor/folder 必须覆盖 `visit_binder` / `fold_binder`：

```text
enter binder: current_index.shift_in(1)
recurse:      binder.super_*_with(self)
leave binder: current_index.shift_out(1)
```

例如 `BoundVarReplacer` 初始 `current_index = D0`。进入 value 内部的嵌套 binder 后变成 `D1`，因此：

- 内层位置引用外层 binder 的 `'a` 是 `D1`，与当前 target depth 匹配，仍会被替换。
- 内层 binder 自己的 `'b` 是 `D0`，不匹配 target depth，必须保留。

这正是“实例化这一层 binder，但不错误消除内部 binder”的机制。

### 10. Capture-avoiding substitution 与 shift

当 replacement 被插入已经穿过若干 binder 的位置时，replacement 自身的 escaping bound variables 可能被新 binder 意外捕获。

通用规则是：

```text
如果 replacement 被带入 k 层新 binder，
则只把 replacement 中相对于它自身 escaping 的 bound vars shifted_in(k)。
```

`Shifter` 使用 `current_index` 判断哪些 vars 对当前子结构是 escaping，只对满足 `debruijn >= current_index` 的 type/region/const bound variables 调整 index。内部 binder 自己绑定的变量不会被 shift。

generic substitution 中的典型例子是：一个实参本身引用外层 bound region，而 `Param` occurrence 位于内层 function binder 中。替换实参时必须根据 `binders_passed` 增加其 de Bruijn depth，使它继续指向原外层 binder。

### 11. Fresh instantiation 必须按 slot 稳定映射

实例化：

```rust
for<'a> fn(&'a u32, &'a u32)
```

不能为两个 occurrence 分别创建两个变量：

```text
错误：fn(&'?0 u32, &'?1 u32)
```

因为原类型要求两处 lifetime 相同。正确做法是每个 binder slot 创建一次 fresh arg，之后按 `BoundVar::index()` 复用：

```text
正确：fn(&'?0 u32, &'?0 u32)
```

`instantiate_binder_with_fresh_vars` 先遍历 `bound_vars` 创建 args 数组，再让所有 region/type/const occurrence 按 slot index 读取同一个元素。这保留了原 binder 内的相等约束。

### 12. 与第 03 章的边界

本章的 `Binder<T>` 引入真正的 `for<...>` bound variables，因此需要 de Bruijn index。

第 03 章的 `EarlyBinder<T>` 不引入 de Bruijn 层；它包装的是相对于 item 泛型参数列表的 `Param`，要求调用者先做 generic-argument instantiation。二者可以嵌套：

```text
EarlyBinder<Binder<FnSig>>
```

处理顺序通常是先替换 item `Param`，再根据高阶检查目的 discharge 内部 `Binder`。

## 常见误区

1. **把 D0 当作固定变量 ID**：D0 只表示当前位置最内层 binder。
2. **进入 binder 就 shift 所有 bound vars**：只应 shift 对当前子结构 escaping 的变量。
3. **把 inference variable 也看成带 de Bruijn depth**：`'?0` 是自由 inference region，不由 de Bruijn index 定位。
4. **实例化外层 binder 时顺便消除内层 binder**：只能替换 target binder 的 occurrences。
5. **同一 bound slot 每次生成一个 fresh var**：会丢失原有相等关系。
6. **在 visitor hook 中对当前节点再次调用 `visit_with`**：可能递归回同一 hook；默认递归应使用 `super_visit_with`。
7. **用 folder 原地修改 interned IR**：folder 产生并 intern 新结构，不改变旧节点。

## 本章小结

- visitor 读取，folder 重建；`super_*` 表示默认递归 children。
- `TypeFlags` 和 `VisitorResult` 提供快速路径与提前终止。
- `Binder<T>` 由 value 和 bound-variable slots 组成。
- bound occurrence 用 `(DebruijnIndex, BoundVar)` 定位。
- de Bruijn index 随 occurrence 的相对嵌套位置变化。
- binder-aware traversal 必须维护当前 depth。
- substitution/移动只 shift escaping vars，以避免捕获。
- fresh instantiation 对每个 binder slot 建立一次稳定映射。

