---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "12"
document: content
status: graded
updated_at: 2026-09-05
---

# 12. Trait Solver：候选搜索

## 学习目标

1. 区分 impl、param-env、builtin、alias-bound 和 object-bound 候选来源。
2. 追踪 relevant impl 索引、fast reject、impl header matching 和 where-clause 求值。
3. 解释 candidate probe、canonical response 与推理状态隔离。
4. 按当前规则判断多个候选的合并、ambiguity 和 NoSolution。
5. 区分 trait goal 的证明与最终 codegen 实例选择。

## 前置知识

第 03 章的泛型实例化，第 05 章的 inference/probe，第 07–09 章的 ParamEnv、obligation 和 normalization，第 10–11 章的 canonical response、nested goals、fixpoint 与 cycle。

## 核心心智模型

一个 candidate 是一种可能证明当前 goal 的途径。当前 next solver 的 assembly 已经包含 evaluation：它不是先收集所有未经检查的 impl ID 再统一证明，而是逐个来源试算，保存来源和 canonical response。

一个候选内部的条件是 AND：header relation 与所有必需 where-clauses 都要满足。不同候选是不同证明途径，逻辑上是 OR；但当候选携带不同推理约束时，solver 还必须判断可以给调用方什么统一回答。

本章以普通正向 trait goal、Typeck 环境为主。coherence、opaque rerun、specialization 与 normalization 的候选偏好保留边界说明，以当前检出源码为准。

## 源码地图

| 路径 | 关键符号 | 职责 |
|---|---|---|
| `compiler/rustc_next_trait_solver/src/solve/trait_goals.rs` | `compute_trait_goal`、`consider_impl_candidate`、`match_assumption`、`merge_trait_candidates` | trait 候选入口、匹配与偏好 |
| `compiler/rustc_next_trait_solver/src/solve/assembly/mod.rs` | `Candidate`、`assemble_and_evaluate_candidates`、`assemble_impl_candidates`、`assemble_param_env_candidates` | 收集并求值多种来源 |
| `compiler/rustc_middle/src/ty/trait_def.rs` | `TyCtxt::for_each_relevant_impl` | blanket 和 Self 外形索引 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/probe.rs` | `ProbeCtxt::enter_inner`、`TraitProbeCtxt::enter` | 候选隔离，打包响应和 cycle 使用信息 |
| `compiler/rustc_next_trait_solver/src/solve/mod.rs` | `try_merge_candidates`、`bail_with_ambiguity`、`flounder` | 响应合并与保守结果 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` | `add_goals`、`evaluate_added_goals_and_make_canonical_response` | 子目标求值与结果导出 |

## 源码精读

### 1. Candidate 已经携带求值结果

位置：`solve/assembly/mod.rs`，`Candidate`。省略派生属性：

```rust,ignore
pub(super) struct Candidate<I: Interner> {
    pub(super) source: CandidateSource<I>,
    pub(super) result: CanonicalResponse<I>,
    pub(super) head_usages: CandidateHeadUsages,
}
```

`source` 表示从哪里证明；`result` 包含 certainty、变量替换与外部约束等；`head_usages` 记录该候选对 cycle head 的依赖，用于 fixpoint 管理。`result` 可以是 `Maybe`，并非每个保存的 candidate 都已经确定成立。返回 `NoSolution` 的普通 impl 分支不会加入成功收集的 candidate 列表。

入口 `solve/trait_goals.rs::compute_trait_goal` 的核心调用是：

```rust,ignore
let (candidates, failed_candidate_info) =
    self.assemble_and_evaluate_candidates(goal, AssembleCandidatesFrom::All)?;
let candidate_preference_mode =
    CandidatePreferenceMode::compute(self.cx(), goal.predicate.def_id());
self.merge_trait_candidates(candidate_preference_mode, candidates, failed_candidate_info)
    .map_err(Into::into)
```

### 2. impl header 与 where-clauses 使用同一份 fresh args

