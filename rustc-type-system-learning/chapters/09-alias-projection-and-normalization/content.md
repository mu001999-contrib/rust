---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "09"
document: content
status: completed
updated_at: 2026-08-12
---

# 09. Alias、Projection 与 Normalization

## 学习目标

完成本章后，应当能够：

1. 从 Type IR 中识别 `TyKind::Alias`、`AliasTy`、`AliasTerm` 与不同 alias kind。
2. 区分普通 type alias 的展开、trait associated type projection、inherent associated type、opaque type 与 free alias。
3. 把 `<T as Trait>::Assoc` 翻译成 `AliasTy { kind, args }`，并说明 `args` 的排列来源。
4. 区分“一个 alias term”与“该 alias 应归一化为某个 term”的 `ProjectionPredicate`。
5. 追踪 projection 从 `ParamEnv`、trait definition、trait object 或 impl 获得结果的过程。
6. 解释 normalization 为什么可能返回“值 + obligations”，以及 ambiguity 时为什么会引入推理变量并延迟证明。
7. 区分类型检查期的 obligation-producing normalization 与后期的 `normalize_erasing_regions`。
8. 理解 eager normalization 与 lazy normalization 的边界，以及当前 new solver 中内部 `NormalizesTo` goal 的作用。

## 前置知识

- 第 03 章：`GenericArgs`、parent generics 与 `rebase_onto`。
- 第 05 章：推理变量、snapshot、解析与回滚。
- 第 07 章：`ClauseKind::Projection` 与 `ParamEnv`。
- 第 08 章：obligation、nested obligations 与 fulfillment。

## 核心心智模型

alias 是一个“目前仍以名字表示、可能需要根据环境计算其实际 term”的 IR 节点：

```text
Alias = kind + def_id + args
```

例如：

```rust,ignore
trait Iterable {
    type Item;
}

impl<T> Iterable for Vec<T> {
    type Item = T;
}
```

`<Vec<u32> as Iterable>::Item` 在归一化前可概念化为：

```text
TyKind::Alias(
    IsRigid::No,
    AliasTy {
        kind: Projection { def_id: Iterable::Item },
        args: [Vec<u32>],
    },
)
```

在当前环境中选择到 `impl<T> Iterable for Vec<T>` 后：

```text
<Vec<u32> as Iterable>::Item
    normalize
        ↓
u32
```

normalization 不是单纯的文本替换。它的完整结果通常是：

```text
Normalized<T> = value + obligations
```

如果 impl 是：

```rust,ignore
impl<T: Decode> Iterable for Container<T> {
    type Item = T::Output;
}
```

那么 normalization 可能同时需要：

```text
选择 impl
  + 证明 T: Decode
  + 继续归一化 T::Output
```

本章可以用一条主线串起来：

```text
Alias term
  “要计算哪个名字？”
        ↓
Projection / NormalizesTo goal
  “它应当等于哪个 term？”
        ↓
candidate assembly
  ParamEnv / trait definition / object / impl
        ↓
candidate confirmation
  实例化 args，取得关联项定义
        ↓
normalized value + nested obligations
```

## 源码地图

