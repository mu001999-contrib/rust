---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "07"
document: content
status: completed
updated_at: 2026-08-02
---

# 07. Predicate、Clause 与 ParamEnv

## 学习目标

完成本章后，应当能够：

1. 区分 `Predicate`、`Clause`、`GenericPredicates` 与 `ParamEnv` 的职责。
2. 把常见 Rust bounds 翻译为对应的 `ClauseKind`。
3. 解释为什么 `Clause` 是 `Predicate` 的可假设子集，以及为什么 `Subtype` 不是 `ClauseKind`。
4. 从 HIR bounds 追踪到 `predicates_of`，再追踪到 `tcx.param_env(def_id)`。
5. 解释父 item predicates 如何通过 `GenericPredicates::parent` 被继承和实例化。
6. 区分 supertrait elaboration、ADT inferred outlives 与函数签名的 implicit implied outlives。
7. 解释 trait solver 如何把 `ParamEnv` 中的 caller bounds 当作候选证明当前 goal。

## 前置知识

- 第 02 章的 `Binder`、bound vars 与 de Bruijn index。
- 第 03 章的 item generics、parent chain、identity args 与实例化。
- 第 04 章的 `RegionOutlives`、`TypeOutlives` 与 universe。
- 第 05 章的推理上下文与 inference variables。
- 第 06 章的 type relation，以及 relation 无法立即处理时产生的 subtype predicate。

## 核心心智模型

先把四个名词固定为四层：

```text
Predicate
  = “当前想证明的一条命题”所使用的通用 IR

Clause
  = Predicate 中可以作为环境假设的一部分

GenericPredicates
  = 某个 definition 自己声明/推导的 clauses + parent 链 + spans

ParamEnv
  = 在某次类型系统操作中已经进入作用域、可供求解器使用的 clauses
```

完整数据流是：

```text
Rust 源码中的泛型 bounds / where clauses
  ↓ HIR lowering
GenericPredicates {
    parent,
    predicates: [(Clause, Span)],
}
  ↓ 递归 parent + identity/具体 args 实例化
实例化后的 clauses
  ↓ supertrait elaboration、去重、必要的 normalization
ParamEnv { caller_bounds }
  ↓ 与待证明命题配对
Goal { param_env, predicate }
  ↓
从 ParamEnv、impl、builtin 等来源组装 candidates
```

最重要的方向是：

```text
ParamEnv 是假设集
Predicate 是目标命题的表示
Goal = 在某个 ParamEnv 下证明某个 Predicate
```

例如：

```rust,ignore
fn duplicate<T: Clone>(x: Vec<T>) -> Vec<T> {
    x.clone()
}
```

检查 `x.clone()` 时，核心证明可以概念化为：

```text
assumptions = ParamEnv [T: Sized, T: Clone]
goal        = Vec<T>: Clone
```

求解器先使用标准库中的 `impl<T: Clone> Clone for Vec<T>`，它产生嵌套目标 `T: Clone`；随后扫描 `ParamEnv`，直接用 caller bound `T: Clone` 完成这个嵌套目标。

## 四个概念的边界

| 概念 | 表示什么 | 是否带 binder | 是否带源码 span | 典型用途 |
|---|---|---:|---:|---|
| `Predicate` | 可交给 solver 证明的命题 | 是 | 否 | goal/obligation 的命题部分 |
| `Clause` | 可成为环境假设的 predicate 子集 | 是 | 否 | `ParamEnv::caller_bounds` |
| `GenericPredicates` | definition 的 clauses 与 parent 元数据 | clauses 自身带 | 是 | query 结果、实例化父子约束 |
| `ParamEnv` | 当前作用域内可假设成立的 clauses | clauses 自身带 | 否 | trait solving、normalization、const evaluation 等 |

`Obligation` 会在第 08 章展开。现在先把它理解为“附带原因、深度和 `ParamEnv` 等处理上下文的 `Predicate`”。所以 `Predicate` 是命题 IR，`Obligation` 是一项需要被处理和诊断的工作。

## 源码地图

| 路径 | 关键符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_type_ir/src/predicate_kind.rs` | `ClauseKind`、`PredicateKind` | 定义可假设 clauses 与全部可证明 predicates 的边界 |
| `compiler/rustc_type_ir/src/predicate.rs` | `TraitPredicate`、`OutlivesPredicate` 等 | 定义各类 predicate 的数据载荷 |
| `compiler/rustc_middle/src/ty/predicate.rs` | `Predicate`、`Clause` | rustc 中 interned、binder-aware 的包装类型 |
| `compiler/rustc_middle/src/ty/generics.rs` | `GenericPredicates`、`instantiate_into` | 保存 own predicates、parent 链并完成实例化 |
| `compiler/rustc_hir_analysis/src/hir_ty_lowering/bounds.rs` | `lower_bounds` | 把 HIR trait/outlives bounds 降低为 clauses |
| `compiler/rustc_hir_analysis/src/collect/predicates_of.rs` | `gather_explicit_predicates_of`、`predicates_of` | 收集显式/编译器插入的 clauses 与 inferred outlives |
| `compiler/rustc_ty_utils/src/ty.rs` | `param_env` | 从 `predicates_of` 构造 item 的参数环境 |
| `compiler/rustc_trait_selection/src/traits/mod.rs` | `normalize_param_env_or_error` | elaboration、去重与旧 solver 所需的 normalization |
| `compiler/rustc_type_ir/src/elaborate.rs` | `Elaborator`、`elaborate` | 从 `T: Trait` 展开 supertraits 等逻辑后果 |
| `compiler/rustc_next_trait_solver/src/solve/assembly/mod.rs` | `assemble_param_env_candidates` | 将 caller bounds 作为 goal candidates |
| `compiler/rustc_trait_selection/src/traits/query/type_op/implied_outlives_bounds.rs` | `compute_implied_outlives_bounds_inner` | 从 assumed-WF types 提取 implicit implied outlives |
| `compiler/rustc_trait_selection/src/regions.rs` | `OutlivesEnvironment::new` | 合并 `ParamEnv` 中的 outlives 与签名 implied bounds |

## 源码精读

### 1. `ClauseKind` 是 `PredicateKind` 的可假设子集

位置：`compiler/rustc_type_ir/src/predicate_kind.rs`，`ClauseKind` 与 `PredicateKind`。

源码结构可压缩为：

```rust,ignore
pub enum ClauseKind<I: Interner> {
    Trait(TraitPredicate<I>),
    RegionOutlives(OutlivesPredicate<I, I::Region>),
    TypeOutlives(OutlivesPredicate<I, I::Ty>),
    Projection(ProjectionPredicate<I>),
    ConstArgHasType(I::Const, I::Ty),
    WellFormed(I::Term),
    ConstEvaluatable(I::Const),
    HostEffect(HostEffectPredicate<I>),
    UnstableFeature(I::Symbol),
}