位置：`solve/trait_goals.rs::consider_impl_candidate`。以下为 probe 内部片段，省略前面的 rigid fast reject、polarity 检查，以及后面的 `impl_super_outlives` 和回调：

```rust,ignore
let impl_args = ecx.fresh_args_for_item(impl_def_id.into());
ecx.record_impl_args(impl_args);
let impl_trait_ref = impl_trait_ref.instantiate(cx, impl_args).skip_norm_wip();

ecx.eq(goal.param_env, goal.predicate.trait_ref, impl_trait_ref)?;
let where_clause_bounds = cx
    .predicates_of(impl_def_id.into())
    .iter_instantiated(cx, impl_args)
    .map(Unnormalized::skip_norm_wip)
    .map(|pred| goal.with(cx, pred));
ecx.add_goals(GoalSource::ImplWhereBound, where_clause_bounds)?;
```

header relation 和 where-clause 实例化复用 `impl_args`，因此 header 中约束出的推理变量会被后面的条件观察到。`eq` 也可能产生 relation nested goals；`add_goals` 会涉及 eager normalization。实际求值并非只有“一次 eq + 一条 trait bound”。

### 3. 候选结果跨越 probe，候选的局部赋值不直接提交

位置：`solve/eval_ctxt/probe.rs::ProbeCtxt::enter_inner`。以下省略 nested EvalCtxt 的构造以及诊断、opaque-access 收尾：

```rust,ignore
let r = nested.delegate.probe(|| {
    let r = f(&mut nested);
    nested.inspect.probe_final_state(delegate, max_input_universe);
    r
});
```

`TraitProbeCtxt::enter` 随后打包：

```rust,ignore
let (result, head_usages) = self.cx.enter_single_candidate(f);
Ok(Candidate { source: self.source, result: result?, head_usages })
```

即使 candidate 返回 `Ok`，probe 中的 inference 试探也不等于已经写回调用方。响应先 canonicalize 成可以离开局部上下文的数据，候选合并后由 query-response 实例化路径应用结果。诊断、缓存和 cycle 使用信息有专门的保留机制，不能把 probe 理解成“所有编译器状态都消失”。

### 4. 合并比较 response，而不仅是 certainty

位置：`solve/mod.rs::try_merge_candidates`。省略函数签名，保留核心逻辑：

```rust,ignore
if candidates.is_empty() {
    return None;
}

let always_applicable = candidates.iter().enumerate().find(|(_, candidate)| {
    candidate.result.value.certainty == Certainty::Yes
        && has_no_inference_or_external_constraints(candidate.result)
});
if let Some((i, c)) = always_applicable {
    return Some((c.result, MergeCandidateInfo::AlwaysApplicable(i)));
}

let one: CanonicalResponse<I> = candidates[0].result;
if candidates[1..].iter().all(|candidate| candidate.result == one) {
    return Some((one, MergeCandidateInfo::EqualResponse));
}

None
```

一个无 inference/external constraints 的 `Yes` response 足以回答当前 goal；否则，所有 canonical responses 完全相等也可合并，包括只有一个候选的情况。若不满足这些规则，调用方根据候选偏好或 `flounder` 返回保守结果。

`flounder` 对空候选返回 `NoSolution`；对无法合并的非空候选调用 `bail_with_ambiguity`，生成无候选特有约束的 ambiguous response，同时汇总 Maybe 信息。这不是求任意多份约束的交集算法。

## 正文

### 1. 去哪里找 candidate？

主要来源：

- impl：用户或库中声明的实现。
- ParamEnv：调用环境已有的 assumptions，通常来自 where-clauses。
- builtin：编译器内建规则，例如 auto trait 的字段分解、部分 `Sized` 和函数调用 trait 行为。
- alias bounds：关联类型或 opaque 等声明上的 item bounds。
- object bounds：`dyn Trait` 携带的 trait/object 信息。

`assemble_and_evaluate_candidates` 先结构性归一化 Self，解析已知变量，然后依条件尝试 alias bounds、ParamEnv、builtin 和 impl/object 候选。排列顺序不等于“第一个成功就永久选中”。