| 路径 | 关键符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_type_ir/src/ty_kind.rs` | `TyKind::Alias`、`AliasTyKind`、`IsRigid` | alias type 的核心 IR 分类 |
| `compiler/rustc_type_ir/src/ty/alias.rs` | `Alias`、`AliasTy`、`AliasTerm` | `kind + args` 的统一载体 |
| `compiler/rustc_type_ir/src/term_kind.rs` | `TermKind`、`AliasTermKind` | 同时覆盖 type alias 与 const alias |
| `compiler/rustc_type_ir/src/predicate.rs` | `ProjectionPredicate`、`NormalizesTo` | “alias 等于 term”的逻辑表示 |
| `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs` | `lower_path_segment` | 普通 type alias 与 checked/free alias 的 lowering 边界 |
| `compiler/rustc_trait_selection/src/traits/normalize.rs` | `NormalizeExt::normalize`、`AssocTypeNormalizer` | old solver 中的深层遍历与 alias 分派 |
| `compiler/rustc_trait_selection/src/traits/project.rs` | `normalize_projection_term`、`project`、`confirm_candidate` | old solver 的 projection 候选搜索与确认 |
| `compiler/rustc_trait_selection/src/solve/normalize.rs` | `normalize_with_universes`、`ReplaceAliasWithInfer` | new solver 与 typeck 之间的 normalization 桥梁 |
| `compiler/rustc_next_trait_solver/src/solve/project_goals/mod.rs` | `compute_projection_goal`、`normalize_associated_term` | new solver 的 projection goal 入口 |
| `compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs` | `compute_normalizes_to_goal`、`consider_impl_candidate` | 内部 `NormalizesTo` 的候选计算 |
| `compiler/rustc_middle/src/ty/normalize_erasing_regions.rs` | `normalize_erasing_regions` | 推理结束后的查询式归一化 |

## 源码精读

### 1. `AliasTyKind`：同一个 `TyKind::Alias` 下的四种语义

位置：`compiler/rustc_type_ir/src/ty_kind.rs`，`AliasTyKind`。

当前源码定义四类 type alias：

```rust,ignore
pub enum AliasTyKind<I: Interner> {
    Projection { def_id: I::TraitAssocTyId },
    Inherent { def_id: I::InherentAssocTyId },
    Opaque { def_id: I::OpaqueTyId },
    Free { def_id: I::FreeTyAliasId },
}
```

它们共享 `TyKind::Alias(IsRigid, AliasTy)` 这个外层表示，但计算规则不同：

| kind | 概念例子 | 结果从哪里取得 |
|---|---|---|
| `Projection` | `<T as Trait>::Assoc` | `ParamEnv`、trait/object bound 或选中的 impl |
| `Inherent` | `Type::Assoc` | inherent impl 的关联项定义 |
| `Opaque` | RPIT、TAIT、RPITIT 的 opaque identity | 受 defining scope 与 `TypingMode` 控制 |
| `Free` | checked/free type alias | alias 的 `type_of(def_id)`，同时注册 alias predicates |

`AliasTy` 本身定义在 `compiler/rustc_type_ir/src/ty/alias.rs`：

```rust,ignore
pub struct Alias<I: Interner, K> {
    pub kind: K,
    pub args: I::GenericArgs,
    // constructor guard omitted
}

