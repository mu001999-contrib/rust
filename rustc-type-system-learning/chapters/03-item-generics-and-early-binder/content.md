---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "03"
document: content
status: completed
updated_at: 2026-07-21
---

# 03. Item 泛型与 EarlyBinder

## 学习目标

完成本章后，应当能够：

1. 区分 item-level `EarlyBinder<T>` 与 higher-ranked `Binder<T>`。
2. 读懂 `Generics` 的 `parent`、`parent_count`、`own_params`，以及 `GenericParamDef::index`。
3. 把 parent chain 展平为完整、按 index 对齐的 `GenericArgs`。
4. 区分 identity args、具体 args 与调用点的 fresh inference args。
5. 使用 `EarlyBinder::instantiate` / `instantiate_identity` discharge item 泛型。
6. 解释 `extend_to` 与 `rebase_onto` 分别如何扩展参数后缀和替换父参数前缀。
7. 说明 early-bound substitution 穿过内部 `Binder` 时为何仍可能需要 shift。

## 前置知识

- 第 01 章中的 `ParamTy`、`ParamConst`、`ReEarlyParam` 与 `GenericArg`。
- 第 02 章中的 visitor/folder、`Binder<T>`、de Bruijn index 与 capture-avoiding shift。
- Rust 的 trait、impl、associated method 与 const generics 表面语法。

## 核心心智模型

item 泛型由两部分配合表示：

```text
参数声明表
tcx.generics_of(def_id)
  └── Generics { parent, parent_count, own_params }
             │
             └── 每个 GenericParamDef 有全局 index

使用参数的值
tcx.type_of(def_id) / tcx.fn_sig(def_id)
  └── EarlyBinder<T>
        └── T 内部出现 ParamTy / ParamConst / ReEarlyParam(index)

实例化
完整 GenericArgs = [parent args..., own args...]
  └── EarlyBinder::instantiate(tcx, args)
        └── 按 index 替换 T 中的 item 参数
```

最重要的不变量是：

```text
param.index == 它在该 item 完整 GenericArgs 中的位置
```

`EarlyBinder` 是一个 API 边界：它提醒调用者内部值仍以某个 item 的 `Param` slots 表示。它不保存 `bound_vars`，也不会为这些参数引入 de Bruijn 层。

## 源码地图

| 主题 | 主要路径与符号 |
|---|---|
| `EarlyBinder` 与实例化 | `compiler/rustc_type_ir/src/binder.rs`：`EarlyBinder`、`ArgFolder` |
| item 参数声明 | `compiler/rustc_middle/src/ty/generics.rs`：`Generics`、`GenericParamDef`、`GenericParamDefKind` |
| 参数节点 | `compiler/rustc_middle/src/ty/sty.rs`：`ParamTy`、`ParamConst`；`compiler/rustc_middle/src/ty/region.rs`：`EarlyParamRegion` |
| args 构造与变换 | `compiler/rustc_middle/src/ty/generic_args.rs`：`identity_for_item`、`for_item`、`extend_to`、`rebase_onto` |
| 常见查询返回值 | `compiler/rustc_middle/src/queries.rs`：`type_of`、`fn_sig` |

源码引用以路径和符号名为准；行号随 rustc 演进可能变化。

## 正文

### 1. `EarlyBinder` 与 `Binder` 是两种不同边界

先把两者并排：

| 维度 | `EarlyBinder<T>` | `Binder<T>` |
|---|---|---|
| 表示的量化层 | item 泛型，如 `struct S<T>`、`fn f<T>` | late-bound / higher-ranked 变量，如 `for<'a> fn(&'a T)` |
| 变量 occurrence | `ParamTy`、`ParamConst`、`ReEarlyParam` | `Bound` / `ReBound` |
| 如何定位变量 | item 完整 args 中的绝对 `index` | `(DebruijnIndex, BoundVar)` |
| wrapper 是否保存变量表 | 否；参数表来自 `tcx.generics_of(def_id)` | 是；保存 `bound_vars` slots |
| 主要 discharge | `instantiate(tcx, args)`、`instantiate_identity()` | fresh vars、placeholders 或专用 bound-var replacement |
| 是否增加 de Bruijn depth | 否 | 是 |

因此：

```rust
fn apply<T>(f: for<'a> fn(&'a T)) {}
```

包含两层不同结构：

```text
EarlyBinder<                    // 管 T
  Binder<FnSig>                 // 管 'a
>
```

`T` 由 item 参数 index 找到，`'a` 由 de Bruijn index 和 bound-var slot 找到。

### 2. 为什么 query 返回 `EarlyBinder`

当前源码中两个典型 query 是：

```text
tcx.type_of(def_id) -> EarlyBinder<Ty>
tcx.fn_sig(def_id)  -> EarlyBinder<PolyFnSig>
```

其中 `PolyFnSig` 本身是 `Binder<FnSig>`。所以读取函数签名时，必须分别处理：

1. 外层 `EarlyBinder`：item 自己及 parent items 的泛型参数；
2. 内层 `Binder`：函数签名中的 late-bound variables。

