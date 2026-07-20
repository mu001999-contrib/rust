---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "01"
document: content
status: completed
updated_at: 2026-07-21
---

# 01. Type IR 基础

## 学习目标

完成本章后，应当能够：

1. 区分 `rustc_hir::Ty` 与 `ty::Ty` 的职责。
2. 解释 `Ty<'tcx>`、`Region<'tcx>`、`Const<'tcx>` 的 interned、不可变表示。
3. 按语义识别 `TyKind` 的核心类别，并区分 `Param`、`Bound`、`Placeholder`、`Infer` 与 `Alias`。
4. 区分 Type IR 表示相等、推理统一、归一化后相等和子类型关系。
5. 追踪 `_` 从 AST/HIR 的歧义 generic argument 到 Type IR inference variable 的 lowering 边界。
6. 解释 inference state 为什么存放在 `InferCtxt`，以及 writeback 为什么不会原地修改 interned `Ty`。

## 前置知识

- 熟悉 Rust 泛型、关联类型、const generics 与 higher-ranked lifetime 的表面语法。
- 能在 rustc workspace 中按符号查找定义和调用点。
- 知道 HIR 是经过解析、宏展开和名称解析后的中间表示，但尚不要求理解完整 query pipeline。

## 核心心智模型

```text
源码语法
  │
  ├── AST → HIR lowering
  ▼
HIR：保留源码结构、HirId、Span，并编码尚未消除的语法歧义
  │
  ├── HIR type lowering（ItemCtxt / FnCtxt）
  ▼
Type IR：interned、不可变、面向类型系统语义
  │
  ├── Ty/Const/Region 只保存结构或变量 ID
  └── InferCtxt 保存 inference variable 的可变求解状态
```

需要始终区分两条轴：

```text
表示层：这个 IR 节点现在是什么？
关系层：两个节点能否统一、归一化后相等或满足子类型关系？
```

`Ty == Ty` 只回答表示是否相同，不替代 inference、normalization 或 trait solving。

## 源码地图

| 主题 | 主要路径与符号 |
|---|---|
| `Ty<'tcx>` 及 rustc_middle 别名 | `compiler/rustc_middle/src/ty/mod.rs`：`Ty` |
| Type IR 类型枚举 | `compiler/rustc_type_ir/src/ty_kind.rs`：`TyKind`、`InferTy` |
| 常量与区域枚举 | `compiler/rustc_type_ir/src/const_kind.rs`：`ConstKind`；`compiler/rustc_type_ir/src/region_kind.rs`：`RegionKind` |
| 泛型实参统一表示 | `compiler/rustc_type_ir/src/generic_arg.rs`：`GenericArgKind`；`compiler/rustc_middle/src/ty/generic_args.rs`：`GenericArg` |
| 缓存类型信息 | `compiler/rustc_type_ir/src/ty_info.rs`：`WithCachedTypeInfo`；`compiler/rustc_type_ir/src/flags.rs` |
| HIR 歧义 generic argument | `compiler/rustc_hir/src/hir.rs`：`AmbigArg`、`InferArg`、`GenericArg`、`TyKind` |
| HIR → Type IR | `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs`：`HirTyLowerer`、`GenericArgsLowerer` |
| generic args 对齐 | `compiler/rustc_hir_analysis/src/hir_ty_lowering/generics.rs`：`lower_generic_args` |
| body 中创建推理变量 | `compiler/rustc_hir_typeck/src/fn_ctxt/mod.rs`：`FnCtxt` 的 `HirTyLowerer` 实现 |
| 推断结果写回 | `compiler/rustc_hir_typeck/src/writeback.rs`：`resolve_type_vars_in_body` |
| 后续章节预览 | `compiler/rustc_type_ir/src/binder.rs`：`Binder`；`compiler/rustc_type_ir/src/canonical.rs`：`Canonical` |

源码引用以路径和符号名为准；行号会随 rustc 演进而漂移。

## 正文

### 1. HIR 类型与 Type IR 类型

对于：

```rust
fn f(x: u32) -> u32 {
    x
}
```

HIR 中参数和返回类型是两个与源码位置对应的 `rustc_hir::Ty`，各自具有 `HirId` 和 `Span`。Type IR 中，两处 `u32` 通常共享同一个 interned `ty::Ty<'tcx>`。

