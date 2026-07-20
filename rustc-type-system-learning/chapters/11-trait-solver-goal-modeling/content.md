---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "11"
document: content
status: completed
updated_at: 2026-09-05
---

# 11. Trait Solver：Goal 建模

## 学习目标

完成本章后，应当能够：

1. 解释 `Obligation`、`Goal`、`Predicate` 与 `ParamEnv` 在 solver 边界上的职责。
2. 读懂 `Goal<I, P> { param_env, predicate }`，并理解泛型参数 `P` 的用途。
3. 追踪普通 root goal 经 canonicalization、search graph 和 `enter_canonical` 进入 `EvalCtxt` 的过程。
4. 根据 `PredicateKind` / `ClauseKind` 判断 `compute_goal` 的分派目标。
5. 解释 goal decomposition 如何通过 `add_goal` / `add_goals` 产生 nested goals。
6. 手算 `try_evaluate_added_goals` 的 fixpoint 行为与 `HasChanged` 的作用。
7. 区分 `Certainty::Yes`、`Certainty::Maybe` 与 `Err(NoSolution)`。
8. 理解 `GoalSource` 如何参与 nested-goal 路径分类和 coinductive cycle 的基础处理。

## 前置知识

- 第 07 章：`Predicate`、`Clause` 与 `ParamEnv`。
- 第 08 章：`Obligation`、fulfillment 与 nested obligations。
- 第 09 章：projection 与 `NormalizesTo`。
- 第 10 章：canonical input、query-local inference variables 与 canonical response。

## 核心心智模型

一个 solver goal 是：

```text
在 assumptions = ParamEnv 下，证明 predicate
```

即：

```rust,ignore
Goal {
    param_env,
    predicate,
}
```

它进入 next solver 后的主线是：

```text
外层 typeck / fulfillment
  Obligation(predicate, param_env, cause, recursion_depth)
  │
  ├─ 抽取 solver 所需逻辑部分
  ▼
Goal { param_env, predicate }
  │
  ├─ eager resolve + canonicalize
  ▼
Canonical<QueryInput<Goal>>
  │
  ├─ search graph：缓存、递归栈、cycle/fixpoint
  ├─ enter_canonical：实例化为 query-local variables/placeholders
  ▼
EvalCtxt::compute_goal
  │
  ├─ 打开 predicate binder
  ├─ 按 PredicateKind / ClauseKind 分派
  ├─ 当前 goal 的专用求解规则
  ├─ add_goal(s)：产生 nested goals
  └─ 迭代求值 nested goals
  ▼
Canonical<Response>
  │
  └─ 应用回调用方 InferCtxt / fulfillment
```

本章的核心区分是：

```text
Goal 建模
  表达“在什么环境下证明什么”

Goal decomposition
  把一个复合证明步骤拆成必须同时成立的 nested goals

Candidate search
  为 trait/projection goal 枚举和比较可能的证明路径
```

candidate search 会在第 12 章集中展开；本章只在解释 decomposition 时使用单个 impl candidate。

## 源码地图

| 路径 | 关键符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_type_ir/src/solve/mod.rs` | `Goal`、`GoalSource`、`QueryInput`、`Response`、`Certainty`、`MaybeCause` | solver 的公共数据模型 |
| `compiler/rustc_type_ir/src/predicate_kind.rs` | `PredicateKind`、`ClauseKind` | goal 的逻辑种类 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` | `EvalCtxt`、`enter_root`、`enter_canonical`、`evaluate_goal_raw`、`compute_goal` | goal 求值主控制流 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` | `add_goal`、`try_evaluate_added_goals` | nested-goal 队列与 fixpoint |
| `compiler/rustc_next_trait_solver/src/solve/trait_goals.rs` | `compute_trait_goal`、`consider_impl_candidate` | trait goal 到 impl where-clauses 的分解 |
| `compiler/rustc_next_trait_solver/src/solve/mod.rs` | `compute_type_outlives_goal`、`compute_subtype_goal`、`compute_well_formed_goal` | 非 trait goal 的不同求解形态 |
| `compiler/rustc_next_trait_solver/src/solve/search_graph.rs` | `SearchGraphDelegate` | canonical goal 的递归求值和 provisional result |
| `compiler/rustc_type_ir/src/search_graph/mod.rs` | `PathKind` | inductive、unknown 与 coinductive 路径分类 |
| `compiler/rustc_type_ir/src/search_graph/global_cache.rs` | `GlobalCache` | 跨 root evaluation 的 goal 结果缓存 |
| `compiler/rustc_middle/src/ty/context.rs` | `new_solver_evaluation_cache` | 挂在 `TyCtxt` 上的全局 solver cache |

## 源码精读

### 1. `Goal` 只保存逻辑命题与假设环境

位置：`compiler/rustc_type_ir/src/solve/mod.rs`，`Goal`。

```rust,ignore
pub struct Goal<I: Interner, P> {
    pub param_env: I::ParamEnv,
    pub predicate: P,
}
```

源码注释将它定义为：在 `param_env` assumptions 下证明 `predicate`。

与第 08 章的 `Obligation` 对照：

```text
Obligation
  predicate
  param_env
  cause
  recursion_depth

Goal
  predicate
  param_env
```

`cause`、span 和 obligation backtrace 服务于调度与诊断，不改变命题的逻辑答案，因此不进入 solver goal 的相等性和缓存键。递归与 cycle 信息由 search graph 的求值栈承担，而不是把 `recursion_depth` 放进每个 `Goal`。

`Goal<I, P>` 对 predicate 类型参数化。外部入口通常使用：

```text
Goal<I, I::Predicate>
```

完成分派后，则可使用更精确的 typed goal：

```text
Goal<I, TraitPredicate<I>>
Goal<I, OutlivesPredicate<I, I::Region>>
Goal<I, SubtypePredicate<I>>
```

`Goal::with` 会保留同一个 `param_env`，只替换 predicate，适合从父 goal 构造 nested goal。

### `TraitRef`：trait identity 与完整 generic arguments

位置：`compiler/rustc_type_ir/src/predicate.rs`，`TraitRef` / `TraitPredicate`。

`TraitRef` 的核心数据只有：

```rust,ignore
pub struct TraitRef<I: Interner> {
    pub def_id: I::TraitId,
    pub args: I::GenericArgs,
}
```

其中：

```text
def_id
  指明是哪一个 trait

args
  对该 trait 全部 early-bound generic parameters 的实例化参数
  第 0 项固定为 Self，后面按 generics 定义顺序排列 lifetime/type/const args
```

例如：

```rust,ignore
trait Convert<'a, T, const N: usize> {}

