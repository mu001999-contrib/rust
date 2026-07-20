---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "16"
document: content
status: completed
updated_at: 2026-09-25
---

# 16. Const 与 Const Generics

## 学习目标

1. 识别 `ty::Const` 的参数、推理变量、const alias 与已求出的值，并追踪它们进入 `GenericArgs` 和数组类型的过程。
2. 区分稳定 const generics、`min_generic_const_args` 与 `generic_const_args` 的表达能力及当前特性条件。
3. 理解 `generic_const_args` 如何借具名 const item 表达依赖泛型的计算，并在尚不能求值时使用定义相等。
4. 分开判断 const 实参的类型、可求值性和相等性；了解 next solver 对 const alias 的归一化结果。
5. 区分类型系统中的 `ValTree` 与 MIR/CTFE 的常量表示。

## 前置知识

第 03 章的 `GenericArgs` 与参数索引；第 05 章的推理变量；第 07 章的 `Clause`、`ParamEnv`；第 09–12 章的 alias、normalization 与 goal；第 13 章的关联项投影。

## 核心心智模型

常量可以是类型的一部分。`[u8; N]` 的长度是 `ty::Const`：编译器在尚不知道 N 的数值时，仍知道它是 `usize`。当长度依赖 `N + 1` 时，当前 `generic_const_args` 路径把计算放在**具名 const item 的定义**中，类型位置引用该 item：

```text
const ADD1<const N: usize>: usize = N + 1;
       定义位置保存计算               ↓
[u8; gca!(ADD1::<N>)] ──→ ConstKind::Alias(Free, def_id=ADD1, args=[N])
                                  ├─ 实参具体：尝试 CTFE → Value
                                  └─ 仍依赖泛型：保留 rigid alias → 按定义及实参比较
```

这里的 `gca!` 是 `min_generic_const_args` 的直接表示标记；下文示例均是 **nightly 实验性代码**，还需启用 next solver。`generic_const_exprs` 的「匿名表达式 + `ConstEvaluatable` 前提」属于另一条较早的实现路径；本章只在对照处介绍它。

把常量问题分成三层：

```text
这个实参是什么 IR？ → 它的类型对吗？ → 能否求出值，或保持具名 alias？
                                       ↘ 与另一个 const 的相等性如何证明？
```