pub enum PredicateKind<I: Interner> {
    Clause(ClauseKind<I>),
    DynCompatible(I::TraitId),
    Subtype(SubtypePredicate<I>),
    Coerce(CoercePredicate<I>),
    ConstEquate(I::Const, I::Const),
    Ambiguous,
    NormalizesTo(NormalizesTo<I>),
}
```

这个嵌套本身表达了集合关系：

```text
ClauseKind ⊂ PredicateKind
```

所有 clause 都能上转为 predicate，因为“已知 `T: Clone`”也可以被当作“请证明 `T: Clone`”。反向转换不总成立：`Subtype(?T0, ?T1)` 是类型检查过程产生的待解目标，不是通常能写进 item where-clause 并被无条件假设的 caller bound。

常见表面语法与 IR 对应为：

| Rust 表面约束 | 主要 IR |
|---|---|
| `T: Clone` | `ClauseKind::Trait` |
| `'a: 'b` | `ClauseKind::RegionOutlives(OutlivesPredicate('a, 'b))` |
| `T: 'a` | `ClauseKind::TypeOutlives(OutlivesPredicate(T, 'a))` |
| `<T as Iterator>::Item = U` | `ClauseKind::Projection` |
| `T` 必须良构 | `ClauseKind::WellFormed(T.into())` |
| `?T0 <: ?T1` | `PredicateKind::Subtype`，不是 clause |

`T: Iterator<Item = U>` 不是一个不可分割节点。它至少提供：

```text
Trait(T: Iterator)
Projection(<T as Iterator>::Item = U)
```

关联类型 binding 因而可以独立参与 normalization 和 projection candidate 选择。

### 2. `Predicate` 与 `Clause` 都保留 binder

位置：`compiler/rustc_middle/src/ty/predicate.rs`，`Predicate`、`Clause`、`Clause::kind`。

两者都是 interned 包装：

```rust,ignore
pub struct Predicate<'tcx>(
    Interned<'tcx, WithCachedTypeInfo<Binder<'tcx, PredicateKind<'tcx>>>>,
);

pub struct Clause<'tcx>(
    Interned<'tcx, WithCachedTypeInfo<Binder<'tcx, PredicateKind<'tcx>>>>,
);
```

`Clause` 在存储层复用 `PredicateKind` 的 interned 表示，但它的构造不变量保证内部只能是 `PredicateKind::Clause(_)`。因此 `Clause::kind` 可以安全地执行：

```rust,ignore
self.0.internee.map_bound(|kind| match kind {
    PredicateKind::Clause(clause) => clause,
    _ => unreachable!(),
})
```

这里没有直接存 `Binder<ClauseKind>`，主要是为了让同一条逻辑事实的 clause 视图与 predicate 视图共享一个 interned 节点：

```rust,ignore
pub fn Clause::as_predicate(self) -> Predicate<'tcx> {
    Predicate(self.0)
}

pub fn Predicate::expect_clause(self) -> Clause<'tcx> {
    match self.kind().skip_binder() {
        PredicateKind::Clause(..) => Clause(self.0),
        _ => bug!(...),
    }
}
```

因此转换关系是：

```text
Clause -> Predicate
  零分配、零 reintern：只换 newtype wrapper

Predicate -> Clause
  先检查内部确实为 PredicateKind::Clause，再换 wrapper
```

若 `Clause` 改为独立保存 `Interned<Binder<ClauseKind>>`，把它当作通用 `Predicate` 使用时就必须构造 `PredicateKind::Clause(kind)` 并在 predicate interner 中再查找/创建一个节点，或者维护另一种带间接引用的表示。当前设计选择统一 predicate interner，也因此 folding、visiting、encoding 可以复用 predicate 路径；例如 `Clause::fold_with` 实际是 `fold_predicate(self.as_predicate()).expect_clause()`。

所以这不是逻辑模型强制要求的唯一表示，而是一项实现取舍：

```text
逻辑层：ClauseKind 是 PredicateKind 的真子集
存储层：Clause 与 Predicate 共用 Binder<PredicateKind> 节点
类型层：Clause newtype + 构造/转换检查维持“只能含 Clause variant”的不变量
```

源码中的 `// FIXME(clause): This is wonky` 也表明维护者知道这种复用会让 `Clause` 的 fold 实现略显绕；它换来的直接收益是共享 interning 与廉价上转。

这里不能把 `Binder` 忽略掉。例如：

```rust,ignore
where for<'a> &'a T: IntoIterator
```

概念上是：

```text
Binder<ClauseKind::Trait(
    TraitPredicate(&'^0 T: IntoIterator)
)>
```

也就是说，where-clause 中的 HRTB 并不会因为进入 `ParamEnv` 就被实例化成某个具体 lifetime；它仍是一条 higher-ranked assumption，等 solver 使用它时再按量词规则处理。

### 3. HIR bounds 如何变成 clauses

位置：

- `compiler/rustc_hir_analysis/src/collect/predicates_of.rs`，`gather_explicit_predicates_of`
- `compiler/rustc_hir_analysis/src/hir_ty_lowering/bounds.rs`，`lower_bounds`

收集器处理两种常见 HIR 入口：

```rust,ignore
fn f<T: Clone>(...)             // inline bound
fn g<T>(...) where T: Clone     // where-clause
```

两者最终都会进入 clause 集合。当前源码还会插入一些并非逐字写出的约束：