// S: Convert<'x, u32, 8>
```

可表示为：

```text
TraitRef {
  def_id: Convert,
  args: [Type(S), Lifetime('x), Type(u32), Const(8)],
}
```

`TraitRef::self_ty()` 正是读取 `args.type_at(0)`。

`TraitRef` 是“引用哪个 trait，以及用什么参数实例化它”，外围结构继续补充命题语义：

```text
TraitRef
  { def_id, args }

TraitPredicate
  { trait_ref, polarity }

ClauseKind::Trait(TraitPredicate)
  把 trait predicate 放入可作为 where-bound/assumption 的 clause

Binder<TraitPredicate> / Predicate
  表达可能出现的 late-bound variables，并进入统一 predicate 表示

Goal
  { param_env, predicate }
  指定在哪个 assumption 环境中证明它

Obligation
  在 goal 的逻辑内容之外保存 cause、recursion_depth 等调度和诊断信息
```

因此，`ParamEnv`、polarity、binder 和 diagnostic cause 都不直接存放在 `TraitRef` 内。

associated type binding 也不属于 `TraitRef::args`。例如：

```rust,ignore
T: Iterator<Item = u32>
```

逻辑上会拆成：

```text
TraitRef:
  Iterator<T>

Projection clause:
  <T as Iterator>::Item == u32
```

trait object 使用相关但不同的 `ExistentialTraitRef`。例如 `dyn Trait<'a, U>` 中具体 `Self` 被 existentially erased，所以它的 args 只有 `['a, U]`；当提供具体 self type 时，`with_self_ty` 才能重新构造普通 `TraitRef`。

### 2. `PredicateKind` 是 `compute_goal` 的分派表

位置：

- `compiler/rustc_type_ir/src/predicate_kind.rs`，`PredicateKind` / `ClauseKind`
- `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`EvalCtxt::compute_goal`

`PredicateKind` 的主要形态包括：

```text
Clause(Trait / Projection / TypeOutlives / RegionOutlives /
       WellFormed / ConstEvaluatable / ...)
Subtype
Coerce
DynCompatible
NormalizesTo
Ambiguous
```

`compute_goal` 先取得：

```rust,ignore
let Goal { param_env, predicate } = goal;
let kind = predicate.kind();
```

然后通过 `enter_forall_with_assumptions` 打开 predicate 外层 binder，再按 kind 分派：

```rust,ignore
Clause(Trait(p))          => compute_trait_goal(...)
Clause(Projection(p))     => compute_projection_goal(...)
Clause(TypeOutlives(p))   => compute_type_outlives_goal(...)
Clause(RegionOutlives(p)) => compute_region_outlives_goal(...)
Subtype(p)                => compute_subtype_goal(...)
Clause(WellFormed(t))     => compute_well_formed_goal(...)
NormalizesTo(p)           => compute_normalizes_to_goal(...)
Ambiguous                 => response(Certainty::AMBIGUOUS)
```

这里存在两个层次：

```text
PredicateKind::Clause(ClauseKind::Trait(...))

外层 PredicateKind
  表示所有可求解 predicate 的并集

内层 ClauseKind
  表示可以作为 where-clause / implied bound assumption 的子集
```

因此第 07 章中的 `Clause`/`Predicate` 区别直接决定本章的 dispatch 形状。

### 3. root `EvalCtxt` 与 canonical `EvalCtxt` 的职责不同

位置：`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`enter_root` / `enter_canonical` / `evaluate_goal_raw`。

`enter_root` 在 solver 外部调用入口创建 search graph：

```text
root EvalCtxt
  owns SearchGraph
  接收调用方普通 Goal
  本身不直接构造 canonical response
```

`evaluate_goal_raw` 对 root 或 nested goal 执行：

```text
eager_resolve_vars
→ canonicalize_goal
→ search_graph.evaluate_goal
```

search graph 真正计算一个 canonical key 时调用 `enter_canonical`：

```text
build_with_canonical
→ 创建 query-local InferCtxt/delegate
→ 实例化 canonical variables
→ 保存 var_values、var_kinds、max_input_universe
→ 共享同一 search graph
→ compute_goal(query-local goal)
```

这个设计使 root goal 与每个 nested goal 都能使用稳定 canonical identity 做缓存、cycle detection 和 response 回放，同时整条递归证明共享同一 search graph。

### 4. impl candidate 通过 where-clauses 分解 goal

位置：`compiler/rustc_next_trait_solver/src/solve/trait_goals.rs`，`consider_impl_candidate`。

考虑：

```rust,ignore
trait Ready {}

impl<T: Clone + Send> Ready for Wrapper<T> {}
```

证明：

```text
Goal(Wrapper<X>: Ready)
```

单个 impl candidate 的确认过程是：

```text
fresh_args_for_item
  impl T -> ?I

eq(goal trait-ref, instantiated impl trait-ref)
  Wrapper<X>: Ready == Wrapper<?I>: Ready
  得到 ?I = X

predicates_of(impl).iter_instantiated(impl_args)
  ?I: Clone
  ?I: Send

goal.with(...)
  Goal(X: Clone, same ParamEnv)
  Goal(X: Send,  same ParamEnv)

add_goals(GoalSource::ImplWhereBound, ...)
```

所以“选中 impl”只建立了父 goal 到证明路径的入口。impl where-clauses 是该 candidate 成立的合取前提：

```text
Wrapper<X>: Ready
  <= candidate impl
  <= X: Clone AND X: Send
```

### 5. `add_goal` 不是简单 `Vec::push`

位置：`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`add_goal`。

当前实现会先：

```text
normalize nested predicate
→ 写入 proof-tree inspection
→ 尝试 compute_goal_fast_path
```

若 fast path 已返回 `Certainty::Yes`，该 goal 当场完成；若是 `Maybe` 或没有 fast path，才进入：

```text
nested_goals: Vec<(GoalSource, Goal, Option<GoalStalledOn>)>
```

`GoalSource` 不只是诊断标签。它还决定父 goal 到 nested goal 这一步在 search graph 中属于哪种 `PathKind`，从而影响 cycle 语义。

### 6. nested goals 通过 progress-sensitive fixpoint 求值

位置：`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`try_evaluate_added_goals` / `evaluate_added_goals_step`。

每一轮会取出当前全部 nested goals，并分别得到：

```text
GoalEvaluation {
  goal,
  certainty,
  has_changed,
  stalled_on,
}
```

这里的 `has_changed` 是每个 goal 自己的结果，不是在整轮结束后对整个 `InferCtxt` 做 snapshot diff。`evaluate_goal` 得到 canonical response 后，会在将 response 实例化并应用到调用方之前检查：

```rust,ignore
let has_changed =
    if !has_only_region_constraints(response) {
        HasChanged::Yes
    } else {
        HasChanged::No
    };

instantiate_and_apply_query_response(...);
```

当前 `has_only_region_constraints` 检查的是：

```text
var_values 对 type/const 等变量仍为 identity（忽略 region variables）
AND 没有新的 opaque-types constraints
AND 没有 normalization nested goals
```

因此下面这些 response 会记为 `HasChanged::Yes`：

```text
?T0 := u32
?C0 := 8
登记新的 opaque-type constraint
返回新的 normalization nested goals
```

若 response 仅增加 region constraints，则记为 `HasChanged::No`。这里的 progress 特指能够推动当前 trait-solver fixpoint 的 inference/external-constraint 变化；region constraints 交由相应的 region 处理阶段，不作为再次运行 trait goals 的理由。

`evaluate_added_goals_step` 对整轮的聚合方式是：

```text
unchanged_certainty = Some(Yes)

依次计算本轮每个 goal：
  单个 goal.has_changed == Yes
    -> unchanged_certainty = None

遍历完本轮取出的所有 goals 后：
  unchanged_certainty == None
    -> 本轮至少有一个 goal 推进了 inference state
    -> 开始下一轮

  unchanged_certainty == Some(certainty)
    -> 整轮没有进展
    -> 已到达当前 fixpoint，返回聚合 certainty
```

单个 goal 的 response 会立即通过 `instantiate_and_apply_query_response` 应用，所以本轮后面求值的 goals 已经可以观察前面 goal 带来的 inference 变化。仍为 `Maybe` 的 goals 会进入下一轮队列；已经为 `Yes` 的 goals 不再入队。

“进入下一轮队列”不等于每个 `Maybe` goal 都会完整地重新运行 solver。`Maybe + HasChanged::No` 时，`GoalEvaluation` 会记录 `stalled_on`，其中保存这个 goal 等待的 inference args、sub-unification roots 和 opaque storage 状态。下一轮调用 `evaluate_goal` 时先执行：

```text
rerunning_stalled_goal_may_make_progress(stalled_on)
```

其行为是：

```text
stalled dependencies 发生变化
  -> MayMakeProgress
  -> 重新运行 fast path 或完整 solver

stalled dependencies 均未变化
  -> WontMakeProgress(previous certainty)
  -> 直接返回原来的 Maybe + HasChanged::No
```

所以某个不相关 goal 的 `HasChanged::Yes` 会开启下一轮，但不会迫使所有 stalled goals 做昂贵的重复求解。例如：

```text
G1 = ?A: Trait，Maybe，stalled_on = [?A]
G2 推进了 ?B，HasChanged::Yes

下一轮：
  G1 仍在队列中
  但 ?A 未变化，所以 G1 快速返回原 Maybe
```

如果 G2 推进的正是 `?A`，G1 才会真正重新求解。`Maybe + HasChanged::Yes` 不会构造稳定的 `stalled_on`，因此下一轮会再次运行，以继续消费它刚产生的 inference progress。

规则是：

```text
certainty = Yes
  当前 nested goal 完成，不再入队

certainty = Maybe
  重新放回 nested_goals

has_changed = Yes
  本轮 inference state 有进展，再运行一轮

所有剩余 Maybe 均无进展
  到达 fixpoint，合并并返回 Maybe certainty

任一 nested goal 返回 NoSolution
  当前 candidate 返回 NoSolution

超过 FIXPOINT_STEP_LIMIT
  返回 overflow certainty
```

例如：

```text
初始队列 [G1, G2]

round 1:
  G1 -> Maybe, HasChanged::Yes，重新入队
  G2 -> Yes，移出队列
  因发生进展，继续

round 2:
  G1 -> Yes，移出队列

最终：Certainty::Yes
```

如果 round 1 中 `G1 -> Maybe, HasChanged::No`，则已经没有新信息可推动它，当前 fixpoint 的结果是 `Maybe`。

### 7. `Certainty` 与 `NoSolution` 是三种不同结果

位置：`compiler/rustc_type_ir/src/solve/mod.rs`，`Certainty` / `MaybeCause`。

```rust,ignore
pub enum Certainty {
    Yes,
    Maybe(MaybeInfo),
}
```

完整 query result 则概念化为：

```text
Ok(Response { certainty: Yes, ... })
Ok(Response { certainty: Maybe(Ambiguity), ... })
Ok(Response { certainty: Maybe(Overflow), ... })
Err(NoSolution)
```

语义分别是：

```text
Yes
  当前证明路径及其 nested goals 已成立；response 仍可携带推理赋值和外部约束

Maybe(Ambiguity)
  当前信息不足以给出唯一稳定答案，可能在 inference 进展后重试

Maybe(Overflow)
  求值达到递归深度或 fixpoint 限制，或对某些 cycle 保守地采用 overflow certainty

NoSolution
  该 goal/candidate 在当前环境下不成立
```

多个 nested goals 是合取关系，使用 `Certainty::and`：只要其中一个为 `Maybe`，整个合取就不能是 `Yes`。多个候选是析取关系，其 response 合并属于第 12 章内容。

### 8. `GoalSource` 与 coinduction 基础

位置：

- `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`，`step_kind_for_source`
- `compiler/rustc_type_ir/src/search_graph/mod.rs`，`PathKind`
- `compiler/rustc_next_trait_solver/src/solve/search_graph.rs`，`initial_provisional_result`

#### 统一心智模型：递归证明路径所携带的依据

把 `PathKind` 理解为“这段证明路径经过了哪些种类的推理规则，以及这些规则能否支持循环证明”的压缩摘要。

只有 `A -> B -> A` 这样的依赖形状，还不足以决定怎样处理循环。反复改写同一个关联类型，与按照 `Send` 的结构规则检查递归字段，虽然都可能形成环，采用的证明规则却不同。`PathKind` 保留的就是这个区别。

在单条边上，三个主要标签可以读作：

- `Inductive`：这一步没有提供允许 coinduction 的依据。
- `Coinductive`：这一步提供了 rustc 认可的 coinductive 展开，例如从 `Send` 进入字段要求。
- `Unknown`：当前实现对这一步能否支持 coinduction 保留语义选择。

沿同一条路径合并时，问的是“是否已经经过至少一次被认可的 coinductive 展开”。因此，一条 `Inductive` 边表达“本步没有提供”，一条 `Coinductive` 边表达“本步提供了”；前者不否定后者。它们不是两个互相冲突的真假判断。

一旦这段路径闭环，摘要再用于选择初始 provisional result：`Coinductive` 允许先假设循环点成立并继续核验，纯 `Inductive` 在 typeck 下以 `NoSolution` 起步，`Unknown` 保留 ambiguity。`ForcedAmbiguity` 是强制保留 ambiguity 的特殊标记，覆盖其他边标签。实际结果仍取决于剩余前提、候选和 fixpoint。

这里“productive”指证明规则所提供的 guard，不是类型变小、推理变量得到解、运行时执行了一步，也不是 `HasChanged::Yes`。边标签由 `step_kind_for_source` 根据 `GoalSource` 和当前 goal context 给出；随后 `PathKind::extend` 合并路径，`cycle_path_kind` 汇总闭环，`initial_provisional_result` 选择初始循环响应。

回到上一例：归一化 `<Node as Project>::Out` 时要求 `Node: Send`，属于 normalization 所需的辅助条件，因此是 `Inductive`；从 `Node: Send` 展开到字段要求，使用的是 coinductive trait 的结构规则，因此是 `Coinductive`。同一个 `Node: Send` 出现在边的起点还是终点、由哪条规则产生依赖，决定了相应步骤的标签。

#### `PathKind` 首先标记一条证明边

`PathKind` 不是某个 goal 自身的种类，也不是该 goal 的最终 `Certainty`。它标记从当前 goal 产生一个 nested goal 的这一步：

```text
parent goal --PathKind--> nested goal
```

例如：

```text
G0 --Coinductive--> G1 --Inductive--> G2 --Unknown--> G3
```

只要 `G3` 是一个尚未出现在当前递归栈中的新 goal，这些标签不会直接决定成功或失败。真正再次遇到栈中的旧 goal 时才形成 cycle：

```text
G0 --K1--> G1 --K2--> G2 --K3--> G0
```

search graph 此时合并 `K1/K2/K3`，得到整个 cycle 的 `PathKind`，并为第一次 fixpoint iteration 提供一个 provisional result。

#### 三种主要路径的直觉：先给循环一个什么临时答案

| `PathKind` | 循环所表达的情况 | 第一次遇到 cycle 时的 provisional result |
|---|---|---|
| `Inductive` | 循环本身没有提供新的证明依据；要求有限证明树 | `Err(NoSolution)`，coherence 当前例外地使用 ambiguity |
| `Unknown` | rustc 当前尚未承诺该类步骤能否支持 coinduction | `Certainty::overflow(false)`，即保留为 `Maybe` |
| `Coinductive` | 环上至少经过一个被认可的 productive step | 无约束的 `Certainty::Yes` |
| `ForcedAmbiguity` | fuzzing/negative-reasoning 测试强制该环保持模糊 | `Certainty::overflow(false)` |

这里的 provisional result 只是循环 fixpoint 的起点，而不是跳过其他前提后直接给出最终答案。cycle 之外的 nested goals 仍然必须成立，循环结果也会重复计算直到稳定。

#### `Inductive`：`A` 不能仅靠“要求 `A`”证明自己

先看抽象规则：

```text
证明 A
  唯一规则要求先证明 A
```

对应无限展开：

```text
A
└─ A
   └─ A
      └─ ...
```

对 inductive 命题，证明必须最终到达 assumption、具体 impl 或其他 base case。因此这个环没有证明内容，第一次回到 `A` 时以 `NoSolution` 作为 provisional result。

当前 rustc 明确认定 `GoalSource::TypeRelating` 是 `Inductive`：类型相等/子类型关系产生的递归步骤只是在继续比较类型，没有穿过能够保护递归的构造步骤。`NormalizesTo` candidate 进入 impl where-clause 时也按 `Inductive` 处理，因为取出 associated item 后会立刻离开该 impl，不能把这一步当成 guarded/productive recursion。

#### `Coinductive`：递归数据的 auto-trait 可以先假设递归点成立

考虑：

```rust,ignore
struct Node {
    next: Option<Box<Node>>,
}

fn require_send<T: Send>() {}
require_send::<Node>();
```

结构化地证明 `Node: Send` 会形成类似链条：

```text
Node: Send
  -> Option<Box<Node>>: Send
  -> Box<Node>: Send
  -> Node: Send       // 回到 cycle head
```

`Send` 是 coinductive auto trait。沿它的结构性 impl/where-clauses 递归检查字段，被 rustc 视为 productive step：我们确实向内部类型前进了一层，而不是原地重复同一个命题。因此第一次再次看到 `Node: Send` 时，可以暂时使用 `Yes`，然后继续验证环外的其他字段和约束。

例如若结构还包含一个不能满足 `Send` 的字段，那个独立 nested goal 仍会让 candidate 失败。`Coinductive` 的含义是“这个结构递归环本身可以成立”，而不是“整个父 goal 无条件成立”。

当前 `CurrentGoalKind::CoinductiveTrait` 包括 coinductive traits，例如 auto traits 和 `Sized`。只有从这类当前 trait goal 进入 `ImplWhereBound`，该边才标记为 `Coinductive`。

#### `Unknown`：当前实现保留决定

考虑一个普通、尚未按 coinductive 处理的 trait，其候选形成：

```text
X: OrdinaryTrait
  -> impl where-clause 要求 X: OrdinaryTrait
  -> 回到原 goal
```

它看起来像 inductive 的无效自我证明，但当前 next solver 计划将来可能扩大 coinductive trait 的范围。因此普通 trait 的 `ImplWhereBound` cycle 暂时标记为 `Unknown`，返回 `Maybe(Overflow)`，既不把循环当作证明，也不在当前阶段确定为 `NoSolution`。

`GoalSource::Misc`、`AliasBoundConstCondition` 和 `AliasWellFormed` 目前也使用 `Unknown`，表示这些边的最终生产性分类尚未固定。

#### 为什么自我递归的 blanket impl 定义可以通过

考虑：

```rust,ignore
trait Tr {
    fn foo(&self);
}

impl<T: Tr> Tr for T {
    fn foo(&self) {}
}
```

这个 impl 不是在声明“所有 `T` 都实现 `Tr`”，而是在声明一条带前提的规则：

```text
对任意 T：
  如果 T: Tr
  那么 T: Tr
```

写成 Horn clause 是：

```text
Tr(T) :- Tr(T)
```

在 impl 定义处，rustc 主要检查：

```text
T 是否被 impl self type/trait ref 约束
trait ref 与 impl items 是否 well-formed
orphan rules 是否满足
是否与其他 impl overlap
方法体在 impl ParamEnv 下是否类型正确
```

这里 `T` 出现在 `for T` 中，所以是 constrained parameter；`Tr` 是当前 crate 的本地 trait，orphan rule 满足；只有这一条 impl 时，也没有另一条 impl 与它冲突。检查方法体时，impl 的 where-clause `T: Tr` 已经位于 `ParamEnv` 中，可以作为 assumption 使用。

rustc 不要求每条 impl rule 在定义时都能从无前提推出一个具体事实，也不要求证明“存在某个 T 满足这个 impl”。因此这条条件恒等规则本身可以注册。

真正尝试证明具体类型时，循环才出现。例如证明：

```text
S: Tr
```

candidate confirmation 过程是：

```text
选择 impl<T: Tr> Tr for T
实例化 T := S
impl head 与 goal 匹配成功
加入 impl where-clause：S: Tr
```

得到证明树：

```text
S: Tr
└─ candidate: impl<T: Tr> Tr for T
   └─ nested goal: S: Tr
      └─ 回到 cycle head
```

这条规则没有提供 base case，所以不能为一个此前没有 `Tr` 证据的具体 `S` 合成实现。在 old solver/相应诊断路径中通常表现为 `overflow evaluating the requirement S: Tr`；next solver 会按当前 ordinary-trait cycle 规则保留相应的 ambiguity/overflow，而不会得到 `Certainty::Yes`。

另一方面，在泛型函数中：

```rust,ignore
fn call<T: Tr>(x: &T) {
    x.foo();
}
```

`T: Tr` 来自函数自己的 `ParamEnv`，所以 method call 可以由 caller-bound candidate 证明。这只说明函数对满足前提的 `T` 是合法的，并没有由递归 blanket impl 创造出新的具体 implementor。

如果再为某个具体类型添加另一条 base impl，例如 `impl Tr for S`，它通常会与 blanket impl 发生 overlap：在假设 `S: Tr` 的情况下，两条 impl 都适用。也就是说，这条递归 blanket impl 虽然可以单独声明，却会占据很大的 coherence 覆盖范围。

当前 rustc 与本仓库中的 Clippy 没有针对这种 impl-header trait cycle 的专用 lint。名称相近的 lint 检查的是其他问题：

```text
rustc::unconditional_recursion
  检查函数体控制流是否所有路径都会递归调用自身

clippy::unconditional_recursion
  补充检查若干 trait method body 中的无条件递归

clippy::trait_duplication_in_bounds
  检查同一组 generic bounds 中重复写出的 trait bound
```

它们都不分析 `impl head -> impl where-clauses -> 同一 trait goal` 形成的逻辑循环。

这个 impl 对“产生新的 concrete implementor”没有进展，但仍然会改变程序的 coherence 空间：它可能与后续或下游的具体 impl overlap，从而阻止那些 impl 被声明。在 specialization、复杂 projections、supertraits 和 mutually recursive bounds 存在时，判断一条规则是否完全无用途也可能超出简单语法匹配。因此若设计 lint，更合适的诊断是：

```text
this impl cannot establish the trait for a previously unimplemented type;
its where-clause requires the same trait goal as its impl head
```

并提示它仍可能影响 overlap/coherence，而不是把它当成完全无语义效果的 dead code。

对于这里带方法的普通 trait，精确的 `impl<T: Tr> Tr for T` 在稳定 Rust 中没有 constructive use：它不会扩大“哪些 concrete types 实现 `Tr`”的集合，也就不会让 `foo` 对此前未实现 `Tr` 的 concrete type 变得可调用。

它可能产生的实际效果主要是 coherence reservation：

```text
占据 `impl Tr for T` 的 blanket overlap 空间
-> 下游或同 crate 的具体 impl 可能因 overlap 被拒绝
-> 将来可以移除自递归前提，把它扩展成真正的 blanket impl
```

这可以作为一种非常间接的 impl-space 占位手段，但会制造 solver overflow/ambiguity，并且难以向代码读者表达意图。rustc 标准库内部有专门的 `#[rustc_reservation_impl]` 表达 impl reservation；该属性是编译器内部机制，不是普通稳定 Rust API。普通库若要限制下游实现，通常使用 private supertrait 的 sealed-trait pattern，并显式为允许的类型提供 base impl。

还有两个需要分开的边界：

```text
普通泛型函数中的 T: Tr
  可以由 ParamEnv assumption 使用
  但这个自递归 impl 并未创造该 assumption

auto trait / rustc 标记的 coinductive trait
  对 self-cycle 使用 coinductive 语义
  不能套用普通 trait 的“least fixed point 不增长”结论
```

用户示例中的 `Tr` 带有方法，因此是普通 trait，而不是 auto trait；在没有 compiler-internal attributes、specialization 等额外机制时，这条 impl 的生产性结果为空，保留下来的只有 coherence 影响。它在 rustc 测试中也有用途：构造最小的 inductive/unknown cycle，用来验证 solver 不会从 `Tr(T) :- Tr(T)` 合成任意 trait evidence。

#### 一条 cycle 上有多种边时如何合并

当前 `PathKind::extend` 的优先级是：

```text
ForcedAmbiguity > Coinductive > Unknown > Inductive
```

因此：

```text
Inductive + Inductive
  -> Inductive

Inductive + Unknown
  -> Unknown

Inductive + Coinductive
  -> Coinductive

Unknown + Coinductive
  -> Coinductive

任意路径 + ForcedAmbiguity
  -> ForcedAmbiguity
```

关键规则是：整条环上只要至少有一个被认可的 `Coinductive` productive step，这个 cycle 就按 coinductive 处理；如果没有 productive step、但含有尚未分类的边，则保留为 `Unknown`；只有所有边都明确是 unproductive 时才是 `Inductive`。

这里的“优先级”不是比较哪种结果更可信，也不是多个 nested goals 做逻辑合取。`PathKind::extend` 在压缩一条 cycle 的路径摘要，它真正记录的状态可以写成：

```text
forced_ambiguity 是否出现？
known_productive_step 是否出现？
unknown_step 是否出现？
```

最终分类相当于：

```text
if 出现 forced ambiguity:
    ForcedAmbiguity
else if 出现至少一个 known productive step:
    Coinductive
else if 出现至少一个 unknown step:
    Unknown
else:
    Inductive
```

因此 `Inductive` 边的含义是“这一步没有增加 guard/productivity”，而不是“这一步已经证明 cycle 为假”。一条 cycle 中出现若干普通辅助步骤，并不会消除另一条边已经提供的 productive guard：

```text
G0 --Coinductive--> G1 --Inductive--> G2 --Inductive--> G0
```

每次从 `G0` 绕回 `G0` 都必然经过那条 `Coinductive` 边，所以递归仍然是 guarded/productive 的。这个判定与 corecursive 定义类似：一次循环可以经过多个辅助函数，只要每圈至少产生一次被认可的构造层，就没有原地无限调用。

`Unknown + Coinductive -> Coinductive` 也基于同一逻辑：判断条件是“环上是否已经存在一个确定的 productive step”。只要答案已经是肯定的，其他边是否也 productive 就不影响这个条件。`ForcedAmbiguity` 是显式要求整条路径保持模糊的特殊状态，所以它仍然覆盖 `Coinductive`。

以 rustc 的回归测试中的证明链为例：

```text
Foo<T>: Send
  --进入 coinductive trait Send 的 impl where-clause-->
T: SendIndir<Foo<T>>
  --进入普通 trait SendIndir 的 impl where-clause-->
Foo<T>: Send
```

第二条边本身不是 coinductive，但每次绕环都会经过第一条 `Send` productive step，所以整个 cycle 按 `Coinductive` 处理。如果把第一条 productive step 也去掉，环上只剩普通或 unproductive edges，才会退化为 `Unknown` 或 `Inductive`。

#### 边的先后顺序与 cycle 的实际范围

在同一个闭合的 cycle 内，先遇到 `Inductive`、后遇到 `Coinductive`，得到的路径分类仍是 `Coinductive`（这里不包含 `ForcedAmbiguity`）。例如：

```text
A --Inductive--> B --Inductive--> C --Coinductive--> A
```

处理 `A -> B` 时，仅记录这一条边的种类；B 是新的 goal，尚未形成 cycle，也就不会因为这条边是 `Inductive` 而生成 cycle 的 `NoSolution`。直到 `C -> A` 再次遇到当前栈中的 A，才把 A 到 C 的路径与最后一条回边一起合并：

```text
Inductive.extend(Inductive).extend(Coinductive)
  = Coinductive
```

`PathKind::extend` 对这几种分类的合并满足交换律和结合律，位置先后不影响结果。从直觉上看，环本身没有天然起点：只要每圈都经过同一条已认可的 productive step，它位于第一步还是最后一步并不改变这一性质。

实现位置：`compiler/rustc_type_ir/src/search_graph/mod.rs`，`check_cycle_on_stack` 先找到重复 input 的 `head_index`，再调用 `cycle_path_kind`。后者的完整函数体是：

```rust,ignore
stack.cycle_step_kinds(head).fold(step_kind_to_head, |curr, step| curr.extend(step))
```

`step_kind_to_head` 就是最后那条回边；它也参与合并。`compiler/rustc_type_ir/src/search_graph/stack.rs` 的 `cycle_step_kinds` 只遍历 cycle head 之后的栈元素，所以进入 cycle head 之前的边不在范围内。

例如：

```text
A --Coinductive--> B --Inductive--> C --Inductive--> B
```

实际闭合的是 `B -> C -> B`，其分类为 `Inductive`。`A -> B` 的 `Coinductive` 是进入这个环之前的一次步骤；反复绕 `B -> C -> B` 时不会再次经过它，因此不能作为这个内层环的 productive step。

同样，某个纯 `Inductive` 环已经闭合后，其他分支后来出现的 `Coinductive` 边不会改写这个环的分类。其他候选可能独立证明 goal，但需要按各自的证明路径处理。

这里确定的是 cycle 的路径分类。首次 coinductive 回边提供无约束的 `Yes` 作为初始 provisional result；若 cycle head 已有 provisional result 则复用已有值。整个 goal 的最终结果仍由其他前提、候选和 cycle fixpoint 决定。

#### 实际代码：先归一化关联类型，再检查 `Send` 字段

下面的完整程序把 normalization 与 auto trait 的结构性检查连成一个环：

```rust
trait Project {
    type Out;
}

impl<T: Send> Project for T {
    type Out = ();
}

struct Node(<Node as Project>::Out);

fn require_send<T: Send>() {}

fn main() {
    require_send::<Node>();
}
```

2026-09-05 使用本机 `rustc 1.99.0-nightly (d453bdd8f 2026-08-14)`，加 `-Znext-solver --emit=metadata` 编译通过。下面是按当前检出源码整理的关键依赖路径，省略 WF、`Sized` 等旁支；它是源码推演，不是编译器日志逐行转录。

先从“归一化 `<Node as Project>::Out`”这个环上节点观察：

1. 使用 `impl<T: Send> Project for T`，匹配得到 `T = Node`，需要证明 `Node: Send`。当前正在计算 `NormalizesTo` candidate，所以进入这条 impl where-bound 的边是 `Inductive`。
2. `Send` 是 auto trait。为了证明 `Node: Send`，builtin candidate 检查它的字段，产生 `<Node as Project>::Out: Send`。当前正在证明 coinductive trait，产生字段要求的边是 `Coinductive`。
3. 检查字段要求时，`add_goal` 会先归一化 predicate 中的字段类型，于是再次需要归一化 `<Node as Project>::Out`。`NormalizeGoal(Coinductive)` 保留第 2 步的边性质。

展开 projection 到内部 `NormalizesTo` 的包装后，关键闭环可以写成：

```text
NormalizesTo(<Node as Project>::Out, ?U)
  --Inductive：Project impl 要求 Node: Send-->
Node: Send
  --Coinductive：检查字段，并归一化字段类型-->
Projection(<Node as Project>::Out == ?V)
  --Inductive：TypeRelating，进入内部 normalization-->
NormalizesTo(<Node as Project>::Out, ?W)
```

这里 `?U`、`?W` 表示各次 query-local 的 unconstrained output variable；在相同环境下 canonicalize 后，首尾是同一个 normalization query。因此，环上边的合并为：

```text
Inductive.extend(Coinductive).extend(Inductive) = Coinductive
```

第一条边的种类由“当前 normalization 如何产生子要求”决定，子要求本身恰好是 `Send`，并不会让这条边提前变成 `Coinductive`。到下一步，从 `Send` 进入字段的结构性要求时，才提供 coinductive step。

这个环允许使用 coinductive provisional result 继续求解；具体输出类型仍来自 impl 中的 `type Out = ()`。其约束与其他子要求通过 response 和 fixpoint 汇合后，字段类型得到 `()`，而 `(): Send` 成立，因此最终 `Node: Send` 成立。这里递归的是证明依赖，最终字段并不按值包含另一个 `Node`。

对应实现：

- `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`：`step_kind_for_source` 给出前两步的边标签；`add_goal` 使用 `NormalizeGoal(self.step_kind_for_source(source))` 保留 normalization 的路径性质。
- `compiler/rustc_next_trait_solver/src/solve/trait_goals.rs`：`consider_auto_trait_candidate` 与 `probe_and_evaluate_goal_for_constituent_tys` 收集字段要求，并通过 `add_goals(GoalSource::ImplWhereBound, goals)` 登记。
- `compiler/rustc_next_trait_solver/src/solve/project_goals/mod.rs`：`normalize_associated_term` 使用 `GoalSource::TypeRelating` 求解内部 `NormalizesTo` goal。
- `tests/ui/traits/next-solver/cycles/coinduction/only-one-coinductive-step-needed.rs`：仓库中的相关回归测试，在类似结构中额外经过普通 trait `SendIndir`，测试“环上至少有一个 productive step”的行为。

#### `GoalSource` 到边标签的当前映射

| nested goal 来源 | 当前上下文 | 边的 `PathKind` | 直觉 |
|---|---|---|---|
| `TypeRelating` | 任意 | `Inductive` | 继续比较类型，没有 guarded step |
| `ImplWhereBound` | 当前证明 coinductive trait | `Coinductive` | 进入 coinductive trait 的结构性前提 |
| `ImplWhereBound` | 当前计算 `NormalizesTo` candidate | `Inductive` | associated item normalization 不构成保护层 |
| `ImplWhereBound` | 普通 trait goal | `Unknown` | 当前暂不固定普通 trait cycle 的语义 |
| `Misc` | 任意 | `Unknown` | 来源信息不足 |
| `NormalizeGoal(k)` | 任意 | 继承 `k` | normalization 不丢失产生它的原路径性质 |
| `AliasBoundConstCondition` / `AliasWellFormed` | 任意 | `Unknown` | 当前保守分类 |

可以把整个机制压缩成一句话：

```text
PathKind 回答的不是“这个 goal 是什么”，
而是“如果沿这条证明边最终绕回自己，第一次应该把循环点暂时当成 false、unknown 还是 true”。
```

这只是 coinduction 基础。candidate 来源、candidate response 合并以及更完整的 cycle 示例将在第 12 和第 17 章继续展开。

## 正文

### Obligation 到 Goal：去掉调度外壳，保留逻辑上下文

fulfillment 处理的是 obligation 生命周期：何时登记、何时重试、如何关联诊断。solver 处理的是逻辑问题：

```text
ParamEnv |- Predicate ?
```

因此同一个 predicate 在不同 `ParamEnv` 中是不同 goal：

```text
Goal {
  param_env: [T: Clone],
  predicate: T: Clone,
}
  -> 可由 assumption 证明

Goal {
  param_env: [],
  predicate: T: Clone,
}
  -> 需要其他 candidate，可能得到 Maybe 或 NoSolution
```

这也是 `ParamEnv` 必须进入 canonical query key 的原因。

### Fulfillment：typeck 与单次 goal solver 之间的任务调度层

type checking 会在不同时间不断产生 obligations，例如泛型 bounds、方法调用、运算符、normalization 和 impl where-clauses。创建 obligation 时，其中的 inference variables 可能仍未确定，因此不能要求每条 obligation 当场得到最终答案。

当前 next solver 的 `FulfillmentCtxt` 保存：

```text
pending:
  (PredicateObligation, Option<GoalStalledOn>)

overflowed:
  PredicateObligation
```

它的主循环可概念化为：

```text
register_predicate_obligation
  ├─ fast path 得到 Yes       -> 直接完成
  ├─ fast path 得到 Maybe     -> 保存 obligation + stalled_on
  └─ 没有 fast path           -> 保存 obligation

try_evaluate_obligations
  ├─ obligation.as_goal()
  ├─ evaluate_root_goal(goal)
  ├─ NoSolution -> 形成真实 fulfillment error
  ├─ Yes        -> 从 pending 移除
  └─ Maybe      -> 带 stalled_on 重新登记

若本轮有 HasChanged::Yes
  -> 再运行一轮，使其他 obligations 观察新的推理结果

若整轮没有 inference progress
  -> 暂时停止，保留 Maybe obligations，等待 typeck 后续信息
```

typeck 的阶段性调用允许 ambiguity 留在 pending 中；要求所有约束收束时，`evaluate_obligations_error_on_ambiguity` 会先尝试求值，再把剩余 pending obligations 转为 ambiguity errors。

例如：

```text
1. typeck 产生 obligation：Vec<?T0>: Clone
2. fulfillment 登记它
3. solver 返回 Maybe，stalled_on = [?T0]
4. 后续表达式令 ?T0 = String
5. fulfillment 再次调用 solver
6. Vec<String>: Clone -> Yes
7. obligation 从 pending 中移除
```

因此 fulfillment 的职责是：

```text
收集 obligations
+ 保存 cause / recursion_depth 等诊断上下文
+ 决定何时调用 solver
+ 根据 inference progress 重试
+ 延迟处理 ambiguity
+ 在最终检查点汇总错误
```

它与 solver 内部 nested-goal fixpoint 的边界是：

```text
FulfillmentCtxt::try_evaluate_obligations
  管理许多 root obligations；typeck 后续还能继续加入新任务

EvalCtxt::try_evaluate_added_goals
  管理一次 root goal/candidate 求值内部产生的 nested goals
```

当前 next solver 下，fulfillment 主要负责调度，实际逻辑求解通过 `obligation.as_goal()` 后交给 `evaluate_root_goal`。old solver 的 fulfillment processor 与 selection 结合得更紧，但“保存未决 obligations，推进到 fixpoint，最终报告剩余问题”的职责相同。

### Trait solver cache：局部 search graph 与 TyCtxt 全局缓存

next solver 的缓存不是简单地以 `FnCtxt` 为作用域。需要区分两层：

| 层次 | 保存位置 | 生命周期 | 能否跨函数复用 |
|---|---|---|---|
| provisional cache / recursion stack | 每次 `enter_root` 新建的 `SearchGraph` | 一次 root goal evaluation | 不能 |
| global evaluation cache | `TyCtxt::new_solver_evaluation_cache` | 当前编译上下文 | 可以，但完整 canonical key 必须相同 |

`enter_root` 每次都会执行：

```rust,ignore
let mut search_graph = SearchGraph::new(root_depth);
```

这层 search graph 保存当前证明树的栈、cycle 信息和 provisional results。它解决的是“一次递归证明内部如何避免重复计算并处理循环”，求值结束后不会成为另一个函数可见的长期缓存。

另一层 `GlobalCache` 挂在 `TyCtxt` 上：

```rust,ignore
pub new_solver_evaluation_cache:
    Lock<search_graph::GlobalCache<TyCtxt<'tcx>>>;
```

它的 map 类型概念上是：

```rust,ignore
HashMap<CanonicalInput, CacheEntry>
```

这里的 `CanonicalInput` 并不只有 predicate。完整输入包含：

```text
CanonicalQueryInput {
  canonical: Canonical {
    value: QueryInput {
      goal: Goal {
        param_env,
        predicate,
      },
      predefined_opaques_in_body,
    },
    var_kinds,
    max_universe,
  },
  typing_mode,
}
```

canonicalizer 会先 canonicalize `input.goal.param_env`，再以同一套 canonical variable 编号处理 predicate。因此：

```text
不同 ParamEnv
  -> 不同 canonical input
  -> 命中不同 cache entry

相同的完整 canonical input
  -> 即使来自不同 FnCtxt / 不同函数
  -> 也可以命中同一个 global cache entry
```

例如两个函数具有 alpha-equivalent 的环境与 goal：

```rust,ignore
fn f<T: Clone>() { /* 求解 Vec<T>: Clone */ }
fn g<U: Clone>() { /* 求解 Vec<U>: Clone */ }
```

二者局部参数名和 inference variable 编号不同，但 canonicalization 后可概念化为同一个 key：

```text
ParamEnv: [^0: Clone]
Goal:     Vec<^0>: Clone
```

因此能够复用全局结果。若第二个函数还具有 `U: Send`、使用不同 `TypingMode`，或者相关 opaque-type 输入不同，完整 key 就不同，不会误用前一个结果。

局部 inference variables 同理。一个 `FnCtxt` 中的 `?T7` 与另一个上下文中的 `?T31` 不会按原始编号写进全局 key；它们会 canonicalize 成 query-local 的 `^0`。cache 中保存 canonical response，命中后再实例化并把结果映射回当前调用方自己的 `InferCtxt`。

这也解释了 `ParamEnv` 与跨函数共享并不矛盾：`ParamEnv` 是缓存键的一部分，而不是缓存的外部隐含状态。很多非泛型函数还会共享 `ParamEnv::empty()`；泛型函数只有在 canonicalized assumptions 也相同时才可能共享。

最后不要把这层缓存与 fulfillment 混在一起：

```text
FulfillmentCtxt pending obligations
  -> 某次函数类型检查期间的任务状态

SearchGraph provisional cache
  -> 一次 root evaluation 内的递归证明状态

TyCtxt GlobalCache
  -> 可跨 root evaluation、跨 FnCtxt 复用的 canonical solver 结果
```

### binder 在 goal dispatch 前被实例化

`Predicate` 本身可以是 binder 包装的 predicate kind。`compute_goal` 不直接对 escaping bound variables 求值，而是先：

```text
predicate.kind()
→ enter_forall_with_assumptions
→ bound vars 变为 fresh placeholders
→ 在扩展后的 universe 中 dispatch
```

因此：

```text
for<'a> T: Trait<&'a u32>
```

进入求值时可概念化为：

```text
T: Trait<&P@U1 u32>
```

然后 candidate relation 与 leak/nameability 检查沿用第 04、10 章的规则。

### decomposition 不总是产生 trait nested goals

不同 goal kind 的“拆解”方式不同：

| goal | 主要动作 | 可能输出 |
|---|---|---|
| `T: Trait` | assembly/evaluate/merge candidates | impl/ParamEnv/builtin 路径与 nested goals |
| `<T as Trait>::Assoc == U` | projection/normalization | trait goal、where-clauses、输出等式 |
| `WF(T)` | `well_formed_goals` | 构成 T 良构所需的 nested goals |
| `A <: B` | type relation | inference constraints、projection/WF nested goals |
| `'a: 'b` | 注册 region outlives | external region constraint |
| `T: 'a` | 注册或拆解 type outlives | region/type-outlives constraints |
| `Ambiguous` | 直接构造 response | `Certainty::Maybe(Ambiguity)` |

因此 goal decomposition 的统一抽象不是“全部变成 trait goals”，而是：

```text
对当前 predicate 执行专用规则
→ 修改 query-local inference/constraint state
→ 必要时登记 nested goals
→ 返回 response
```

以 subtype 为例，`compiler/rustc_next_trait_solver/src/solve/mod.rs` 的 `compute_subtype_goal` 在常规分支调用 `self.sub(...)`。它再进入 `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` 的 `EvalCtxt::relate`，处理 relation 返回的子任务。下面保留了关键分类与登记步骤，省略 tracing 和其他方法：

```rust,ignore
let goals = self.delegate.relate(param_env, lhs, variance, rhs, self.origin_span)?;
for &goal in goals.iter() {
    let source = match goal.predicate.kind().skip_binder() {
        ty::PredicateKind::Subtype { .. }
        | ty::PredicateKind::Clause(ty::ClauseKind::Projection(..)) => {
            GoalSource::TypeRelating
        }
        ty::PredicateKind::Clause(ty::ClauseKind::WellFormed(_)) => GoalSource::Misc,
        p => unreachable!("unexpected nested goal in `relate`: {p:?}"),
    };
    self.add_goal(source, goal)?;
}
```

例如两个不同的未解析类型变量 `?X`、`?Y` 嵌在 tuple 中：

```text
(?X, u8) <: (?Y, u8)
  -> 逐项按协变关系比较
  -> ?X <: ?Y：relation 产生 Subtype nested goal
  -> u8 <: u8：直接成立
```

对应生成分支位于 `compiler/rustc_type_ir/src/relate/solver_relating.rs` 的 `SolverRelating::tys`：两个类型变量出现在协变位置时，把 `SubtypePredicate` 放入 `self.goals`。由此可见，type relation 会同时处理可立即建立的推理约束，以及需要 solver 后续处理的 nested goals。

### 完整追踪：`Wrapper<?T>: Ready`

定义：

```rust,ignore
trait Ready {}

impl<T: Clone> Ready for Wrapper<T> {}
```

调用方产生：

```text
Obligation {
  predicate: Wrapper<?T0>: Ready,
  param_env: P,
  cause: C,
  recursion_depth: D,
}
```

solver 输入：

```text
Goal(P, Wrapper<?T0>: Ready)
→ canonical Goal(P, Wrapper<^0>: Ready)
→ query-local Goal(P, Wrapper<?Q0>: Ready)
```

trait goal dispatch：

```text
compute_trait_goal
→ assemble_and_evaluate_candidates
→ impl candidate
```

candidate probe：

```text
impl T -> ?I0
eq(Wrapper<?Q0>, Wrapper<?I0>)
→ ?I0 = ?Q0

impl where-clause
→ nested Goal(P, ?Q0: Clone)
```

若 `?Q0` 尚无更多信息：

```text
?Q0: Clone -> Maybe
parent response -> Maybe
caller fulfillment 记录 stalled variables
```

随后调用方得到：

```text
?T0 = String
```

重试时：

```text
Wrapper<String>: Ready
→ impl candidate
→ nested String: Clone
→ Yes
→ parent Yes
```

这条链把前五章核心对象放到了一起：

```text
Obligation 负责等待和重试
Goal 负责表达命题
Canonicalization 负责查询边界
EvalCtxt 负责局部求值状态
Candidate 负责选择证明路径
Nested goals 负责证明路径的前提
Response 负责把结果带回调用方
```

## 常见概念辨析

1. `Goal` 不等于 `Predicate`。

   `Predicate` 是命题；`Goal` 是 `ParamEnv + Predicate`。

2. `Goal` 不等于 `Obligation`。

   `Obligation` 还携带 cause、深度与 fulfillment/诊断上下文；solver goal 保留逻辑求值所需字段。

3. `GoalSource` 不等于 `ObligationCause`。

   `GoalSource` 描述父 goal 为什么产生这个 nested goal，并参与 cycle path classification；`ObligationCause` 服务于外层诊断与证明链来源。

4. `Certainty::Yes` 不表示 response 没有约束。

   它可以同时返回 `?T = u32`、region constraints 或 opaque constraints。

5. ambiguity 不等于失败。

   `Maybe` 表示当前求值不能稳定决定；`NoSolution` 才表示该路径不成立。

6. decomposition 不等于 candidate enumeration。

   trait/projection goal 需要 candidates；outlives、subtype、WF 等 goal 有各自的直接规则或结构分解。

7. nested goal 的 `Maybe` 不会立刻结束整个 candidate。

   若本轮有 inference progress，fixpoint 会重试；只有到达无进展状态才合并为最终 `Maybe`。

8. coinduction 不是“所有递归 trait goal 都成功”。

   search graph 根据 `GoalSource`、当前 goal kind 和整条 cycle path 判断 provisional result。

## 本章小结

`Goal<I, P>` 是 next solver 的基本逻辑单元：它用 `param_env` 表示 assumptions，用 `predicate` 表示待证明命题。普通 goal 在 `evaluate_goal_raw` 中 canonicalize，经 search graph 和 `enter_canonical` 进入 query-local `EvalCtxt`。`compute_goal` 打开 binder 后按 `PredicateKind` / `ClauseKind` 分派；每种 goal kind 可以修改推理状态、登记外部约束或通过 `add_goal(s)` 产生 nested goals。nested goals 按 `HasChanged` 驱动的 fixpoint 迭代求值，最终形成 `Yes`、`Maybe` 或 `NoSolution`。`GoalSource` 同时记录分解来源并影响 search graph 的 inductive/coinductive cycle 处理。