```text
rustc_hir::Ty = 用户如何写、写在何处
ty::Ty         = 类型系统认为它在语义上是什么
```

因此 Type IR 不应承担源码定位职责；诊断所需的 span、cause 等信息由外围结构保存。

### 2. `Ty<'tcx>` 是轻量句柄

`Ty<'tcx>` 可近似理解为：

```text
Ty<'tcx>
  └── 指向 tcx arena 中的 interned 对象
        ├── TyKind<'tcx>
        ├── TypeFlags
        └── outer_exclusive_binder
```

由此得到几个不变量：

- `Ty<'tcx>` 是可复制的轻量句柄，不是一棵按值内嵌的完整类型树。
- 子类型、region、const 和 args 继续通过 interned 句柄或列表组成结构。
- `'tcx` 是编译器上下文拥有期，不是用户程序中的 lifetime。
- interned `Ty` 不可变；相同表示可共享节点。
- `WithCachedTypeInfo` 在 interning 时缓存 flags 和 binder 信息，使 `has_infer()`、`has_param()` 等查询不必每次递归整棵树。

`Const<'tcx>` 采用相似的 interned 结构；`Region<'tcx>` 也被 intern，但其布局与 `Ty`、`Const` 不完全相同。

### 3. 按语义分类 `TyKind`

不必先背诵所有 variant，可以先建立以下分类：

| 类别 | 代表 variant | 语义 |
|---|---|---|
| 外层构造已知 | `Bool`、`Int`、`Adt`、`Ref`、`Array`、`Tuple`、`FnDef`、`Dynamic`、`Closure` | 已知最外层类型构造 |
| item 泛型符号 | `Param` | 定义环境中的泛型参数 |
| binder 变量 | `Bound` | 引用外围 binder 的 slot |
| 高阶检查占位符 | `Placeholder` | universally quantified variable 的刚性代表 |
| 推断变量 | `Infer` | 由某个 `InferCtxt` 创建并求解 |
| 尚待归约 | `Alias` | projection、opaque、free alias 等 |
| 错误恢复 | `Error` | 已发出诊断后继续编译 |

“外层构造已知”不代表整个类型已确定：

```text
Vec<?0t>
&'?0 Param(T)
```

两者的 outer constructor 已知，内部仍含符号变量。

### 4. `Ty`、`Const`、`Region` 与 `GenericArg`

| 概念 | 类型 | 常量 | 区域 |
|---|---|---|---|
| item 泛型参数 | `TyKind::Param` | `ConstKind::Param` | `ReEarlyParam` / `ReLateParam` |
| binder 变量 | `TyKind::Bound` | `ConstKind::Bound` | `ReBound` |
| 高阶 placeholder | `TyKind::Placeholder` | `ConstKind::Placeholder` | `RePlaceholder` |
| 推断变量 | `TyKind::Infer` | `ConstKind::Infer` | `ReVar` |
| 错误恢复 | `TyKind::Error` | `ConstKind::Error` | `ReError` |

三类实参通过 `GenericArgKind` 统一：

```text
Lifetime(Region)
Type(Ty)
Const(Const)
```

例如：

```rust
struct Matrix<T, const ROWS: usize, const COLS: usize>(
    [[T; COLS]; ROWS]
);
```

字段类型近似为：

```text
Array(
    Array(Param(T/#0), ParamConst(COLS/#2)),
    ParamConst(ROWS/#1),
)
```

`Matrix<u8, 3, 4>` 近似为：

```text
Adt(MatrixDef, [Type(u8), Const(3usize), Const(4usize)])
```

### 5. 表示相等、统一与归一化

| 比较 | 直接 `==` | 还需要什么 |
|---|---:|---|
| `u32` 与 `u32` | true | 无 |
| `?0t` 与 `u32` | false | 在所属 `InferCtxt` 中建立 equality relation |
| `<T as Iterator>::Item` 与 `u32` | false | 在合适 `ParamEnv` 下 normalization/goal solving |
| `Vec<?0t>` 与 `Vec<u32>` | false | 匹配共同 constructor，并递归统一 args |

所以不能用 `==` 代替：

- unification；
- normalization；
- subtyping/coercion；
- trait solver 能证明的类型等式。

### 6. `_`、`AmbigArg` 与 lowering 边界