```text
T                    -> 默认 T: Sized（除非使用 ?Sized 等放宽）
const N: usize       -> ConstArgHasType(N, usize)
trait Trait { ... }  -> predicates_of(Trait) 额外加入 Self: Trait
```

`lower_bounds` 对 trait bound 调用 `lower_poly_trait_ref`；对 outlives bound 则直接构造：

```rust,ignore
ClauseKind::TypeOutlives(OutlivesPredicate(param_ty, region))
```

并用 `Binder::bind_with_vars` 保留该 where predicate 引入的 bound vars。

因此 `explicit_predicates_of` 这个 query 名称应从“收集路径”理解：结果不只包含用户逐字写下的 token，还包含 lowering 阶段按语言规则加入、应当像用户 bounds 一样处理的 clauses，例如默认 `Sized`。

### 4. `predicates_of` 在 explicit 结果上继续补充信息

位置：`compiler/rustc_hir_analysis/src/collect/predicates_of.rs`，`predicates_of`。

核心控制流是：

```rust,ignore
let mut result = tcx.explicit_predicates_of(def_id);

let inferred_outlives = tcx.inferred_outlives_of(def_id);
result.predicates += inferred_outlives;

if tcx.is_trait(def_id) {
    result.predicates += Self: ThisTrait;
}
```

所以可以用以下包含关系理解：

```text
explicit_predicates_of(def_id)
  + 可物化的 inferred outlives
  + trait 自身的 Self: Trait
  = predicates_of(def_id)
```

这里的 inferred outlives 主要针对 ADT 与 lazy type alias。例如：

```rust,ignore
struct Ref<'a, T> {
    value: &'a T,
}
```

字段类型良构要求 `T: 'a`。对 ADT，这个要求会通过 outlives inference 物化为 definition predicates，调用者构造/使用 `Ref<'a, T>` 时必须满足它。

### 5. `GenericPredicates` 保留 parent 链，而不是立即复制所有父约束

位置：`compiler/rustc_middle/src/ty/generics.rs`，`GenericPredicates` 与 `instantiate_into`。

```rust,ignore
pub struct GenericPredicates<'tcx> {
    pub parent: Option<DefId>,
    pub predicates: &'tcx [(Clause<'tcx>, Span)],
}
```

其中 `predicates` 是当前 definition 自己的部分；完整实例化会先递归 parent：

```rust,ignore
if let Some(def_id) = self.parent {
    tcx.predicates_of(def_id).instantiate_into(tcx, instantiated, args);
}

instantiated.predicates.extend(
    self.predicates.iter().map(|(p, _)| {
        EarlyBinder::bind(tcx, *p).instantiate(tcx, args)
    }),
);
```

考虑：

```rust,ignore
impl<T: Clone> Wrapper<T> {
    fn map<U: Default>(&self) {}
}
```

方法 `map` 自己的 predicates 主要涉及 `U`，parent 指向 impl；对方法做完整实例化时先获得 impl 的 `T: Clone`，再加入方法自己的 `U: Default`。这就是第 03 章 generics parent chain 在约束层的对应物。

`instantiate_own` 只实例化当前层；`instantiate` 则递归包含 parent。阅读调用点时必须先确认它选择的是哪一个。

### 6. `tcx.param_env`：实例化、elaboration、normalization

位置：

- `compiler/rustc_ty_utils/src/ty.rs`，`param_env`
- `compiler/rustc_trait_selection/src/traits/mod.rs`，`normalize_param_env_or_error`

普通 item 的主路径可以缩写为：

```rust,ignore
let InstantiatedPredicates { predicates, .. } =
    tcx.predicates_of(def_id).instantiate_identity(tcx);

let unnormalized_env = ParamEnv::new(tcx.mk_clauses(&predicates));

normalize_param_env_or_error(tcx, unnormalized_env, cause)
```

`instantiate_identity` 的意义是把 item 的 early-bound params 保持为自己的 identity args，同时递归纳入 parent predicates。它并不是把 `T` 变成某个调用点的 `u32`。

`normalize_param_env_or_error` 首先调用 `util::elaborate`。例如：

```rust,ignore
trait Base {}
trait Child: Base {}
fn f<T: Child>() {}
```

未展开环境包含 `T: Child`，elaboration 会补入 `T: Base`。它也会用 visited set 去重，避免循环 supertraits 导致无限展开。

随后，旧 solver 路径还要求对环境中的 aliases 做 normalization；next solver 使用 lazy normalization，不具有同样的“先把所有 caller bounds 完全归一化”要求。这里应把 normalization 看成当前实现阶段的环境准备步骤，而不是 `ParamEnv` 的抽象定义。

`ParamEnv` 本身非常小：

```rust,ignore
pub struct ParamEnv<'tcx> {
    caller_bounds: Clauses<'tcx>,
}
```

它表达的是当前作用域的逻辑前提，不包含全局 impl 数据库，也不包含正在证明的目标。

### 7. solver 如何使用 caller bounds

位置：`compiler/rustc_next_trait_solver/src/solve/assembly/mod.rs`，`assemble_param_env_candidates`。

当前源码的关键循环非常直接：

```rust,ignore
for assumption in goal.param_env.caller_bounds().iter() {
    match probe_and_consider_param_env_candidate(goal, assumption)? {
        Ok(candidate) => candidates.push(candidate),
        Err(...) => { /* 记录不适用原因 */ }
    }
}
```

这说明 `ParamEnv` 并不是在 goal 创建时就把答案“查出来”，而是 candidate assembly 时的一类候选来源。对于同一个 predicate：

```text
goal 1: prove T: Clone under [T: Clone]  -> 可由 ParamEnv candidate 成功
goal 2: prove T: Clone under []         -> 不能使用该候选，可能失败或依赖其他 impl
```

因此缓存和 canonical query 通常不能只以 `Predicate` 为 key；环境会影响证明结果，常见组合就是 `ParamEnvAnd<Predicate>`。

这里的“遍历 caller bounds”不是说一个 goal 只能由一条 clause 独立完成。更准确地说，每条 `assumption` 都先被当作一个可能的候选规则，求解器尝试把当前 goal 的“头部”与该 assumption 匹配。