普通 Self 若仍为未解类型变量，当前实现通常走保守 ambiguity，而不是遍历所有具体类型来猜它是什么；registered opaque 有专门路径。这与 `Store: Pick<?A>` 不同：后者 Self 已知，只是其他 trait argument 未知，可以在匹配时约束 `?A`。

### 2. relevant impl 与 fast reject：先缩小范围，再真正匹配

`TyCtxt::for_each_relevant_impl` 从该 trait 的 impl 集合中遍历 blanket impls，再根据 Self 的 simplified type 找 non-blanket bucket。无法取得适用索引时有更宽的遍历路径。

```rust,ignore
impl<T> SomeTrait for T { /* ... */ }         // Self 无固定外形
impl<T> SomeTrait for Container<T> { /* ... */ } // Self 有 Container 外形
```

第二条即使有泛型，也能按 Container 索引。两条是索引形状示意，并非建议在同一程序里同时声明重叠 impl。

取到 impl 后，`DeepRejectCtxt::args_may_unify` 快速排除明显不匹配的参数；通过这一筛查只表示值得进入 probe，尚未证明 header、where-clauses 成立。

### 3. 完整匹配实例：约束怎样传到 where-clause？

```rust
struct Store<T>(T);
trait Convert<A> {}
impl<K: Clone> Convert<K> for Store<K> {}
```

假设 query-local goal 是 `Store<?X>: Convert<String>`。省略隐式 sizedness 条件：

1. 进入该 impl 的 probe，为 K 创建 `?K`。
2. 实例化 header 得到 `Store<?K>: Convert<?K>`。
3. 对完整 trait-ref 做 eq：`Store<?X> == Store<?K>` 且 `String == ?K`，得到 `?X = ?K = String`。
4. 同一份 args 实例化 where-clause 得到 `?K: Clone`，解析后是 `String: Clone`。
5. 子目标成功，candidate response 记录输入变量 `?X = String` 与 `Yes`。局部 `?K` 不会裸露给调用方。
6. 若合并保留这份响应，query 调用方实例化它，才把等式应用到自己的输入推理变量。

第 3 步的 eq 可以产生其他 relation goals；第 4 步继承的是当前 goal 的 ParamEnv，并以 impl args 替换 predicates。它没有把 `K: Clone` 当作不需证明的新 assumption。

### 4. ParamEnv candidate 也要匹配

在 `fn f<T: Clone>()` 内证明 `T: Clone`，可以使用已有 assumption。若外层 goal 是 `Store<T>: Convert<T>`，先由 impl 产生 `T: Clone`，再由 ParamEnv 证明子目标。

`assemble_param_env_candidates` 遍历 `caller_bounds`，为每条相关 assumption 试探匹配。`match_assumption` 会实例化 assumption 的 binder，再 eq 两个 trait refs；关系要求同样需要处理。并非任意 clause 都可证明任意 goal。

两个必要子条件 `T: Clone`、`T: Send` 可以分别用环境中的两条 clause 证明；无需把两条 clause 拼成一个候选。单个 impl candidate 内的子目标队列负责 AND。

### 5. 为什么两个 Yes 仍可得到 Maybe？

```rust
struct Selector;
trait Pick<A> {}
impl Pick<u32> for Selector {}
impl Pick<bool> for Selector {}
```

两条 impl 的 trait 参数不同，可以共存。对 goal `Selector: Pick<?A>`，它们分别产生：

```text
candidate 1: Yes，条件 ?A = u32
candidate 2: Yes，条件 ?A = bool
```

这里 Yes 的准确含义是“在本 response 的约束下，该证明成功”。第一条并没有证明任意 `?A` 都可行。

两个 probe 互相隔离，因此第一个候选的临时 `?A = u32` 不会污染第二个。两份 response 不能按 AND 同时应用，也不能根据 impl 遍历顺序任意挑一个。若没有其他偏好规则，合并保留 ambiguity。之后外部若确定 `?A = u32`，重新求值就可以排除 bool 分支。