generic argument 位置的 `_` 可能是 type argument，也可能是 const argument：

```rust
Foo<_>
```

HIR 使用不可构造的空 enum `AmbigArg` 限制表示：

```text
Ty<'hir, ()>       可以构造 TyKind::Infer(())
Ty<'hir, AmbigArg> 无法构造 TyKind::Infer(AmbigArg)
```

因此 `GenericArg::Type(Ty<AmbigArg>)` 和 `GenericArg::Const(ConstArg<AmbigArg>)` 不会把顶层 `_` 藏在各自的 `Infer` variant 内；歧义 `_` 必须统一表示为：

```text
hir::GenericArg::Infer(InferArg)
```

这只限制顶层表示位置。一个已经确定为 type 的复合 argument 仍可在更深层 generic args 中包含 `_`，例如 `Vec<_>`。

完整边界是：

```text
源码 Matrix<_, 3, 4>
  ↓ parser / AST
ast::GenericArg 中的 infer type 语法
  ↓ AST → HIR lowering
hir::GenericArg::Infer
  ↓ HIR type lowering，读取 GenericParamDefKind
Type 参数  → TyKind::Infer
Const 参数 → ConstKind::Infer
  ↓ type/const inference
求解具体类型或常量
```

`AmbigArg` 不负责 type/const kind resolution；真正的 kind 来自被引用 item 的 generic parameter definition。

### 7. HIR type lowering 不是 AST → HIR lowering

应区分：

| 操作 | 输入 → 输出 | 主要职责 |
|---|---|---|
| AST/HIR lowering | AST → HIR | desugaring、HIR IDs、保存部分语法歧义 |
| HIR type lowering | HIR → Type IR | 根据定义、泛型参数和当前上下文构造语义类型 |

`HirTyLowerer` 的两类重要上下文：

- `ItemCtxt` lower item signature，没有 `InferCtxt`；不合法的 `_` 会诊断并产生 error type/const。
- `FnCtxt` lower body 中的类型，拥有 `InferCtxt`；合法的 `_` 可以创建真正的 inference variable。

`GenericArgsLowerer` 是 HIR type lowering 内部的策略接口，不是独立 compiler phase。通用的 `lower_generic_args` 负责 parent args 拼接、`Self`、参数—实参对齐、默认值/推断选择和错误恢复；具体 lowering context 决定 provided/inferred argument 如何构造。

### 8. `Param`、`Bound`、`Placeholder`、`Infer`

| 表示 | 核心含义 | 能否被 inference 赋值 | 所有者/作用域 |
|---|---|---:|---|
| `Param(T)` | item 定义中的抽象泛型参数 | 否 | `generics_of(def_id)` 定义的 item 环境 |
| `Bound` | binder 引入的变量引用 | 否，必须先实例化 | 对应 `Binder` |
| `Placeholder` | 任意但固定的刚性代表 | 否 | universe 与高阶检查操作 |
| `Infer(?0t)` | 等待约束求解的存在性变量 | 是 | 创建它的 `InferCtxt` |

检查泛型函数体时：

```rust
fn id<T>(x: T) -> T { x }
```

`T` 是 `Param(T)`，不能通过统一把定义本身改成 `u32`。在调用点，先发生 substitution：

```text
Param(T) → Infer(?0t)
```

随后才由实参和其他约束统一 `?0t`。

普通 `Binder` 是可以包裹 `FnSig`、`TraitRef`、predicate 或 `Ty` 的通用量化容器，不是普通 `TyKind` variant。`TyKind::UnsafeBinder` 则是一种真正的第一等类型构造。这部分的 de Bruijn index、escaping bound vars 与 capture avoidance 在第 02 章展开。

### 9. 不可变 Type IR 与外部推断状态

统一：

```text
?0t = u32
```

不会把已有的：

```text
TyKind::Infer(?0t)
```

原地改写为 `TyKind::Uint(U32)`。可变映射存放在 `InferCtxt` 的 unification tables 中：

```text
不可变 Type IR       可变 InferCtxt
Infer(?0t)           ?0t → unknown → u32
```

解析原类型时，folder 根据推断表构造并 intern 新的 resolved Type IR。常见层次包括：

- `shallow_resolve`：只解析最外层推断变量；
- `resolve_vars_if_possible`：递归解析已有答案，保留尚未求解的变量；
- `fully_resolve`：要求相关变量全部可解析，否则返回错误。