对 trait goal 而言，`fast_reject_assumption` 会先筛掉 `ClauseKind`、trait `DefId` 或参数形状明显不匹配的 assumption。进入 `match_assumption` 后：

```rust,ignore
let assumption_trait_pred = instantiate_binder_with_infer(trait_clause);
eq(goal.predicate.trait_ref, assumption_trait_pred.trait_ref)?;
then(ecx)
```

也就是说，环境中的：

```text
assumption: T: Clone
goal:       T: Clone
```

匹配成功后，这条 assumption 本身就能形成一个 candidate。

但复合依赖不是在 `assemble_param_env_candidates` 这一层一次性展开完成的。它由产生 candidate 的那条规则把 requirements 加入 `EvalCtxt`，再通过 `try_evaluate_added_goals` / `evaluate_added_goals_and_make_canonical_response` 递归证明。

例如标准库 impl 可概念化为：

```text
impl<T: Clone> Clone for Vec<T>

head:        Vec<T>: Clone
requirement: T: Clone
```

证明 `Vec<X>: Clone` 时，impl candidate 匹配 head，并添加 nested goal `X: Clone`；随后 nested goal 再可能由 `ParamEnv` 中的 `X: Clone` candidate 证明。整个证明树是多步的：

```text
goal Vec<X>: Clone
  candidate impl<T: Clone> Clone for Vec<T>
    nested goal X: Clone
      candidate ParamEnv assumption X: Clone
```

所以 `ParamEnv` 遍历的是“这一层 goal 有哪些环境 assumption 可以直接作为候选”。如果当前 goal 依赖多个条件，这些条件会表现为 added goals，而每个 added goal 又会独立走候选组装流程。

同样，`T: Iterator<Item = U>` 在环境里通常不是单条不可拆的巨大 premise，而是至少有两条可独立使用的 clause：

```text
Trait(T: Iterator)
Projection(<T as Iterator>::Item = U)
```

当 goal 是 `T: Iterator`，前者能匹配；当 goal 是 normalize `<T as Iterator>::Item` to `U`，后者能匹配。它们分别作为不同 goal 的 candidate 出现。

#### 依赖多个 clause 的证明流程

考虑：

```rust,ignore
struct Pair<A, B>(A, B);
trait Show {}

impl<A, B> Show for Pair<A, B>
where
    A: Clone,
    B: std::fmt::Debug,
{}

fn f<X, Y>(value: Pair<X, Y>)
where
    X: Clone,
    Y: std::fmt::Debug,
{
    needs_show::<Pair<X, Y>>();
}
```

在 `needs_show::<Pair<X, Y>>()` 处，核心 goal 是：

```text
goal: Pair<X, Y>: Show

param_env:
  X: Clone
  Y: Debug
```

solver 不会在同一层从 `ParamEnv` 中同时取出 `X: Clone` 和 `Y: Debug` 来“合成”出 `Pair<X, Y>: Show`。第一层要先找到一个 head 能匹配当前 goal 的 candidate：

```text
impl<A, B> Show for Pair<A, B>
where
    A: Clone,
    B: Debug
```

impl candidate 的处理流程是：

```text
1. 为 impl generics 创建 fresh args：
   A -> ?A
   B -> ?B

2. 实例化 impl self type：
   Pair<?A, ?B>: Show

3. 与当前 goal 做 eq：
   Pair<?A, ?B>: Show == Pair<X, Y>: Show

   得到：
   ?A = X
   ?B = Y

4. 实例化 impl where-clauses，并加入 nested goals：
   ?A: Clone  -> X: Clone
   ?B: Debug  -> Y: Debug

5. 分别证明 nested goals。
```

形成的证明树是：

```text
prove Pair<X, Y>: Show
  candidate: impl<A, B> Show for Pair<A, B>
    nested goal: X: Clone
      candidate: ParamEnv assumption X: Clone
    nested goal: Y: Debug
      candidate: ParamEnv assumption Y: Debug
```

如果两个 nested goals 都是 `Yes`，impl candidate 才能给原 goal 产出 `Yes`。如果其中一个失败，则这个 impl candidate 失败；如果其中一个 ambiguous，则原 candidate 的结果也会带上 ambiguity。

源码对应关系：

```text
consider_impl_candidate
  fresh_args_for_item
  eq(goal.trait_ref, impl_trait_ref)
  predicates_of(impl).iter_instantiated(...)
  add_goals(GoalSource::ImplWhereBound, where_clause_bounds)
  evaluate_added_goals_and_make_canonical_response(...)

evaluate_added_goals_and_make_canonical_response
  try_evaluate_added_goals
    evaluate_added_goals_step
      evaluate_goal(...)
```

因此，“一个 goal 依赖多个 clause”在实现里表现为：

```text
一个 candidate 匹配当前 goal 的 head
  -> 产生多个 nested goals
    -> 每个 nested goal 各自寻找 candidate
      -> 结果汇总回父 candidate
```

#### impl candidate 如何寻找

impl candidate 的寻找可以分成两层：

```text
candidate assembly:
  找出“可能相关”的 impl_def_id

candidate confirmation / consideration:
  对每个 impl_def_id 实例化、匹配 head、检查 where-clauses
```

next solver 在 `assemble_impl_candidates` 中调用：

```rust,ignore
cx.for_each_relevant_impl(goal.predicate.trait_ref(cx), |impl_def_id| {
    if cx.impl_is_default(impl_def_id) {
        return Ok(());
    }
    consider_impl_candidate(goal, impl_def_id, ...)
})
```

`for_each_relevant_impl` 不会无脑遍历全 crate 的所有 impl。它先用 goal 的 trait `DefId` 找到该 trait 的 impl 表：

```text
trait_impls_of(Trait)
  blanket_impls
  non_blanket_impls: SimplifiedType -> Vec<ImplDefId>
```

`trait_impls_of_provider` 构造这张表时，会收集上游 crate 和本 crate 对该 trait 的 impl。对每个 impl，它取 impl self type，尝试做 `simplify_type`：

```text
impl Clone for Vec<T>     -> SimplifiedType::Adt(Vec)
impl Clone for Option<T>  -> SimplifiedType::Adt(Option)
impl<T> Clone for T where ... -> 无法简化成具体外层类型，进入 blanket_impls
```