反过来，多条候选若返回完全相同的 response，多个来源可以得到确定回答。证明 goal 也不等同于已经确定最终 codegen 使用哪个 impl instance；第 20 章继续讨论后者。

#### 5.1 不同 candidate 为何可以合并相同 response？

关键是区分“证明从哪里来”和“这次求解向调用方返回什么”。不同 candidate 可以是不同的证明途径，但给调用方完全相同的求解答案。

trait goal 求值要回答的是：这个要求是否成立，以及需要如何约束输入变量、增加哪些外部约束。它不要求把某个唯一 impl ID 作为这次 canonical response 的返回值。

当前 `compiler/rustc_type_ir/src/solve/mod.rs::Response` 的字段是（省略属性）：

```rust,ignore
pub struct Response<I: Interner> {
    pub certainty: Certainty,
    pub var_values: CanonicalVarValues<I>,
    pub external_constraints: I::ExternalConstraints,
}
```

`CandidateSource` 则在 `Candidate` 中，和 `result` 并列。候选来源不是 Response 的字段。

假定候选偏好筛选已经结束，两个候选剩下的完整响应相同：

```text
C1：source = S1，result = R
C2：source = S2，result = R

R：certainty = Yes，输入 ?A 的值 = u32，无其他约束
```

无论采用哪条证明途径，本次求解都告诉调用方同样的事情：“令 ?A = u32，goal 成立。”调用方无需在两个不同类型、区域要求或其他约束之间作选择；返回 R 已经完整表达这两条途径共同给出的答案。

从不同证明途径为 OR 的角度，直觉上就是 `R OR R = R`。这只是说明相同答案可以去重，不是在把两个 impl 声明合成一条 impl，也不是把各候选的所有约束按 AND 叠加。

对比不同 response：

```text
C1：Yes，?A = u32
C2：Yes，?A = bool
```

这里返回 u32 或 bool 会影响调用方后续的类型检查。没有其他依据就不能任意选择，因此保留 ambiguity。造成这种歧义的是求解答案的差异，而不仅是存在两个来源。

#### 5.2 源码合并的是 result，不是要求 Candidate 完全相等

`compiler/rustc_next_trait_solver/src/solve/mod.rs::try_merge_candidates` 的 EqualResponse 分支明确写的是：

```rust,ignore
let one: CanonicalResponse<I> = candidates[0].result;
if candidates[1..].iter().all(|candidate| candidate.result == one) {
    return Some((one, MergeCandidateInfo::EqualResponse));
}
```

这里拿第一份 response 作为相等比较的代表；不是根据遍历顺序选择第一条 impl 来执行。既然所有 result 都相等，拿哪一份 result 返回都一样。

“完全相同”包含 canonical 包装信息和 Response 的全部内容，不只是 certainty 都为 Yes，也不只是简写中显示了同一个类型。比如 var_values 相同但 region constraints、opaque types 或 normalization nested goals 不同，就不能仅凭类型相同进入这个分支。

相同的 Maybe responses 也可以合并，但仍返回 Maybe；合并本身不会把尚未确定的证明提升为 Yes。

#### 5.3 那么不同 impl 的方法体怎么办？

goal 求值与具体方法实例解析是不同任务。generic body 可以依赖 `T: Trait` assumption 通过检查，而不在这个阶段选出所有未来具体 T 的方法体。需要具体实例时，编译器另有实例解析流程；例如 `compiler/rustc_ty_utils/src/instance.rs::resolve_instance_raw` / `resolve_associated_item`，第 20 章继续精读。

也因此，EqualResponse 不能使两条原本冲突的普通 impl 变合法：coherence 仍独立检查两条 impl 是否存在不允许的重叠。同一具体 trait-ref 的两条冲突实现，不能靠“它们都回答 Yes”绕过 coherence。

另外，candidate 并不一定对应用户写的 impl。环境 assumption、builtin、alias bound 都是可能的证明途径。多种途径证明同一件事，与两个方法实现争用同一个具体实例，并不是同一个问题。