HIR type checking 结束时，writeback 读取临时 `TypeckResults` 中含 infer var 的类型和 `InferCtxt` 中的答案，构造新的最终 `TypeckResults`。它不会修改原来的 interned `Ty`。

### 10. 后续章节预览：canonicalization

Canonicalization 将只在某个 `InferCtxt` 中有意义的变量稳定重编号：

```text
?42t → ^0
?7t  → ^1
```

并把 kind、universe 等信息存入 `Canonical<V>`，使逻辑等价的问题可以跨 inference context 交给 solver 和 cache。它不是 normalization，也不直接求解类型。

```text
Infer(?0t)
  ↓ canonicalize
Bound(Canonical, ^0)
```

这只是为理解本章变量分类提供边界；canonical query 的输入、响应和回程映射将在第 10 章系统学习。

### 11. `TypeFlags`：intern 时缓存的递归摘要

Type IR 是大量共享的 interned DAG。resolve、normalize、substitute 和 escaping-variable 检查经常需要先问：

- 是否包含 inference variables？
- 是否包含 generic parameters？
- 是否包含 alias 或 placeholder？
- 是否有 bound variables 或 free regions？
- 是否依赖当前函数或 inference context 中的本地名字？

为避免每次递归整棵结构，`Ty` 和 `Const` 在 interning 时把递归摘要缓存到：

```rust
pub struct WithCachedTypeInfo<T> {
    pub internee: T,
    pub flags: TypeFlags,
    pub outer_exclusive_binder: DebruijnIndex,
}
```

例如：

```text
Infer(?0t)
    flags = HAS_TY_INFER

Option<?0t>
    flags = child.flags

Vec<Option<?0t>>
    flags = child.flags
```

所以最外层 `Vec` 节点即可回答 `has_infer_types() == true`，无需再次进入 `Option`。

主要 flags 可按用途理解：

| 类别 | 代表 flags | 典型用途 |
|---|---|---|
| 泛型参数 | `HAS_TY_PARAM`、`HAS_RE_PARAM`、`HAS_CT_PARAM` | 是否可能需要 substitution |
| 推断变量 | `HAS_TY_INFER`、`HAS_RE_INFER`、`HAS_CT_INFER` | 是否可能需要 inference resolution |
| Placeholder | `HAS_*_PLACEHOLDER` | universe / higher-ranked 检查 |
| Alias | `HAS_ALIAS`、projection/opaque/rigidity flags | 是否可能需要 normalization |
| Bound variables | `HAS_RE_BOUND`、`HAS_TY_BOUND`、`HAS_CT_BOUND` | binder 与 escaping 检查 |
| Region | `HAS_FREE_REGIONS`、`HAS_RE_ERASED` | region erase、writeback、codegen |
| 本地名字 | `HAS_FREE_LOCAL_NAMES` | 是否适合全局 cache |

`HAS_FREE_LOCAL_NAMES` 是组合性质。`Param`、`Infer`、placeholder、fresh variable、某些 local region 等都会使其成立。因此完整判断还要纳入由基础 flag 派生出的缓存约束。

### 12. 快速路径只描述当前 IR 表示

典型快速路径：

```rust
if !value.has_non_region_infer() {
    return value;
}
```

所以：

```text
resolve_vars_if_possible(Vec<T>)
    → 立即返回；Param 不是 Infer

resolve_vars_if_possible(Vec<?0t>)
    → 必须 fold

normalize(Vec<u32>)
    → 立即返回；没有 Alias

normalize(Vec<<T as Iterator>::Item>)
    → 必须尝试 normalization
```

flags 描述当前 interned 表示，不会跟随 `InferCtxt` 中的答案原地改变。例如：

```text
原 Ty：Infer(?0t)                 flags = HAS_TY_INFER
推理表：?0t → <Vec<u32> as Trait>::Assoc
```

原 `Ty` 和 flags 仍保持不变。只有 resolve 得到新的 alias `Ty` 后，新节点才具有 alias flags。因此某些流程必须遵循：

```text
resolve → normalize
```

不能因为原始 infer node 没有 alias flag，就断定解析结果也无需 normalization。

### 13. `outer_exclusive_binder`