查询 `Vec<X>: Clone` 时：

```text
trait_def_id = Clone
self_ty      = Vec<X>
simplified   = Adt(Vec)
```

因此候选枚举大致是：

```text
1. 所有 blanket impls of Clone
2. non_blanket_impls[Adt(Vec)]
```

不会去看 `Option<T>: Clone`、`Box<T>: Clone`、`String: Clone` 这些外层 self type 明显不匹配的 impl。这个索引就是 impl candidate 搜索的第一道粗筛。

这里的 `blanket_impls` 不是“只要 impl 有泛型参数就算 blanket”。rustc 在这张表里使用的是“能不能按 self type 的外层形状建立索引”：

```text
impl<T> Clone for Vec<T>
  self type 外层是 Vec
  simplify_type -> Adt(Vec)
  放入 non_blanket_impls[Adt(Vec)]

impl<T> Trait for T where T: Copy
  self type 外层是参数 T
  无法按具体外层类型索引
  放入 blanket_impls

impl<T> Trait for <T as SomeTrait>::Assoc
  self type 外层是 projection
  通常无法按具体外层类型索引
  放入 blanket_impls
```

所以：

```text
generic impl
  = 源码层面带泛型参数的 impl

blanket impl bucket
  = rustc impl 索引中不能按具体 self type 外层形状归桶、必须更广泛考虑的 impl
```

`impl<T> Clone for Vec<T>` 是泛型 impl，但不是这个意义上的 blanket impl bucket。它可以被 `Vec<_>` 这个外层 self type 索引，所以会落入 `non_blanket_impls[Adt(Vec)]`。

如果 self type 不能安全简化，旧 `TyCtxt::for_each_relevant_impl` 会退化为枚举所有 non-blanket impls；next solver 的 interner 版本更细一些，例如对整数/浮点推理变量会枚举所有具体整数/浮点桶，对 alias/placeholder 通常只考虑 blanket impls，因为 normalization 后适用的 impl 会在 normalize self type 的路径里处理。

拿到 `impl_def_id` 后，还没有证明它真的适用。`consider_impl_candidate` 会做第二道筛选：

```text
1. 读取 impl_trait_ref。
2. 用 DeepRejectCtxt::args_may_unify 快速判断 goal args 和 impl args 是否可能统一。
3. 检查 impl polarity：positive/negative/reservation impl 是否能用于当前 goal。
4. 在 probe 中为 impl generics 创建 fresh args。
5. 实例化 impl_trait_ref。
6. 用 eq(goal.trait_ref, impl_trait_ref) 真正匹配 head。
7. 把 impl 的 predicates_of(impl_def_id) 实例化后作为 nested goals。
8. 评估 nested goals，得到 candidate response。
```

所以完整流程可以压缩成：

```text
goal Pair<X, Y>: Show
  trait_impls_of(Show)
    blanket_impls
    non_blanket_impls[SimplifiedType::Adt(Pair)]
  for each impl_def_id:
    quick reject by args_may_unify
    instantiate impl args
    eq goal head with impl head
    add impl where-clauses as nested goals
    evaluate nested goals
```

这也解释了为什么 solver 不需要一开始就“知道”哪个 impl 最终成立。candidate assembly 只是找可能相关的 impl；真正成立与否，要等 head unification 和 nested obligations 都走完。

#### `add_goals` 如何把 where-clauses 变成 goals

`add_goals` 本身并不理解“where-clause”。它接收的参数已经是 `Goal<I, I::Predicate>` 迭代器：

```rust,ignore
fn add_goals(
    &mut self,
    source: GoalSource,
    goals: impl IntoIterator<Item = Goal<I, I::Predicate>>,
) {
    for goal in goals {
        self.add_goal(source, goal)?;
    }
}
```

真正把 impl where-clauses 包成 goals 的地方在 `consider_impl_candidate`：

```rust,ignore
let where_clause_bounds = cx
    .predicates_of(impl_def_id.into())
    .iter_instantiated(cx, impl_args)
    .map(Unnormalized::skip_norm_wip)
    .map(|pred| goal.with(cx, pred));

ecx.add_goals(GoalSource::ImplWhereBound, where_clause_bounds)?;
```

拆开看是四步：

```text
1. cx.predicates_of(impl_def_id)
   取出 impl definition 上的 predicates。

2. iter_instantiated(cx, impl_args)
   用当前 impl candidate 的 fresh args 实例化这些 predicates。

3. skip_norm_wip
   取出尚未显式 normalizing 的 Clause/Predicate 内容。

4. goal.with(cx, pred)
   构造一个新 Goal：
     param_env = 原 goal.param_env
     predicate = pred
```

`Goal::with` 的定义就是“保留同一个 `param_env`，只替换 `predicate`”：

```rust,ignore
Goal { param_env: self.param_env, predicate: predicate.upcast(cx) }
```

所以如果原始 goal 是：

```text
Goal {
  param_env: [X: Clone, Y: Debug],
  predicate: Pair<X, Y>: Show,
}
```

impl 是：

```rust,ignore
impl<A, B> Show for Pair<A, B>
where
    A: Clone,
    B: Debug,
{}
```

在 candidate probe 中：

```text
impl_args:
  A -> ?A
  B -> ?B

head eq:
  Pair<?A, ?B>: Show == Pair<X, Y>: Show

constraints:
  ?A = X
  ?B = Y
```

where-clauses 先被实例化为：

```text
?A: Clone
?B: Debug
```

在同一个 probe/inference 状态下，它们等价于：

```text
X: Clone
Y: Debug
```

然后 `goal.with(cx, pred)` 得到 nested goals：

```text
Goal {
  param_env: [X: Clone, Y: Debug],
  predicate: ?A: Clone,
}

Goal {
  param_env: [X: Clone, Y: Debug],
  predicate: ?B: Debug,
}
```

注意这里 nested goal 的 `param_env` 仍然是调用点的环境，而不是 impl 自己的 where-clauses。impl 的 where-clauses 是要被证明的 requirements；调用点的 `param_env` 是证明这些 requirements 时可用的 assumptions。

进入 `add_goal` 后，还会做三件事：