#### 5.4 来源仍在何处起作用？

EqualResponse 并非声明 source 在整个 solver 中都无关：

- `merge_trait_candidates` 在通用合并前处理环境、builtin、alias bounds、specialization 等来源偏好。
- trait 求值保留的 `TraitGoalProvenVia` 会影响随后 normalization 的候选策略。
- candidate 的 `head_usages` 由 search graph 管理，服务于 cycle fixpoint；例如 ParamEnv 合并的 EqualResponse 分支没有像 AlwaysApplicable 分支那样主动忽略其他候选的 head usages。

因此规则的边界是：在当前允许合并的候选集合内，完整 canonical responses 相同，就可以向调用方返回这一份相同的响应；来源策略、cycle 验证与 coherence 继续各司其职。

一句话：不同 candidate 可以是不同的“证明过程”；相同 response 表示这些过程对本次求解给出了同一个“答案”。合并答案不要求合并证明过程或选定运行时方法体。

### 6. candidate preference 位于通用合并之前

对 trait goals，`merge_trait_candidates` 的当前规则还包括：

- coherence 使用专门分支，不能照搬 Typeck 的偏好。
- trivial builtin（无 nested requirements 的内建候选）优先。
- marker 类 trait 对特定 Self alias bounds 有专门偏好。
- 存在 non-global ParamEnv candidates 时，优先合并环境候选；其后还有 alias-bound 偏好。
- 其他分支处理 specialization、dyn builtin 兼容规则与 global bounds，再进入普通合并。

assembly 本身也会按环境/alias 候选的无约束条件跳过部分 impl 搜索。因而“收集所有 impl 后随便挑一个”和“ParamEnv 永远优先于一切”都不足以描述当前实现。

本章习题涉及通用 response 合并时，会明确假定这些偏好筛选已经结束。`NormalizesTo` 的 `assemble_and_merge_candidates` 还依赖 `TraitGoalProvenVia`，不要与 trait goals 的 `merge_trait_candidates` 混为一个入口。

### 7. Trait solver、candidate 与 coherence 的关系

三者处于不同层次：coherence 是 impl 集合的合法性检查任务，trait solver 是它会调用的求解机制，candidate 是 solver 内部尝试证明一个 goal 的途径。

| 概念 | 主要问题 | 观察对象 |
|---|---|---|
| trait solver | 在给定环境与求解模式下，这个 goal 如何成立、需要什么约束？ | 一次 trait/type goal |
| candidate | 使用这个来源证明 goal，会得到什么响应？ | 某条 impl、环境 assumption、builtin 等证明途径 |
| coherence | 这些 impl 能否合法共存，是否存在不允许的适用范围重叠？ | impl 声明及其跨 crate 实现权限 |

普通 type checking 与 coherence 都可以调用 trait solver，但要回答的问题不同。coherence 并不是“求解完一个 goal 后，数一数 candidate 是否恰好为一”。

#### 7.1 Coherence 包含 orphan 与 overlap 两方面

- orphan check：当前 crate 是否有权声明这条 trait impl。判断涉及 trait/type 的归属、类型参数覆盖与 fundamental 类型等规则，不只是“有一个本地类型”这么简单。
- overlap check：两条 impl 的适用范围是否可能相交。普通 trait impl 若存在不允许的重叠则报错；specialization 等有专门规则。