`TypeFlags` 能说明“存在 bound variables”，但不能单独回答它们是否逃出当前值。`outer_exclusive_binder` 近似表示：

> 当前值中普通 de Bruijn index 的最大值再加一。

例如：

```text
u32
outer_exclusive_binder = 0

&ReBound(D0, 'a) u32
outer_exclusive_binder = 1
```

用 binder 捕获后：

```text
Binder<for<'a> &'a u32>
outer_exclusive_binder = 0
has_escaping_bound_vars() = false
```

但 `skip_binder()` 得到裸露的内部值后：

```text
&ReBound(D0, 'a) u32
outer_exclusive_binder = 1
has_escaping_bound_vars() = true
```

`Binder` 捕获变量不会清除“内部存在 bound variables”的 flags。因此下面两项可以同时成立：

```text
HAS_BOUND_VARS            = true
has_escaping_bound_vars() = false
```

前者描述结构中存在 binder variables；后者描述是否有变量逃出当前值的 binder 边界。

### 14. Outer rigid 不等于内部全部确定

`TyKind::is_known_rigid()` 主要回答最外层类型构造是否确定：

| 类型 | outer known rigid | 原因 |
|---|---:|---|
| `u32` | 是 | primitive 固定 |
| `Vec<?0t>` | 是 | 最外层一定是 `Adt(Vec)` |
| `&'?r T` | 是 | 最外层一定是 reference |
| `?0t` | 否 | 可能解析成任意类型 |
| `T` | 否 | substitution 后可能是任意类型 |
| `<T as Trait>::Item` | 否 | normalization 后可能是任意类型 |
| bound type / placeholder | 否 | 是刚性变量，但具体外层构造未知 |

还要区分 `Alias(IsRigid::Yes, ...)`：它表示该 alias 在当前 solver scope 中无需继续 normalization，不表示已经知道它最终是某个 `Adt`、reference 或 primitive。当前实现中它仍是 `Alias`。

## 常见误区

1. **把 `Ty<'tcx>` 当成可变类型节点。** 它是 interned、不可变句柄；推断答案在 `InferCtxt`。
2. **把 `Param` 当成未求解的 `Infer`。** `Param` 先通过 substitution 被实例化，实例化结果才可能是 inference variable。
3. **用 `==` 判断类型系统等价。** Alias 可能需要 normalization，infer vars 需要 relation/unification。
4. **认为 `AmbigArg` 自己决定 `_` 是 type 还是 const。** 它只让歧义位置的 `Infer` variant 不可构造；kind resolution 读取 `GenericParamDefKind`。
5. **把 AST → HIR 与 HIR → Type IR 都简称为同一次 lowering。** 两者输入、输出和允许创建 inference variable 的上下文不同。
6. **认为 writeback 覆盖了原 interned `Ty`。** 它构造新的 resolved 结果。
7. **把 placeholder 说成“所有类型本身”。** 它是 universally quantified variable 的任意但固定、刚性代表。
8. **把 canonicalization 当成 normalization。** 前者稳定表示上下文局部变量，后者归约 alias/projection。
9. **认为只列直接 flag 就是完整 flags。** 还要考虑 `HAS_FREE_LOCAL_NAMES` 等组合 flag。
10. **把 `HAS_BOUND_VARS` 等同于 escaping。** binder 可以捕获变量，使 escaping 为 false，同时保留 bound-variable flags。
11. **认为 flags 会随 inference table 自动变化。** flags 属于当前 immutable IR；resolve 后的新节点才有新 flags。

## 本章小结

本章建立了后续源码阅读的底层坐标系：

```text
HIR 描述源码结构
Type IR 描述类型系统语义

Ty/Const/Region 是 interned、不可变表示
InferCtxt 保存可变求解状态

Param       通过 substitution 实例化
Infer       通过约束和统一求解
Bound       必须由 binder 管理
Placeholder 是高阶检查中的刚性代表
Alias       可能需要 normalization

TypeFlags 与 outer_exclusive_binder
            缓存当前 IR 的递归属性和 binder 范围

表示相同 != 可以统一 != 归一化后相等
```

这些不变量是第 02 章 visitor/folder 与 binder、第 03 章 `EarlyBinder`/`GenericArgs`、第 04 章 universe/placeholder，以及第 10 章 canonicalization 的共同前提。