本章源码基线为 `upstream/main` 的 `0d38a842626`。在该基线上，`generic_const_args` 仍标记为 incomplete；具体语法和特性开关以源码及 [Unstable Book：generic_const_args](https://doc.rust-lang.org/beta/unstable-book/language-features/generic-const-args.html) 为准。

## 源码地图

| 仓库相对路径 | 符号 | 本章作用 |
|---|---|---|
| `compiler/rustc_type_ir/src/ty_kind.rs` | `TyKind::Array` | 数组元素类型与 const 长度 |
| `compiler/rustc_type_ir/src/const_kind.rs` | `ConstKind`、`AliasConstKind`、`InferConst`、`ValTreeKind` | 类型级常量及 alias 身份 |
| `compiler/rustc_middle/src/ty/consts.rs` | `Const::new_value` 等 | 当前 interner 的常量构造与取值 |
| `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs` | `check_const_item_in_type_system`、`ForbidParamUsesFolder` | 具名 const 与匿名 const 的使用边界 |
| `compiler/rustc_hir_analysis/src/collect/clauses_of.rs` | `gather_explicit_clauses_of`、`const_evaluatable_clauses_of` | `ConstArgHasType` 与旧路径的可求值前提 |
| `compiler/rustc_type_ir/src/relate/combine.rs` | `combine_consts` | const 推断变量及 alias 的关系检查 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` | `evaluate_const_and_instantiate_projection_term` | 求不出具体值时的 GCA rigid-alias 回退 |
| `compiler/rustc_next_trait_solver/src/solve/mod.rs` | `compute_const_evaluatable_goal`、`compute_const_arg_has_type_goal` | 常量相关目标 |
| `compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs` | `push_const_arg_has_type_goal` | const alias 归一化后的类型检查 |
| `compiler/rustc_middle/src/mir/interpret/queries.rs` | `const_eval_resolve` | 带实例实参的 CTFE 查询 |
| `src/doc/unstable-book/src/language-features/generic-const-args.md` | 特性说明 | 定义相等、特性组合与新语法示例 |
| `tests/ui/const-generics/gca/` | `basic.rs`、`non-type-equality-fail.rs` 等 | 当前编译器行为的 UI 测试 |

## 源码精读

### 1. 数组长度、const 参数与 const alias

`compiler/rustc_type_ir/src/ty_kind.rs::TyKind` 与 `compiler/rustc_type_ir/src/const_kind.rs::ConstKind` 的相关变体（其余省略）：

```rust,ignore
Array(I::Ty, I::Const),

Param(I::ParamConst),
Infer(InferConst),
Alias(ty::IsRigid, ty::AliasConst<I>),
Value(I::ValueConst),
Expr(I::ExprConst),
```

`[u8; N]` 可概念化为 `Array(u8, Param(N))`。`[u8; gca!(ADD1::<N>)]` 则引用具名 const item，可概念化为 `Array(u8, Alias(Free(ADD1), [N]))`。`Expr` 仍存在于 IR，但它的源码注释明确指出该变体服务于 `generic_const_exprs`；不能把它当成 GCA 的核心表示。

`Param` 是定义位置量化的 const 泛型参数；`Infer(Var)` 是本次推断要解出的常量变量；`Value` 是已经算出的类型级值。`Infer(Fresh)` 用于轻量缓存 freshening，不是普通待解的 `ConstVid`。

`compiler/rustc_type_ir/src/const_kind.rs::AliasConstKind` 的相关形式：

```rust,ignore
Projection { def_id: I::TraitAssocConstId },
InherentSelf { def_id: I::InherentAssocConstId },
InherentImpl { def_id: I::InherentAssocConstId },
Free { def_id: I::FreeConstAliasId },
Anon { def_id: I::AnonConstId },
```

`AliasConst` 还持有 `args`。`ADD1::<N>` 是 `Free`，`<T as Capacity>::CAP` 是 `Projection`；同一 `def_id` 配不同 args 仍是不同实例。`InherentSelf` 与 `InherentImpl` 区分固有关联常量的 Self 形式与 impl 形式的参数，和第 13 章的关联项参数转换相呼应。

### 2. GCA 让具名 const 在类型位置可见

`compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs::check_const_item_in_type_system` 的关键分支：

```rust,ignore
if tcx.features().generic_const_args() || alias_const.is_direct_const(tcx) {
    Ok(())
} else {
    // ……报告该 const 尚未标记为直接表示……
}
```

`min_generic_const_args` 的 `gca!` 使表达式以类型系统可见的直接形式表示；`generic_const_args` 进一步允许具名 const item 承担泛型计算。`ForbidParamUsesFolder::error` 在 GCA 已开启、匿名 const block 使用泛型参数时建议提取到具名 `type const` item。换言之，主线是**具名定义 + 类型位置引用**，而非直接把 `{ N + 1 }` 当成可自由推理的类型级算式。

`compiler/rustc_ast_passes/src/feature_gate.rs` 对 `generic_const_args` 要求 `-Znext-solver=globally`。当前 `tests/ui/const-generics/gca/basic.rs` 使用 `generic_const_items`、`min_generic_const_args`、`generic_const_args` 和 `-Znext-solver` 测试这条路径。

### 3. 求不出值时保留 rigid alias

`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs::evaluate_const_and_instantiate_projection_term`，省略可求值分支与细节：

```rust,ignore
None if self.cx().features().generic_const_args() => {
    if self.deeply_resolve_ignoring_regions(alias_const).has_non_region_infer() {
        self.evaluate_added_goals_and_make_canonical_response(Certainty::AMBIGUOUS)
    } else {
        self.eq(
            param_env,
            projection_term.to_term(self.cx(), ty::IsRigid::Yes),
            expected_term,
        )?;
        self.evaluate_added_goals_and_make_canonical_response(Certainty::Yes)
    }
}
```

可以求值时先得到具体 const，再与 `expected_term` 建立关系；含尚未解出的推理变量时保留歧义；已知仍依赖泛型且目前无法求值时，把**原始投影**以 rigid alias 形式交给关系检查。源码刻意使用原始 `projection_term`，因为尝试求值时可能曾转到 impl 内的 const 定义，而对外应保留 trait 上原来的 alias 身份。这一分支也覆盖求值失败的其他情形，因此 `rigid` 是当前求解状态的表示，不是“已经证明表达式可求值”的证明。

这就是 GCA 所说的定义相等的基础：保留同一个具名定义及对应实参，不需要先解出 `N + 1` 的数值。对不同 const item，即使源码里写了同一算式，也不能仅凭算术直觉把两个未求值的 alias 合并；若实参具体并成功求值，则可比较结果值。`tests/ui/const-generics/gca/non-type-equality-fail.rs` 检查了不同关联常量在泛型环境中的这种边界。

### 4. 类型检查、可求值目标与旧路径的前提

`compiler/rustc_hir_analysis/src/collect/clauses_of.rs::gather_explicit_clauses_of` 为声明的 const 参数加入：

```rust,ignore
ty::ClauseKind::ConstArgHasType(ct, ct_ty)
```

`compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs::push_const_arg_has_type_goal` 也会在 const alias 归一化后注册类型目标，确保得到的常量类型符合别名声明。`ConstArgHasType(C, usize)` 关心 **C 的类型**；`ConstEvaluatable(C)` 关心能否在相应检查点得到合法值；`C == D` 关心**关系**，三者不能互相替代。

同一 `clauses_of.rs` 仅在 `generic_const_exprs` 开启时调用 `const_evaluatable_clauses_of` 收集旧路径的 `ConstEvaluatable` 前提；该收集器会跳过直接表示的 type const。GCA 的具名 alias 路径因此不以 `where [(); N + 1]:` 作为主机制。`compiler/rustc_next_trait_solver/src/solve/mod.rs::compute_const_evaluatable_goal` 对未解出的 `Infer` 返回 `AMBIGUOUS`；对求值仍太泛化的非 rigid alias 也可能返回 `AMBIGUOUS`，并不直接判定 `NoSolution`。

### 5. const 相等检查如何产生后续目标

`compiler/rustc_type_ir/src/relate/combine.rs::combine_consts` 的相关分支（省略其他分支）：

```rust,ignore
(ty::ConstKind::Infer(ty::InferConst::Var(a_vid)),
 ty::ConstKind::Infer(ty::InferConst::Var(b_vid))) => {
    infcx.equate_const_vids_raw(a_vid, b_vid);
    Ok(a)
}

(ty::ConstKind::Alias(ty::IsRigid::No, alias), _)
    if infcx.next_trait_solver() => {
    relation.register_predicates([ty::ProjectionClause {
        projection_term: alias.into(),
        term: b.into(),
    }]);
    Ok(b)
}
```

两个 const 推理变量可合并；需要归一化的非 rigid alias 在 next solver 路径注册 projection clause。旧 `generic_const_exprs` 路径的 `ConstEquate` 是另一个特定分支，不能把它当成 GCA 的通用相等入口。rigid alias 则由结构关系处理，保留其定义身份与实参。

## 正文

### 1. 稳定 const generics：值成为类型的一部分

```rust
struct Buffer<T, const N: usize>([T; N]);

fn keep<const N: usize>(xs: [u8; N]) -> [u8; N] {
    xs
}

const THREE: usize = 1 + 2;

fn main() {
    let x: Buffer<u8, { 1 + 2 }> = Buffer([1, 2, 3]);
    let y: Buffer<u8, THREE> = x;
    assert_eq!(y.0, keep([1, 2, 3]));
}
```

`Buffer<u8, 3>` 和 `Buffer<u8, 4>` 是不同类型。稳定 const 参数类型目前限于整数、`bool`、`char` 等 [Reference 所列类型](https://doc.rust-lang.org/reference/items/generics.html#const-generics)。具体表达式作为 const 实参需要花括号，例如 `{ 1 + 2 }`；泛型定义内的 `[T; N]` 可以在 N 尚未知时形成类型。

稳定规则下，const 参数在类型或 array repeat 的相关表达式里须独立出现。因此 `[u8; N]` 可以，而直接写 `[u8; N + 1]` 会触及泛型常量表达式限制。具体 `{ 1 + 2 }` 不依赖泛型参数，编译器可直接求出 3。

### 2. `min_generic_const_args` 到 `generic_const_args`

`min_generic_const_args` 提供较窄的“直接 const 实参”机制：`gca!` 告诉 lowering 保留类型系统可见的结构，而不是把表达式一概包成不透明的匿名 const。`generic_const_args` 建立在此机制上，允许用具名 const item 承载更复杂的泛型计算。两者均属 incomplete；`generic_const_args` 还要求 next solver。`compiler/rustc_feature/src/unstable.rs` 的 implied-features 表明，开启 `generic_const_args` 会同时开启 `min_generic_const_args`；下面并列列出特性名，是为了对应仓库示例。

当前 Unstable Book 展示的较新形式是 `type const`：

```rust,ignore
#![feature(generic_const_items, min_generic_const_args, generic_const_args)]
#![expect(incomplete_features)]
// 还需 -Znext-solver=globally

type const ADD1<const N: usize>: usize = const { N + 1 };

struct PlusOne<const N: usize> {
    data: [u8; ADD1::<N>],
}
```

关键点在于 `ADD1` 是具名定义，数组长度引用的是它的 alias，而不是把 `N + 1` 直接放进匿名 const 实参。

仓库 `tests/ui/const-generics/gca/basic.rs` 还使用 `const` 定义加 `gca!` 实参的写法；使用当前仓库的 nightly 编译器时需启用 `-Znext-solver=globally`：

```rust,ignore
#![feature(generic_const_items, min_generic_const_args, generic_const_args)]
#![expect(incomplete_features)]

use std::gca;

const ADD1<const N: usize>: usize = N + 1;

struct PlusOne<const N: usize> {
    data: [u8; gca!(ADD1::<N>)],
}

const FOUR: [u8; gca!(ADD1::<3>)] = [0; 4];
```

`ADD1` 的右侧可以使用 N，因为它是具名泛型 const item 的定义体；类型位置引用 `ADD1::<N>`，于是类型系统拥有一个可追踪的 alias。`PlusOne<3>` 的长度最终能求出 4；在抽象 N 下，`ADD1::<N>` 可以保持为 alias。这里没有 `where [(); N + 1]:` 这样的可求值前提。

两种示例都把泛型计算收束到**具名 const 定义**。具体 nightly 的语法和开关仍在演进；本章的机制解释以当前仓库源码、[Unstable Book](https://doc.rust-lang.org/beta/unstable-book/language-features/generic-const-args.html) 与 UI 测试为准。

### 3. GCA 的相等性：具名身份与具体值

在抽象泛型环境里，两个 `ADD1::<N>` 引用同一个 const 定义、携带同一实参，可以按同一个未求值 alias 来处理。两个不同的定义 `ADD1::<N>` 与 `ALSO_ADD1::<N>` 即使右侧都写 `N + 1`，也不能仅靠表面算式宣称相等。具体到 `N = 3` 后，若两者均能求值，便可以比较各自的 `Value(4_usize)`。

这和第 13 章的关联类型投影相似：`<T as Trait>::Assoc` 先保留 alias 身份，有足够的 impl 或环境信息后再归一化。关联常量 `<T as Capacity>::CAP` 也是 `Projection` alias；具名自由 const `ADD1::<N>` 是 `Free` alias。定义相等不是一个通用的符号代数证明器：`N + 1` 与 `1 + N` 的交换律并非 GCA 自动使用的规则。

### 4. IR 分类与 `ValTree`

| 例子 | 概念化的 `ConstKind` | 已知信息 |
|---|---|---|
| 泛型签名中的 `N` | `Param(ParamConst)` | 参数索引、名字与类型，数值由实例提供 |
| 待推断的 `?C0` | `Infer(Var(ConstVid))` | 本次推断的未知 const |
| 已知长度 `3_usize` | `Value(Value { ty: usize, valtree: 3 })` | 类型与规范化的值 |
| `ADD1::<N>` | `Alias(Free(ADD1), [N])` | 具名定义及泛型实参 |
| `<T as Capacity>::CAP` | `Alias(Projection(CAP), [T])` | 关联常量投影身份 |
| 具体匿名 `{ 1 + 2 }` | 可先经 `Alias(Anon, args)`，求值后为 `Value` | 与 lowering/求值阶段有关 |

这是阶段模型；具体节点还依赖 lowering 入口、特性和当时能否直接求值。`Bound` 与 `Placeholder` 延续第 02、04、10 章的 binder 和 universe 模型，不是普通的未知 const 参数。

`ConstKind::Value` 携带类型和 `ValTree`。`compiler/rustc_type_ir/src/const_kind.rs::ValTreeKind` 将标量表示为 `Leaf`，聚合结构表示为 `Branch`。类型系统要求相等的常量具有相等的规范表示；CTFE 的 `AllocId` 是分配身份，两个不同分配仍可能有相同内容，padding 也不适合直接参与类型级值比较。因此类型系统使用 `ValTree`，而 MIR/CTFE 还保有内存、指针与布局信息。`const_eval_resolve` 在具备实例实参时尝试求值；仍过于泛化时可能返回 `TooGeneric`。

### 5. 三个问题分别证明

以 `struct Flag<const B: bool>;` 为例：

| 问题 | 典型目标或操作 | 结果含义 |
|---|---|---|
| 实参 C 的类型正确吗？ | `ConstArgHasType(C, bool)` | 类型与参数声明相符 |
| C 在此处能得到合法值吗？ | `ConstEvaluatable(C)` / CTFE | 可求值或仍需更多信息 |
| C 与 D 相同吗？ | const relation、alias normalization、具体值比较 | 在当前环境下建立相等 |

两个值都可求值，并不表示它们相等；两个未求值的具名 alias 相同，也不要求先算出具体值。GCA 的 rigid-alias 回退正是为了让后一种关系可被类型系统处理。

### 6. 与 `generic_const_exprs` 的边界

较早的 `generic_const_exprs` 直接允许 `[u8; N + 1]`，常用 `where [(); N + 1]:` 给出可求值前提。其 `thir_abstract_const`、`expand_abstract_consts` 与 `satisfied_from_param_env` 路径曾试图把表达式结构和环境前提匹配起来。该特性开关仍存在于当前源码，但 rustc 开发指南把此设计称为 “the now dead `generic_const_exprs`”；next solver 中 `ConstKind::Expr` 分支也尚未支持。

本章的研究重点因此是 GCA 的**具名 const alias、定义相等、必要时求值**。旧路径用于理解 `ConstKind::Expr`、`ConstEvaluatable` 的来历，不代表 GCA 需要相同的 `where [(); ...]:` 写法。两条实验路径都不能被视为已稳定。

## 常见误区

- `const N: usize` 声明泛型参数；`const ADD1<const N: usize>: usize = ...` 声明具名泛型 const item；两者在 IR 中分别可作为 `Param` 与 `Alias(Free, args)` 出现。
- GCA 在类型位置引用具名 const；直接的匿名 `{ N + 1 }` 仍受限制。具名 item 的定义体可以包含该计算。
- `generic_const_args` 与 `generic_const_exprs` 是不同实现路径；后者的 `ConstEvaluatable` where 前提不是前者的主机制。
- 定义相等根据具名 const 身份与实参建立关系；具体值相等还可以在求值后确认，不能把两者都理解为算术化简。
- `ConstArgHasType`、可求值检查与 const equality 各回答不同问题。
- `ValTree` 是类型系统的值表示；CTFE 另需处理内存、指针和布局。

## 本章小结

`ty::Const` 让 const 泛型参与类型表示。稳定用法中，`Param(N)` 已能构成 `[T; N]`；GCA 把泛型计算放进具名 const item，类型位置持有 `Alias(def_id, args)`。next solver 能求值时产生 `Value`，暂时求不出但推断变量已解决时可保留 rigid alias，从而进行定义相等检查。类型正确、可求值、相等分别有各自的检查路径。

验证记录：2026-09-25 按当前 `upstream/main`（`0d38a842626`）核对源码、Unstable Book 与 `tests/ui/const-generics/gca/` 中的 `check-pass`/`compile-fail` 用例。本机 `rustc 1.99.0-nightly (2026-08-14)` 早于该源码基线，缺少当前示例使用的 `std::gca`，因此 GCA 示例依据仓库 UI 测试而非该本机编译器的执行结果；稳定 const 泛型示例保留此前的编译验证。