概念说明可参阅 [Rust Compiler Development Guide：Coherence](https://rustc-dev-guide.rust-lang.org/coherence.html)。本节具体求解流程以当前仓库源码为准。

实现入口与辅助模块：

- `compiler/rustc_hir_analysis/src/coherence/mod.rs` 与 `coherence/orphan.rs`：impl 合法性及 orphan 检查。
- `compiler/rustc_trait_selection/src/traits/coherence.rs`：`overlapping_trait_impls`、`overlap`、`impl_intersection_has_impossible_obligation`。
- `compiler/rustc_next_trait_solver/src/coherence.rs`：`orphan_check_trait_ref`、`trait_ref_is_knowable`，为跨 crate 推理判断实现权限和可知性。

#### 7.2 Overlap 检查把“两个 impl 有没有交集”转化为求解问题

考虑两条 impl：

```rust,compile_fail
trait Render {}
impl<T: Clone> Render for T {}
impl Render for u32 {}
```

coherence 要问的是：是否存在同一份具体 trait-ref，使两条 impl 同时适用？

1. 第一条实例化为 `?T: Render`，附带要求 `?T: Clone`。
2. 第二条 header 是 `u32: Render`。
3. 等同两个 header，得到 `?T = u32`。
4. 把两边 predicates 与 header relation 产生的要求一起登记，核心子目标成为 `u32: Clone`。
5. solver 能证明这个条件，因而两条 impl 存在共同适用点 `u32: Render`，本例报 E0119。

注意此时 solver 为子目标寻找的是 **Clone 的 candidate**。外层正在比较两条 Render impl，不等于内层直接把这两条 Render impl 当作候选合并。

位置：`compiler/rustc_trait_selection/src/traits/coherence.rs::overlap`。以下为分段摘录，省略诊断、negative-impl、模式分支等代码：

```rust,ignore
let infcx = tcx
    .infer_ctxt()
    .skip_leak_check(skip_leak_check.is_yes())
    .with_next_trait_solver(tcx.next_trait_solver_in_coherence())
    .build(TypingMode::Coherence);

// ……两条 impl 分别以 fresh args 实例化……
let param_env = ty::ParamEnv::empty();

let mut obligations =
    equate_impl_headers(selcx.infcx, param_env, &impl1_header, &impl2_header)?;

obligations.extend(
    [&impl1_header.predicates, &impl2_header.predicates].into_iter().flatten().map(
        |&predicate| Obligation::new(infcx.tcx, ObligationCause::dummy(), param_env, predicate),
    ),
);
```

这里 `ParamEnv::empty()` 有实际用途：两条 impl 的泛型换成了用于寻找交集的 inference variables；它们的 where-clauses 被作为 obligations 检查，而非直接作为环境中的既定事实。`TypingMode::Coherence` 另外设置求解政策，和 `ParamEnv` 是不同维度。

#### 7.3 Coherence 需要排除“可能重叠”，Maybe 不足以证明不重叠

如果 header 不可能统一，就没有交集。header 能统一后，若某个必需条件被可靠地判为不可能成立，也可以排除交集。

而 `Maybe` 表示还不能排除条件成立，因此不能据此认定 impl 不重叠。普通 overlap 路径会保留潜在交集，再由外层检查是否属于允许的重叠。

当前 next-solver 路径的 `impl_intersection_has_impossible_obligation`：

1. 先用较浅深度的 `root_goal_may_hold_with_depth` 快速尝试排除。
2. 使用 `ObligationCtxt` 登记并求值整个条件集合，让共享 inference constraints 一起作用。
3. 真正的 hard error 可以排除交集；ambiguity 和 overflow 本身不构成不相交的证明。

这并不要求 solver 总能给出一个完整的具体重叠实例。“未能排除交集”也可能使普通 impl 对被拒绝。反之，某个候选失败并不够，必须对相应 goal/条件集合得到可靠结论。

#### 7.4 为什么当前没有 impl，也可能要保留 Maybe？

coherence 还要考虑其他 crate 按规则可以增加的实现：包括下游/兄弟 crate 的合法 impl，以及上游可兼容增加的 impl。它不是任意想象未来实现，而是用 orphan/knowability 规则约束可能性。

```rust,compile_fail
trait Local {}
impl<T: std::fmt::Display> Local for T {}
impl Local for Vec<u8> {}
```

header 交集要求 `T = Vec<u8>`，剩下的条件是 `Vec<u8>: Display`。即使当前没有这条实现，也不能简单推导两条 Local impl 永远不相交；上游拥有相关 trait/type，可以在未来增加对应实现。本例报 E0119，并提示上游未来可能增加该 Display impl。

对应 next solver 的 `solve/assembly/mod.rs::consider_coherence_unknowable_candidate`：

- 使用 `trait_ref_is_knowable` 判断是否可以只依据当前已知实现作结论。
- 若不可知，尝试 `CandidateSource::CoherenceUnknowable`，以 `Certainty::AMBIGUOUS` 为求值上限。
- 还会检查相关 supertrait 要求；这些要求若能被排除，也可能令这条途径失败。

这是一种表达跨 crate 不确定性的特殊 candidate，不是某个真实 impl 的 ID。

若 trait-ref 在规则下是 knowable，则可以进行可靠的 implicit negative reasoning；某些 overlap 模式也会利用显式 negative impl。因而 coherence 既不是封闭地只看当前正向 impl，也不是遇到任何缺失 impl 都一律返回 Maybe。

#### 7.5 多个 candidate 不等于违反 coherence

沿用前面的合法代码：

```rust
struct Selector;
trait Pick<A> {}
impl Pick<u32> for Selector {}
impl Pick<bool> for Selector {}
```

coherence 比较的是完整 trait-ref：`Selector: Pick<u32>` 与 `Selector: Pick<bool>`，其中 `u32` 与 `bool` 无法统一，因此两条 impl 不重叠。

但对未完成推断的 goal `Selector: Pick<?A>`，两条 impl 仍可分别成为 candidate，给出 `?A = u32` 和 `?A = bool`。这里的 ambiguity 来自调用点还没确定 trait argument，而不是 impl 集合不合法。

另外，同一个 goal 还可能同时有 ParamEnv 与 impl 两种证明来源。coherence 不要求所有 solver goals 只有一种证明来源；它检查的是 impl 适用范围及声明权限。

#### 7.6 TypingMode 会影响 candidate 和 cycle 政策

coherence 复用同一套 goal/canonicalization/candidate/probe 机制，但采用专门的保守规则。例如：

- assembly 会考虑 `CoherenceUnknowable`。
- `merge_trait_candidates` 在 coherence 下提前走专用分支，不套用普通 Typeck 的全部候选偏好。
- 第 11 章的纯 `Inductive` cycle，在当前 coherence 模式下初始响应保留 ambiguity，而不是像 Typeck 那样以 `NoSolution` 起步，避免过早把可能重叠判为不相交。

对应源码：`solve/assembly/mod.rs::assemble_and_evaluate_candidates`、`solve/trait_goals.rs::merge_trait_candidates`、`solve/search_graph.rs::initial_provisional_result`。

简要记忆：coherence 检查“规则能否共存”；solver 帮它检查“交集条件能否成立”；candidate 是检查每个条件时的一条可能证明途径。

验证记录：2026-09-05，使用本机 `rustc 1.99.0-nightly (d453bdd8f 2026-08-14)`，加 `-Znext-solver --emit=metadata` 分别检查以上三个完整最小程序（补充空 `main`）。Render 示例与 Local/Display 示例均产生 E0119；两条 Pick impl 的程序编译通过。

## 常见误区

- candidate 的 `Yes` 要与它的变量约束一起读。
- impl 具有泛型与它属于哪个 Self 索引 bucket 是两个问题。
- header 匹配成功后，还要验证 where-clauses 和 relation 产生的要求。
- candidate probe 返回 Ok 仍不等于提交其 inference 赋值；保留下来的是可合并的 response。
- 多个 candidate 与 ambiguity 不一一对应；多个不同来源可能给出相同结果。
- 普通未知 Self 的保守求值与已知 Self、未知 trait argument 的可推理性应分别分析。

## 本章小结

先按来源发现证明途径，再在隔离 probe 中实例化、匹配、检查子条件并导出 canonical response；之后按候选偏好与响应合并规则回答 goal。第 11 章解释“一个证明过程怎样推进”，第 12 章补上“多条证明途径怎样共同决定答案”。