wrapper 的作用是让类型系统 API 明确表达：query 返回的值仍位于定义端参数坐标系中，调用者要先选择用什么 args 解释它。

### 3. `Generics` 只存 own params，并用 parent chain 表达继承

[`Generics`](../../../compiler/rustc_middle/src/ty/generics.rs) 的核心字段为：

```rust
pub struct Generics {
    pub parent: Option<DefId>,
    pub parent_count: usize,
    pub own_params: Vec<GenericParamDef>,
    // ...
}
```

- `parent`：拥有外层泛型参数的 item，例如 associated method 的 trait/impl。
- `parent_count`：所有 parent 参数在完整 args 中占用的前缀长度。
- `own_params`：当前 item 新声明的参数，不重复保存 parent 参数。
- `count()`：`parent_count + own_params.len()`。

每个 [`GenericParamDef`](../../../compiler/rustc_middle/src/ty/generics.rs) 包含：

```text
name, def_id, index, kind
```

这里的 `index` 是展平 parent chain 后的绝对位置，而不是 `own_params` 内的局部下标。

### 4. 手算 parent chain 与绝对 index

考虑：

```rust
trait Build<'a, T> {
    fn build<U, const N: usize>(&'a self, value: T, extra: U) -> [U; N];
}
```

trait 的参数表近似为：

```text
Build::Generics
  parent       = None
  parent_count = 0
  own_params   = [Self#0, 'a#1, T#2]
  count        = 3
```

method 的参数表近似为：

```text
build::Generics
  parent       = Some(Build)
  parent_count = 3
  own_params   = [U#3, N#4]
  count        = 5
```

于是 `build` 的任何完整 `GenericArgs` 都必须按如下 slot 对齐：

```text
index:  0      1    2    3    4
param:  Self   'a   T    U    N
args:  [A0,    A1,  A2,  A3,  A4]
```

参数名称便于打印和诊断；实例化真正依赖的是 kind 与 index 对齐。

### 5. `GenericArgs` 是统一的实参向量

`GenericArgs` 的每个元素是以下三者之一：

```text
Lifetime(Region)
Type(Ty)
Const(Const)
```

同一个 args 列表可以同时承载 `Self`、lifetime、type 和 const 参数。`ArgFolder` 遇到参数 occurrence 时按 index 读取对应元素，并检查 kind：

```text
ParamTy(index = i)        -> args[i] 必须是 Type
ReEarlyParam(index = i)   -> args[i] 必须是 Lifetime
ParamConst(index = i)     -> args[i] 必须是 Const
```

缺少 slot 或 slot kind 不匹配都表示调用者没有遵守完整参数表的不变量。

### 6. identity args 与具体 args

[`GenericArgs::identity_for_item`](../../../compiler/rustc_middle/src/ty/generic_args.rs) 为每个参数构造“映射回自己”的实参：

```text
Self#0 -> Param(Self#0)
'a#1   -> ReEarlyParam('a#1)
T#2    -> Param(T#2)
U#3    -> Param(U#3)
N#4    -> ParamConst(N#4)
```

所以 `build` 的 identity args 可写成：

```text
[Self#0, 'a#1, T#2, U#3, N#4]
```

identity instantiation 的用途是：在 item 自己的定义环境中，把 query 返回值解释成仍引用同一组刚性参数的值。

具体实例化则会提供真实 replacement。例如：

```text
[Widget, 'x, u32, String, 4]
```

应用到 `build` 的签名后得到近似结果：

```text
fn(&'x Widget, u32, String) -> [String; 4]
```

调用点尚未知道具体实参时，也可以把某些 slots 填为 fresh inference args；这仍是先实例化 `Param`，然后再由 `InferCtxt` 求解，不是让 `Param` 本身参与赋值。

### 7. `identity_for_item` 如何沿 parent chain 构造完整列表

实现骨架是：

```text
for_item(def_id)
  -> fill_item(generics_of(def_id))
       -> 若有 parent，先递归 fill_item(parent)
       -> 再按 own_params 顺序 append 当前参数
       -> assert(param.index == args.len())
```

这条断言把“声明表 index”和“实参向量位置”锁在一起。它也解释了为何为 associated item 手工只构造 `[U, N]` 不够：method 内的 `Self`、`'a`、`T` occurrence 仍会索引前面的 parent slots。

### 8. `EarlyBinder::instantiate` 做什么

[`EarlyBinder::instantiate`](../../../compiler/rustc_type_ir/src/binder.rs) 使用 `ArgFolder` 折叠内部值：

```text
ParamTy(i)       -> args[i].expect_type()
ReEarlyParam(i)  -> args[i].expect_lifetime()
ParamConst(i)    -> args[i].expect_const()
```

它是结构 substitution，不等于 normalization。当前 API 返回 `Unnormalized<T>`，调用方需要在相应阶段决定如何处理潜在 alias；源码中的 `.skip_norm_wip()` 表达的是显式接受当前未归一化值。