pub type AliasTy<I> = Alias<I, AliasTyKind<I>>;
```

因此 alias identity 不直接保存“最终类型”，而是保存：

```text
哪个定义（kind 中的 def_id）
+ 用什么 GenericArgs 实例化
```

对于 projection，`args` 包含 trait 的参数以及关联项自己的 GAT 参数。例：

```rust,ignore
trait Map<K> {
    type Out<V>;
}
```

`<S as Map<K>>::Out<V>` 可概念化为：

```text
AliasTy {
  kind: Projection { def_id: Map::Out },
  args: [S, K, V],
}
```

这里 `[S, K]` 属于 trait/parent 层，`[V]` 属于关联项自己的 generics。

### 2. 普通 type alias 通常在 lowering 时展开

位置：`compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs`，`lower_path_segment`。

当前实现的关键分支是：

```rust,ignore
if let DefKind::TyAlias = tcx.def_kind(def_id)
    && tcx.type_alias_is_checked(def_id)
{
    let alias_ty = ty::AliasTy::new_from_args(tcx, ty::Free { def_id }, args);
    Ty::new_alias(tcx, ty::IsRigid::No, alias_ty)
} else {
    tcx.at(span).type_of(def_id).instantiate(tcx, args).skip_norm_wip()
}
```

所以对稳定 Rust 中常见的透明别名：

```rust,ignore
type Pair<T> = (T, T);
```

`Pair<u32>` 通常在 lowering 时就得到 `(u32, u32)`，不会长期保留一个 `AliasTyKind::Free` 节点。

`Free` 主要用于当前的 checked type alias 路径；`type_alias_is_checked` 在启用 `checked_type_aliases` 时成立，也会覆盖包含 TAIT 的相关别名。归一化 `Free` alias 时，编译器既实例化右侧 `type_of`，也把该 alias 的 predicates 注册为 goals。

课程规划里的 “weak alias” 是较早语境中的称呼；在当前源码中追踪这一概念时，应使用 `Free` / free alias 这一组名称。

### 3. `ProjectionPredicate`：把 alias 与结果 term 联系起来

位置：`compiler/rustc_type_ir/src/predicate.rs`，`ProjectionPredicate`；`compiler/rustc_type_ir/src/term_kind.rs`，`AliasTermKind`。

alias 节点只表示待计算项：

```text
<T as Iterator>::Item
```

projection predicate 表示它与某个结果相等：

```text
ProjectionPredicate {
  projection_term: <T as Iterator>::Item,
  term: U,
}
```

也就是：

```text
<T as Iterator>::Item == U
```

`term` 使用 `Term`，因此同一个框架既能表达关联类型，也能表达关联常量：

```text
TermKind::Ty(Ty)
TermKind::Const(Const)
```

这也解释了为什么源码逐渐使用 `AliasTerm` / `ProjectionTerm`，而不只叫 `ProjectionTy`：同一套求解流程正在覆盖 type 与 const。

第 07 章见过的：

```text
ClauseKind::Projection(<T as Iterator>::Item == U)
```

可以作为 `ParamEnv` 中的 assumption。它不只是说 `T: Iterator`，还直接给出了关联项的值。单独的 `T: Iterator` 并不能推出 `Item` 的具体类型。

### 4. old solver：遍历值并对 alias 分派

位置：`compiler/rustc_trait_selection/src/traits/normalize.rs`，`NormalizeExt::normalize`、`AssocTypeNormalizer::fold_ty`。

类型检查期常见入口是：

```rust,ignore
infcx.at(&cause, param_env).normalize(value)
```

返回：

```text
InferOk {
  value: normalized_value,
  obligations,
}
```

old solver 下，`AssocTypeNormalizer` 作为 `TypeFolder` 深层访问输入。遇到 `TyKind::Alias` 后按 kind 分派：

```rust,ignore
match data.kind {
    ty::Projection { .. } => self.normalize_trait_projection(...),
    ty::Inherent { .. } => self.normalize_inherent_projection(...),
    ty::Free { .. } => self.normalize_free_alias(...),
    ty::Opaque { def_id } => { /* 由 TypingMode 决定是否 reveal */ }
}
```

这是一种 eager normalization：调用者要求归一化整个值，folder 立即深入其子结构，尽量替换所遇到的 alias，并收集过程中产生的 obligations。

例如：

```text
Option<<T as Iterator>::Item>
```

在 `[<T as Iterator>::Item == u32]` 的 `ParamEnv` 下可得到：

```text
Option<u32>
```

如果 alias 的结果本身仍含 alias，`normalize_with_depth_to` 会继续递归归一化。

### 5. old solver：projection candidate 从哪里来

位置：`compiler/rustc_trait_selection/src/traits/project.rs`，`project`。

`project` 为一个 projection 按顺序组装候选：

```rust,ignore
assemble_candidates_from_param_env(...);
assemble_candidates_from_trait_def(...);
assemble_candidates_from_object_ty(...);
assemble_candidates_from_impls(...);
```

四个来源分别表示：

1. `ParamEnv`

   例如调用点已有 `<T as Iterator>::Item == u32`。

2. trait definition

   例如关联类型自身的 bound 中包含另一个 projection equality，能为嵌套 projection 提供信息。

3. trait object

   例如 `dyn Iterator<Item = u32>` 自带 existential projection bound。

4. impl selection

   先证明/选择 `T: Trait`，再从适用 impl 的关联项定义取得值。

`ProjectionCandidateSet` 最终是：

```text
None | Single(candidate) | Ambiguous | Error
```

`ParamEnv` candidate 具有优先级。尤其要区分：

```text
T: Iterator
```

只能证明 trait；而：

```text
T: Iterator<Item = u32>
```

在 IR 中会包含 trait clause 和 projection clause，后者才能把 `T::Item` 归一化为 `u32`。

### 6. impl candidate 如何给出关联类型值

位置：`compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs`，`consider_impl_candidate`。当前 new solver 的实现把这条路径表达得很集中。

对于：

```rust,ignore
trait Convert<A> {
    type Out<B>;
}

impl<T> Convert<i32> for Vec<T>
where
    T: Clone,
{
    type Out<B> = (T, B);
}
```

归一化：

```text
<Vec<u32> as Convert<i32>>::Out<bool>
```

核心步骤是：

```text
1. fresh_args_for_item(impl)
   impl<T> 变成 impl<?T>

2. eq(goal_trait_ref, instantiated_impl_trait_ref)
   Convert<i32> for Vec<u32>
   ==
   Convert<i32> for Vec<?T>

3. 得到 ?T = u32

4. predicates_of(impl).instantiate(impl_args)
   产生 u32: Clone 等 nested goals

5. 处理 associated item / GAT 自己的 predicates

6. translate_args / rebase args
   把 goal、选中 impl 和实际定义关联项的 impl 参数对齐

7. type_of(associated_item).instantiate(target_args)
   (T, B) -> (u32, bool)