```text
1. normalize nested goal 的 predicate；
2. 记录到 proof/inspect tree；
3. 尝试 fast path，不能立刻完成则压入 nested_goals 队列。
```

随后 `evaluate_added_goals_and_make_canonical_response` 调用 `try_evaluate_added_goals`，逐个 `evaluate_goal` 这些 nested goals，并把结果汇总回当前 impl candidate。

#### 定义位置泛型如何和 `ParamEnv` 中的类型对应

这里要分两种情况。

第一种是在同一个 item/body 内部：

```rust,ignore
fn f<T: Clone>(x: T) {
    needs_clone::<T>();
}
```

`tcx.param_env(f_def_id)` 构造时会取：

```text
tcx.predicates_of(f_def_id).instantiate_identity(tcx)
```

因此 `ParamEnv` 中的 caller bound 是用该 item 的 identity params 表达的：

```text
ParamEnv:
  Param(T#0): Clone
```

函数体中 `x: T`、`needs_clone::<T>()` 里的 `T` 也是同一个 definition 的 `Param(T#0)`。也就是说，在同一个 item 内部并不需要额外“匹配名字”；IR 里它们本来就是同一个 param identity：

```text
goal:
  Param(T#0): Clone

param_env assumption:
  Param(T#0): Clone
```

所以 `assemble_param_env_candidates` 扫到这条 assumption 后，`match_assumption` 对 trait ref 做 `eq`，二者直接相等。

第二种是 impl candidate 的定义位置泛型：

```rust,ignore
impl<A, B> Show for Pair<A, B>
where
    A: Clone,
    B: Debug,
{}

fn f<X: Clone, Y: Debug>() {
    needs_show::<Pair<X, Y>>();
}
```

这里 impl 定义位置的 `A`、`B` 和调用点的 `X`、`Y` 不是同一个 param identity。solver 不会按名字对应，也不会把 `A` 直接当作 `X`。

`consider_impl_candidate` 会先为 impl generics 创建 fresh args：

```text
A -> ?A
B -> ?B
```

然后实例化 impl head：

```text
impl head:
  Pair<?A, ?B>: Show
```

再与当前 goal 做 `eq`：

```text
goal:
  Pair<X, Y>: Show

eq:
  Pair<?A, ?B>: Show == Pair<X, Y>: Show

产生约束：
  ?A = X
  ?B = Y
```

接着 impl where-clauses 用同一组 `impl_args` 实例化：

```text
A: Clone -> ?A: Clone
B: Debug -> ?B: Debug
```

这些 nested goals 与 head matching 产生的约束存在于同一个 probe/inference 状态中，所以证明 `?A: Clone` 时，solver 可以通过 `?A = X` 把它和当前 `ParamEnv` 中的 `X: Clone` 对上；`?B: Debug` 同理。

因此对应关系不是：

```text
impl 的 A 按名字映射到调用点的 X
```

而是：

```text
impl 的 A
  -> fresh var ?A
  -> 通过 impl head 和 goal head 的 eq 约束为 X
  -> nested goal ?A: Clone 在同一 inference state 中由 ParamEnv 的 X: Clone 证明
```

这里 `ParamEnv` 没有被改写成包含 `?A: Clone`。`ParamEnv` 仍然是调用点环境：

```text
ParamEnv:
  X: Clone
  Y: Debug
```

被改写/约束的是 impl candidate probe 里的 inference variables。这个设计让同一个 impl 可以尝试匹配不同 goal，而不需要在全局环境中复制或重命名泛型参数。

#### `Self: Trait` clause 在哪里引入

对 trait 本身，`Self: Trait` 不是用户显式写出来的 where-clause，而是 `predicates_of` 查询主动补进去的。

位置：`compiler/rustc_hir_analysis/src/collect/predicates_of.rs`，`predicates_of`。

当前实现是：

```rust,ignore
if tcx.is_trait(def_id) {
    result.predicates = tcx.arena.alloc_from_iter(
        result
            .predicates
            .iter()
            .copied()
            .chain(std::iter::once((
                ty::TraitRef::identity(tcx, def_id).upcast(tcx),
                DUMMY_SP,
            ))),
    );
}
```

`ty::TraitRef::identity(tcx, def_id)` 对 trait `Trait` 来说就是概念上的：

```text
Self: Trait
```

源码注释说明了原因：这不是用户写的 predicate，但调用 trait method 或投影 associated type 时，必须证明 trait 确实适用于当前 self type。把它放进 trait 的 `predicates_of`，能让 rustc 在使用 trait item 时自然产生这项要求。

例如：

```rust,ignore
trait Trait {
    type Assoc;
    fn method(&self) {}
}
```

trait definition 的 `predicates_of(Trait)` 会包含：

```text
Self: Trait
```

随后 `tcx.param_env(def_id)` 会从 `tcx.predicates_of(def_id).instantiate_identity(tcx)` 构造环境。因此 trait 自身或 trait item/default method 继承父 predicates 后，就能在对应环境中看到这条 assumption。

这个 `Self: Trait` 的主要用途是“trait/trait item 自己的环境”和“使用 trait item 时应产生的要求”，不是“用户写了 `T: Trait` 后，solver 再去读取普通 `predicates_of(Trait)` 来证明它”。

当用户写：

```rust,ignore
fn f<T: Trait>(x: T) {}
```

lowering 会直接在当前 item 的 predicates 中得到：

```text
T: Trait
```

`tcx.param_env(f)` 里也就是这条 `T: Trait`。证明 `T: Trait` 时，`ParamEnv` candidate 可以直接匹配这条 assumption，不需要读取 trait definition 的普通 `predicates_of(Trait)`，也不会靠 trait 自身的 `Self: Trait` 来证明 `T: Trait`。如果这么做会变成一种循环的“Trait 成立因为 Trait 的 predicates 里有 Self: Trait”。

不过，`T: Trait` 会触发另一类 trait-definition 查询：elaboration。对 `ClauseKind::Trait(T: Trait)` 做 elaboration 时，源码调用的是：

```rust,ignore
cx.explicit_implied_predicates_of(Trait)
// 或只关心 self predicates 时：
cx.explicit_super_predicates_of(Trait)
```