`instantiate_identity()` 的结果在结构上通常仍包含相同 `Param` 节点，但它在语义上 discharge 了 wrapper：调用者声明“现在就在该 item 的 identity 参数环境中”。这里的“identity placeholder”是概念说法，不是第 04 章将讨论的 `TyKind::Placeholder` / `RePlaceholder`。

### 9. Early substitution 穿过 `Binder` 仍需 shift

`EarlyBinder` 自己不增加 de Bruijn depth，但它内部可以包含真正的 `Binder`：

```rust
type Func<A> = fn(A);
type MetaFunc = for<'a> fn(Func<&'a i32>);
```

实例化 `Func<A>` 时，如果 `A` 的 replacement 含有指向外层 `'a` 的 escaping `ReBound(D0)`，而 `A` 的 occurrence 位于内部 function binder 下面，那么 replacement 插入该位置后必须变为 `D1`。

`ArgFolder` 因此维护 `binders_passed`：

```text
进入内部 Binder  -> binders_passed += 1
替换 Param       -> 对 replacement 的 escaping bound vars shifted_in(binders_passed)
离开内部 Binder  -> binders_passed -= 1
```

这不是说 early param 有 de Bruijn index；需要 shift 的是作为具体 arg 传入、并含有 escaping bound vars 的 replacement。

### 10. `extend_to`：保留已有前缀，补齐 descendant slots

当已有某个 parent 的完整 args，需要构造 child item 的 args 时，可以使用：

```text
existing_args.extend_to(tcx, child_def_id, make_missing)
```

语义为：

```text
已有 index 的 slot -> 复用 existing_args[index]
尚未覆盖的 slot    -> 调用 make_missing(param, args_so_far)
```

例如已有 trait args：

```text
[Widget, 'x, u32]
```

扩展到 `build<U, N>`，并为新 slots 创建 inference args，可得：

```text
[Widget, 'x, u32, ?U, ?N]
```

closure 能看到已经构造的 `args_so_far`，因此也可按先前 slots 实例化默认参数。

### 11. `rebase_onto`：换父坐标系，保留 child 后缀

`rebase_onto` 处理 trait item 与 impl item 等映射。源码中的经典例子是：

```rust
trait X<S> { fn f<T>(); }
impl<U> X<U> for U { fn f<V>() {} }
```

若：

```text
self            = [Self, S, T]  // trait method f 的 args
source_ancestor = X              // X 的参数前缀长度为 2
target_args     = [U]            // impl 一侧的父参数坐标系
```

则：

```text
self.rebase_onto(...)
  = target_args + self[source_ancestor.count()..]
  = [U] + [T]
  = [U, T]
```

可以把它记成：

```text
extend_to   = 保留已有前缀，生成缺失后缀
rebase_onto = 替换来源前缀，保留已有后缀
```

### 12. 安全解包与常见调用形态

`EarlyBinder` 提供三种不同意图：

```text
instantiate(tcx, args)   用指定完整 args 做 substitution
instantiate_identity()  在 item 自身参数环境中使用 identity mapping
no_bound_vars()          仅在内部值完全没有 Param 时取出
```

`skip_binder()` 适合读取不依赖泛型参数的数据，例如内部值的 `DefId` 或函数参数个数；对依赖参数的 Type IR，应先实例化。

典型源码阅读方式：

```rust
let args = ty::GenericArgs::identity_for_item(tcx, def_id);
let ty = tcx.type_of(def_id).instantiate(tcx, args).skip_norm_wip();
```

若就在 item 自己的 identity 环境中，也常见：

```rust
let ty = tcx.type_of(def_id).instantiate_identity().skip_norm_wip();
```

## 常见误区

1. `EarlyBinder` 的参数声明表位于 `Generics`，而 `Binder` 自带 `bound_vars`。
2. `GenericParamDef::index` 对齐完整 parent-chain args，不是 `own_params` 的局部位置。
3. associated item 的 `GenericArgs` 包含 parent 前缀和 own 后缀。
4. identity instantiation 保留 `Param` 表示，但已经明确了它们所在的 item 参数环境。
5. early substitution 本身不创建 de Bruijn 层；replacement 穿过内部真正的 `Binder` 时仍需 capture-avoiding shift。
6. instantiation 与 normalization 是两个阶段；`Unnormalized<T>` 让这一边界显式可见。

## 本章小结

读取 item 泛型时，可以按以下顺序机械推导：

```text
1. 找 def_id，并读取 generics_of(def_id)
2. 沿 parent chain 计算完整参数 slots
3. 用 GenericParamDef::index 标定每个 Param occurrence
4. 构造完整 GenericArgs：[parent..., own...]
5. 用 EarlyBinder::instantiate 或 instantiate_identity discharge wrapper
6. 若 replacement 被插入内部 Binder，按 binders_passed shift escaping vars
7. 再按调用阶段决定 normalization、inference 或 relation checking
```

掌握这套“声明表 + 扁平 args + 按 index substitution”的模型后，trait/impl associated items、默认泛型参数以及后续 monomorphization 的参数传递都会落在同一条主线上。