8. 继续 normalize 结果，并把最终 term 与输出变量相等
```

这正是第 08 章 “select impl 后产生 nested obligations” 在 projection 上的具体版本：取得关联项值与证明 impl where-clauses 是同一次 candidate confirmation 的两个输出。

### 7. ambiguity：先以推理变量占位，再留下 projection obligation

位置：`compiler/rustc_trait_selection/src/traits/project.rs`，`normalize_projection_term`。

old solver 中，如果 projection 因尚未确定的 inference vars 暂时无法归一化，`normalize_projection_term` 会：

```text
< ?T as Iterator >::Item
        ↓
       ?U

并注册：
< ?T as Iterator >::Item == ?U
```

源码注释直接描述了这套行为：ambiguity 时创建 fresh variable，并生成 deferred predicate，等更多类型信息出现后再处理。

这保留了两类信息：

```text
当前类型结构可以继续工作：使用 ?U

未来仍需满足语义约束：
< ?T as Iterator >::Item == ?U
```

假设后续统一得到：

```text
?T = Vec<u32>
```

fulfillment 重新处理 projection obligation，选择 `Vec<u32>` 的 impl，最终得到：

```text
?U = u32
```

所以 normalization 与 inference 并非严格的前后两阶段，而是会通过 inference variables 和 obligations 相互驱动。

### 8. new solver：`Projection` 与内部 `NormalizesTo`

位置：

- `compiler/rustc_next_trait_solver/src/solve/project_goals/mod.rs`，`normalize_associated_term`
- `compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs`，`compute_normalizes_to_goal`
- `compiler/rustc_type_ir/src/predicate.rs`，`NormalizesTo`

对外的 projection goal 可以写成：

```text
<T as Trait>::Assoc == Expected
```

new solver 不直接用 `Expected` 来决定归一化候选。它先创建一个完全未约束的输出变量：

```text
NormalizesTo {
  alias: <T as Trait>::Assoc,
  term: ?U,
}
```

先只根据 alias 计算候选和结果，再执行：

```text
?U == Expected
```

原因是 normalization 应尽量表现为 alias 的函数：

```text
normalize(alias) -> term
```

而不是让期望类型反向影响“选择哪个归一化结果”。因此源码要求 `NormalizesTo.term` 是 unconstrained inference variable，并说明 `NormalizesTo` 是 projection 实现细节，不会作为普通 nested goal 泄露出 solver。

若 candidate 的 nested goals 处于 ambiguity，solver 会把相关 nested normalization goals 带回外层 projection goal，使它们仍可受后续 inference 进展驱动。

### 9. eager、lazy 与 structural normalization

可以从“何时必须知道 alias 的实际结构”来区分：

```text
eager/deep normalization
  主动遍历整个值，尽量把内部 aliases 都换成结果

lazy normalization
  alias 可以继续留在 IR 中；需要比较、选择或查看外层结构时再建立 goal

structural normalization
  为了做某个结构敏感操作，只归一化到足以看见外层 TyKind
```

例如判断一个类型是否是 tuple，需要先看见最外层结构：

```text
<T as Trait>::Assoc
```

如果它最终是 `(u32, bool)`，则至少要 structural normalize 到 tuple；而 tuple 内部若仍含其他 alias，当前操作未必需要全部展开。

当前 rustc 同时保留 old solver 与 new solver 路径，源码注释中的 “lazy normalization” 也常用于描述正在演进的设计方向。阅读调用点时应优先确认：

```text
调用的是 deep normalize 吗？
只需要 structural normalize 吗？
结果是否携带 obligations/goals？
当前 TypingMode 是否允许 reveal opaque？
```

### 10. `normalize_erasing_regions` 的使用阶段

位置：`compiler/rustc_middle/src/ty/normalize_erasing_regions.rs`，`TyCtxt::normalize_erasing_regions`。

类型检查期常用的 normalization 工作在 `InferCtxt` 中：

```text
可能含 inference vars
+ 需要 ParamEnv
+ 返回 obligations
+ 可因 ambiguity 延迟
```

后期的 `normalize_erasing_regions` 面向已经完成主要推理的值：

```text
erase regions
+ canonical query / solver normalization
+ 期望得到可供 layout、instance、codegen 等阶段使用的结果
```

因此两者的心智模型分别是：

```text
typeck normalization
  “在推理仍进行时，把能算的 alias 算掉，并登记剩余证明任务”

normalize_erasing_regions
  “在不再关心具体 region identity 的后期环境中，取得可消费的规范化结果”