也就是说，它读取的是 trait header 和关联类型 bounds 所蕴含的 predicates，例如 supertrait：

```rust,ignore
trait Trait: Base {}
```

从 `T: Trait` elaboration 得到：

```text
T: Base
```

这里用的不是普通 `predicates_of(Trait)` 中那条 identity `Self: Trait`，而是专门的 implied/super predicates 查询。

supertrait bounds 不是由这条 `Self: Trait` 直接硬编码出来的，而是通过 `explicit_super_predicates_of` / `implied_predicates_with_filter` 降低 trait header 上的 superbounds。那里用 `tcx.types.self_param` 作为 self type 调 `lower_bounds`，例如：

```rust,ignore
trait Child: Base {}
```

会降低出概念上的：

```text
Self: Base
```

然后 elaboration 可以在具体环境中把 `T: Child` 推出 `T: Base`。

所以有三件事要分开：

```text
trait 自身的 identity predicate
  Self: Trait
  由 predicates_of(trait_def_id) 主动追加

trait header 的 supertrait predicate
  Self: Base
  由 explicit_super_predicates_of / lower_bounds 处理 supertraits 得到

具体类型的 supertrait 后果
  T: Child -> T: Base
  由 ParamEnv elaboration 得到
```

### 8. `ParamEnv::empty()` 的真实使用边界

位置：`compiler/rustc_middle/src/ty/mod.rs`，`ParamEnv::empty`。

源码注释说得很直接：`ParamEnv::empty()` 适用于“没有 where-clauses in scope”的上下文，并且大多数情况下使用空环境都是不正确的。它不是一个方便的默认值，而是在明确表达：

```text
本次证明/归一化/等价检查不允许使用任何 caller bounds。
```

真实调用大致分为几类。

#### 入口函数与特殊签名检查

`compiler/rustc_hir_analysis/src/check/entry.rs` 检查 `main` 的返回类型时使用空环境。源码旁边也有注释：

```rust,ignore
// Main should have no WC, so empty param env is OK here.
let param_env = ty::ParamEnv::empty();
```

`main` 不能依赖某个泛型 `where` 环境来让签名成立，因此检查 `main` 的 `Termination` 相关约束时不应带入 caller bounds。

类似地，`compiler/rustc_hir_analysis/src/check/mod.rs` 的函数签名匹配检查，以及 `compiler/rustc_passes/src/check_attr.rs` 的 proc macro 签名检查，会用空环境比较“语言要求的签名”和“用户写的签名”。这类检查关心的是固定 ABI/入口/属性规则，不是某个泛型函数体内部的假设集。

#### 全局 trivial bounds 检查

`compiler/rustc_hir_analysis/src/check/wfcheck.rs` 的 `check_false_global_bounds` 会筛出 global predicate：

```rust,ignore
if pred.is_global() && !pred.has_type_flags(TypeFlags::HAS_BINDER_VARS) {
    Obligation::new(..., empty_env, pred)
}
```

这里的意图是检查类似“全局上是否真的成立”的 trivial bound。既然 predicate 不依赖当前泛型参数，就不应让当前 item 的 caller bounds 参与证明；否则会把“全局事实”误判成“在某个局部假设下成立”。

#### Coherence、overlap 与 orphan check

`compiler/rustc_trait_selection/src/traits/coherence.rs` 在 impl overlap 检查中使用空环境。源码注释解释了原因：为了这个检查，会把 impl 泛型参数换成 fresh inference variables，而不是把 placeholder types 放入作用域，因此 evaluation 在 empty environment 中进行。

`compiler/rustc_hir_analysis/src/coherence/orphan.rs` 的 orphan check 也会在 normalization 或把 fresh vars 映射回 identity params 时使用空环境。coherence 检查关心的是 impl 头之间是否可能重叠、是否满足孤儿规则；它不能借用某个 impl 自己的 where-clauses 来证明会改变全局一致性判断的事实。

#### lint 的模块/全 crate 上下文

`compiler/rustc_lint/src/late.rs` 初始化 module-level 或 crate-level `LateContext` 时使用：

```rust,ignore
param_env: ty::ParamEnv::empty()
```

模块或 crate 本身没有某个具体 item 的泛型 where-clause 作用域。进入具体 item/body 后，lint context 才会切换到对应 item 的环境。

#### 受限的 const evaluation 与结构匹配

`compiler/rustc_trait_selection/src/traits/mod.rs` 在处理 min const generics 的 anon const 时使用空环境，旁边注释也承认这件事需要理由：只有非常受限的 const 参数会走这里，因此空环境可接受。

`compiler/rustc_ty_utils/src/structural_match.rs` 检查 ADT 自身是否实现内部的 `StructuralPartialEq` lang item 时，也用空环境注册 bound。这个检查问的是“这个 ADT 类型本身是否有 derive 注入的结构相等实现”，不是“在当前泛型假设下能否推出某个结构相等性质”。

总结成一条经验：

```text
用 tcx.param_env(def_id)
  当检查发生在某个 item / body / impl / trait 的泛型假设作用域内。

用 ParamEnv::empty()
  当检查是全局的、语言固定规则的、coherence 用的，
  或者调用点已经明确证明不需要任何 caller bounds。
```

### 9. 两类 implied outlives 不走同一条存储路径

位置：

- `compiler/rustc_hir_analysis/src/outlives/mod.rs`，`inferred_outlives_of`
- `compiler/rustc_trait_selection/src/traits/query/type_op/implied_outlives_bounds.rs`，`compute_implied_outlives_bounds_inner`
- `compiler/rustc_trait_selection/src/regions.rs`，`OutlivesEnvironment::new`

#### ADT/lazy alias：物化到 `predicates_of`

```rust,ignore
struct Ref<'a, T>(&'a T);
```

`T: 'a` 是该类型定义自身良构所需的约束。`inferred_outlives_of` 对 ADT/lazy alias 计算这类约束，`predicates_of` 将其追加到 definition predicates。

#### 函数签名/impl header：从 assumed-WF types 隐式提取

```rust,ignore
fn use_ref<'a, T>(x: &'a T) {
    requires_outlives::<'a, T>();
}

fn requires_outlives<'a, T: 'a>() {}
```