```

## 正文

### 完整示例：从 projection IR 到结果

考虑：

```rust,ignore
trait Lookup<K> {
    type Value;
}

impl<T: Clone> Lookup<usize> for Vec<T> {
    type Value = Option<T>;
}

fn use_value<T: Clone>(x: <Vec<T> as Lookup<usize>>::Value) {
    // ...
}
```

projection IR：

```text
AliasTy {
  kind: Projection { def_id: Lookup::Value },
  args: [Vec<T>, usize],
}
```

在函数体的 `ParamEnv = [T: Clone, T: Sized]` 下归一化：

```text
goal trait ref:
  Vec<T>: Lookup<usize>

impl head:
  Vec<?X>: Lookup<usize>

eq:
  ?X = T

impl predicate:
  ?X: Clone
  -> T: Clone

associated type value:
  Option<?X>
  -> Option<T>
```

输出：

```text
value: Option<T>
obligations: [T: Clone]
```

随后 fulfillment 使用调用点 `ParamEnv` 证明 `T: Clone`，整个 normalization 才在语义上完成。

### projection equality 与 trait bound 的配对

源码：

```rust,ignore
fn f<T: Iterator<Item = u32>>(x: T) { ... }
```

可概念化为两条 caller bounds：

```text
T: Iterator
<T as Iterator>::Item == u32
```

它们的职责分别是：

```text
Trait clause
  证明 Iterator 实现存在

Projection clause
  给出 Item 的具体值
```

这解释了 old solver 源码里的注释：从 `ImplSource::Param` 只能知道 trait 成立，无法直接知道关联类型；真正的值由 `assemble_candidates_from_param_env` 找到 projection clause。

### normalization 为何会产生 nested obligations

候选的“结果是什么”和“候选是否适用”是两个相连的问题：

```rust,ignore
impl<T> Trait for Wrapper<T>
where
    T: Bound,
{
    type Assoc = Result<T, T::Error>;
}
```

选择并确认这个 candidate 后至少产生：

```text
T: Bound
<T as Bound>::Error 的继续归一化任务
```

因此只看最终 `Result<...>` 不足以表示 normalization 的全部结果。`Normalized { value, obligations }` 正是为了同时返回计算值和合法性条件。

### opaque 也是 alias，但 reveal 规则不同

`Opaque` 与 `Projection` 共用 `TyKind::Alias`，并不意味着它们能用同样的方式随时展开：

```text
Projection
  通过 trait/impl/ParamEnv 计算关联项

Opaque
  对使用方保持 identity；只在 defining scope 或允许 reveal 的 TypingMode 下连接 hidden type
```

当前 old-solver normalizer 的 `fold_ty` 明确按 `TypingMode` 控制 opaque：typeck/coherence/borrowck 周边通常保留 opaque，`PostAnalysis` / `Codegen` 才允许进一步 reveal。opaque 的完整规则将在第 14 章展开。

## 常见概念辨析

1. `AliasTy` 与 `ProjectionPredicate` 是不同层次。

   前者是一个 term；后者是关于该 term 与结果之间关系的命题。

2. `T: Trait` 与 `<T as Trait>::Assoc == U` 提供不同信息。

   前者说明 impl 存在；后者说明关联项的值。

3. 普通 `type Alias<T> = ...` 不一定在 Type IR 中保留为 alias。

   当前稳定路径通常在 lowering 时直接实例化右侧；`Free` 是 checked/free alias 路径。

4. normalization 的结果不只是一棵新类型。

   在类型检查期，它还可能产生必须进入 fulfillment 的 obligations。

5. ambiguity 下返回推理变量不表示约束消失。

   fresh variable 与 deferred projection obligation 共同保留待求解关系。

6. opaque alias 的 hidden type 不由普通 projection candidate 搜索随时揭示。

   它受 defining scope、typing mode 和 reveal 规则约束。

## 本章小结

`TyKind::Alias` 把尚未完全计算的名字保留在 Type IR 中；`AliasTy` 用 `kind + args` 标识这个名字。projection normalization 把 `<T as Trait>::Assoc` 转成实际 term，其信息可来自 `ParamEnv`、trait definition、trait object 或 impl。candidate confirmation 会实例化 impl/关联项参数、取得关联项定义，并产生 impl where-clauses 等 nested obligations。信息不足时，normalization 可以用推理变量占位，并留下 projection obligation 等待 fulfillment。new solver 进一步用内部 `NormalizesTo(alias, ?U)` 保证候选计算主要由 alias 决定，再把 `?U` 与外部 expected term 相等。