函数体能使用 `T: 'a`，因为调用者必须保证输入类型 `&'a T` 良构。但当前实现不会简单地把这条约束加入该函数自身的 `ParamEnv::caller_bounds`。

`OutlivesEnvironment::new` 分别收集：

1. `ParamEnv` 中显式或已物化的 `TypeOutlives` clauses；
2. 从 `assumed_wf_tys` 调用 `implied_bounds_tys` 得到的 implicit implied bounds；
3. higher-ranked region assumptions。

`compute_implied_outlives_bounds_inner` 会先 normalize 类型，再请求其 WF obligations，并只提取 `RegionOutlives`、`TypeOutlives`，递归跟进 `WellFormed`。其他 trait、projection、subtype 等 predicates 在这一步不会被当作 implied outlives 结果。

这与 Rust 的语言规则一致：由类型良构性自动获得的是 lifetime bounds，不会自动获得任意 trait bounds。例如：

```rust,ignore
use std::fmt::Debug;

struct NeedsDebug<T: Debug>(T);

fn bad<T>(x: NeedsDebug<T>) {}
// 仍需显式写 T: Debug
```

不要把以下三件事混为一谈：

| 现象 | 实现动作 | 进入普通 `ParamEnv` caller bounds？ |
|---|---|---:|
| `T: Child` 推出 `T: Base` | supertrait elaboration | 是 |
| ADT 字段 `&'a T` 要求 `T: 'a` | inferred outlives，追加到 `predicates_of` | 是，作为 definition predicate |
| 函数输入 `&'a T` 让函数体可假设 `T: 'a` | assumed-WF type 的 implicit implied outlives | 通常否，进入 outlives/region 环境 |

## 从一段源码手算完整环境

考虑：

```rust,ignore
trait Base {}
trait Stream: Base {
    type Item;
}

fn consume<'a, T, U>(x: &'a T)
where
    T: Stream<Item = U> + 'a,
    U: Clone,
{}
```

### 第一步：lower own clauses

忽略顺序和内部 `DefId/args` 细节，可得到：

```text
Trait(T: Sized)                         // 默认 bound
Trait(U: Sized)                         // 默认 bound
Trait(T: Stream)
Projection(<T as Stream>::Item = U)
TypeOutlives(T: 'a)
Trait(U: Clone)
```

### 第二步：elaborate

因为 `Stream: Base`：

```text
Trait(T: Stream)
  -> Trait(T: Base)
```

去重后的 caller bounds 因而还包含 `T: Base`。

### 第三步：在环境中证明目标

若函数体产生 goal `T: Base`，可直接选择 `ParamEnv` candidate。若产生 goal `Vec<U>: Clone`：

```text
Vec<U>: Clone
  -> 选择 impl<T: Clone> Clone for Vec<T>
  -> nested goal U: Clone
  -> 使用 ParamEnv 中 U: Clone
```

若调用点把 `T/U` 实例化成具体类型，则调用点需要证明 `consume` 的实例化 predicates；函数体内部使用的 identity `ParamEnv` 与调用点证明具体 bounds 是两个不同视角。

## `ParamEnv::empty()` 何时成立

`ParamEnv::empty()` 表示没有 in-scope where clauses。它通常适合：

- 已完全单态化的 codegen 场景；
- 明确不允许/不预期出现泛型参数的某些分析；
- 某些 coherence 检查刻意选择的开放世界语义。

它不是“调用 API 时不知道传什么”的默认值。在泛型 item 内把正确环境替换为空环境，最直接的后果是原本能由 `T: Clone` caller bound 完成的 goal 变成无法证明；在 normalization、const evaluation 等路径上还可能导致更隐蔽的错误。

## 常见误区

### 误区 1：`Predicate` 等于 where-clause

where-clause 通常降低为 `Clause`；`Predicate` 的范围更大，还包括 `Subtype`、`Coerce`、`DynCompatible`、`ConstEquate`、`NormalizesTo` 等求解目标。

### 误区 2：`ParamEnv` 保存当前所有待处理 obligations

`ParamEnv` 保存假设。待处理工作由 goal/obligation/fulfillment machinery 管理，这属于下一章。

### 误区 3：`T: Iterator<Item = U>` 只产生一个 trait clause

trait implemented 与 associated item equality 是不同逻辑事实，lowering 会形成 trait 与 projection clauses。

### 误区 4：`predicates_of` 已经是最终 `ParamEnv`

`predicates_of` 返回带 parent/spans 的 `GenericPredicates`；还需要实例化 parent 链、构造环境、elaborate，且按 solver 路径处理 normalization。

### 误区 5：所有“隐含 bound”都进入 `ParamEnv`

supertrait elaboration 和 ADT inferred outlives 会体现在 caller bounds；函数签名 assumed-WF 所带来的 implicit implied outlives 通常进入专门的 outlives/region 环境。

### 误区 6：函数参数使用某个受约束类型，就能隐含任意 trait bound

当前语言规则只隐含良构性所需的 lifetime bounds；trait bounds 仍需显式声明。

### 误区 7：进入 `ParamEnv` 会消除 `for<'a>` binder

clauses 自身是 binder-aware 的。higher-ranked clause 仍表示“对所有 `'a`”，只有 solver 使用它时才按 higher-ranked 规则实例化。

## 本章小结

1. `Predicate` 是通用的可证明命题；`Clause` 是可放进假设环境的 predicate 子集。
2. `GenericPredicates` 保存某个 definition 的 own clauses、parent 和 spans；完整实例化会先递归 parent。
3. `tcx.param_env(def_id)` 从 identity-instantiated predicates 构造 caller bounds，再进行 elaboration、去重与所需 normalization。
4. `ParamEnv` 是假设集；solver 在 candidate assembly 时逐条尝试这些 assumptions。
5. `T: Iterator<Item = U>` 通常拆成 trait clause 与 projection clause。
6. supertrait elaboration、ADT inferred outlives、函数签名 implicit implied outlives 是三条不同的数据路径。
7. 同一个 predicate 在不同 `ParamEnv` 中可能有不同证明结果，因此环境是 goal 语义的一部分。
