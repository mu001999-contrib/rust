---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "17"
document: content
status: exercises_assigned
updated_at: 2026-09-25
---

# 17. 高阶类型与量化约束

## 学习目标

- 把 `for<'a>` 理解为量化约束，而不是“尚未推断出的某个生命周期”。
- 区分 binder 中的 bound variable、进入 binder 后的 placeholder，以及某次实例化产生的 infer variable。
- 按量词顺序分析 HRTB trait goal、ParamEnv assumption 与 impl candidate。
- 手算高阶函数子类型关系，指出哪一侧替换为 placeholder、哪一侧实例化为推理变量。
- 解释 universe nameability、placeholder leak、region checking 的分工，并追踪当前 rustc 源码。

## 前置知识

第 02 章的 `Binder` / de Bruijn index，第 04 章的 `ReBound` / `RePlaceholder` / universe，第 06 章的函数参数逆变，第 10–12 章的 canonical goal、candidate 与 response。第 13 章的 GAT 可作为“binder 不只出现在函数指针里”的例子。

## 核心心智模型

用逻辑式先判断谁要适应谁：

```text
证明  for<'a> Trait<&'a T>
      ↑ 必须对任意 'a 成立：用刚性 placeholder P@U1 代表挑战者选择的 'a

使用  for<'a> Trait<&'a T> 这一已有前提，去证明某个具体 Trait<&'x T>
      ↑ 只需选一个实例：用新 infer var ?r，再由匹配决定 ?r = 'x
```

`ReBound(D0, var)` 是还处于量词之下的 IR 名称；`RePlaceholder(U1, bound)` 是打开全称量词后的刚性代表；`ReVar(?r@U1)` 是求“存在一个合适实例”时创建、可以被约束的未知量。同一 bound variable 重复出现，替换后仍是同一个 placeholder 或 infer var；两个不同 bound variable 不能仅因拼写相似而合并。

高阶子类型的原则是“目标（super）的**任意**实例，都能由来源（sub）的**某个**实例满足”。写成 `∀目标. ∃来源. 来源 <: 目标`。这个顺序决定：协变位置里先把 super 的 binder 打开为 placeholder，再把 sub 的 binder 实例化为 infer vars；逆变位置则交换两侧；不变关系两方向都检查。子类型内部若遇到函数参数，还要再应用一次函数参数逆变，这与外层 binder 的量词顺序是两件事。

## 源码地图

| 路径 | 符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_type_ir/src/binder.rs` | `Binder`、`bound_vars`、`skip_binder` | 量词和被绑定变量的 IR 容器 |
| `compiler/rustc_infer/src/infer/relate/higher_ranked.rs` | `InferCtxt::enter_forall_and_leak_universe`、`enter_forall`、`leak_check` | 打开全称量词、创建 universe 与旧式早期检查 |
| `compiler/rustc_infer/src/infer/mod.rs` | `InferCtxt::instantiate_binder_with_fresh_vars` | 为一次可选择的实例创建 ty/region/const 推理变量 |
| `compiler/rustc_infer/src/infer/relate/type_relating.rs` | `TypeRelating::binders` | 旧 relation 的高阶子类型和不变关系 |
| `compiler/rustc_type_ir/src/relate/solver_relating.rs` | `SolverRelating::binders` | next solver 中相同量词方向的关系检查 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` | `EvalCtxt::compute_goal`、`enter_forall_with_assumptions`、`evaluate_added_goals_and_make_canonical_response` | 高阶 goal、placeholder 假设、响应前 leak 检查 |
| `compiler/rustc_next_trait_solver/src/solve/trait_goals.rs` | `TraitGoal::match_assumption` | 把 caller bound 作为可选实例使用 |
| `compiler/rustc_infer/src/infer/region_constraints/leak_check.rs` | `RegionConstraintCollector::leak_check` | 检查不能从外层 universe 命名的 placeholder 是否泄漏 |

相关官方说明：[HRTB](https://rustc-dev-guide.rust-lang.org/traits/hrtb.html)、[实例化 binder](https://rustc-dev-guide.rust-lang.org/ty_module/instantiating_binders.html)、[placeholder 与 universe](https://rustc-dev-guide.rust-lang.org/borrow_check/region_inference/placeholders_and_universes.html)。官方 HRTB 页面保留了较早的 taint-set / plug-leaks 讲法；本章实现细节以当前检出的源码为准。

## 源码精读

### 1. `Binder` 保存量词边界

来自 `compiler/rustc_type_ir/src/binder.rs` 的 `Binder`（省略 derive 与其他方法）：

```rust
pub struct Binder<I: Interner, T> {
    value: T,
    bound_vars: I::BoundVarKinds,
}

pub fn bound_vars(&self) -> I::BoundVarKinds {
    self.bound_vars
}
```

`value` 中可包含 `ReBound` 等变量，`bound_vars` 给出这一层 binder 的变量种类与顺序。`for<'a, 'b>` 的两个变量占两个位置；`for<'a> fn(&'a T, &'a T)` 的两次使用指向同一个位置。`skip_binder` 只是取出内部值，不等于实例化；源码文档明确警告，不能把可能逃逸的 bound vars 当普通自由变量使用。

### 2. 全称量词变为 placeholder

来自 `compiler/rustc_infer/src/infer/relate/higher_ranked.rs` 的 `InferCtxt::enter_forall_and_leak_universe`（只展示核心分支；省略快速返回、debug 与闭包细节）：

```rust
let next_universe = self.create_next_universe();

let delegate = FnMutDelegate {
    regions: &mut |br| {
        ty::Region::new_placeholder(self.tcx, ty::PlaceholderRegion::new(next_universe, br))
    },
    types: &mut |bound_ty| {
        Ty::new_placeholder(self.tcx, ty::PlaceholderType::new(next_universe, bound_ty))
    },
    consts: &mut |bound_const| {
        ty::Const::new_placeholder(
            self.tcx, ty::PlaceholderConst::new(next_universe, bound_const),
        )
    },
};
self.tcx.replace_bound_vars_uncached(binder, delegate)
```

新 universe 不只服务 lifetime：bound type 和 bound const 也有 placeholder。`(universe, bound identity)` 一起区分 placeholder；只写 `P@U1` 是本章的简写。已经存在于 U0 的 `?r@U0` 不能把 U1 新名称当作自己的解。

### 3. 一次实例化变为 infer vars

来自 `compiler/rustc_infer/src/infer/mod.rs` 的 `InferCtxt::instantiate_binder_with_fresh_vars`（省略 span/origin 和替换 delegate）：

```rust
for bound_var_kind in bound_vars {
    let arg: ty::GenericArg<'_> = match bound_var_kind {
        ty::BoundVariableKind::Ty(_) => self.next_ty_var(span).into(),
        ty::BoundVariableKind::Region(br) => {
            self.next_region_var(RegionVariableOrigin::BoundRegion(span, br, lbrct)).into()
        }
        ty::BoundVariableKind::Const => self.next_const_var(span).into(),
    };
    args.push(arg);
}
```

每个 bound-var **位置**产生一个新变量，同一位置的多次引用复用该变量。它们处于当前 universe：在 `enter_forall` 的闭包**内部**调用此函数，才会创建能命名刚引入的 placeholder 的新变量。不能把“所有 infer vars 都可指向 P”当成一般规则。

### 4. 两种 relation 的量词方向

来自 `compiler/rustc_infer/src/infer/relate/type_relating.rs` 的 `TypeRelating::binders`（仅协变/逆变分支）：

```rust
ty::Covariant => {
    infcx.enter_forall(b, |b| {
        let a = infcx.instantiate_binder_with_fresh_vars(span, HigherRankedType, a);
        self.relate(a, b)
    })?;
}
ty::Contravariant => {
    infcx.enter_forall(a, |a| {
        let b = infcx.instantiate_binder_with_fresh_vars(span, HigherRankedType, b);
        self.relate(a, b)
    })?;
}
```

`a` 是左侧、`b` 是右侧；协变时右侧是必须应付的任意目标。若 binder 处于逆变上下文，实际子类型方向反转，故换为左侧的任意实例。不变分支在源码中双向执行，不能只测一个方向。`SolverRelating::binders` 使用 `enter_forall_with_empty_assumptions` 和 `instantiate_binder_with_infer`，但量词方向相同。

### 5. 高阶 goal 与 assumption 走相反动作

`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` 的 `EvalCtxt::compute_goal`：

```rust
let Goal { param_env, predicate } = goal;
let kind = predicate.kind();
self.enter_forall_with_assumptions(kind, param_env, |ecx, kind| {
    Ok(match kind {
        // 按 PredicateKind 分派；此处省略其他分支
        ty::PredicateKind::Clause(ty::ClauseKind::Trait(predicate)) => {
            ecx.compute_trait_goal(Goal { param_env, predicate }).map(|(r, _via)| r)?
        }
        // ...
    })
})
```

`compiler/rustc_next_trait_solver/src/solve/trait_goals.rs` 的 `TraitGoal::match_assumption`：

```rust
let assumption_trait_pred = ecx.instantiate_binder_with_infer(trait_clause);
ecx.eq(goal.param_env, goal.predicate.trait_ref, assumption_trait_pred.trait_ref)?;
then(ecx)
```

前者把**待证明的 goal** 的 binder 打开，接受任意代表；后者把**可使用的前提**实例化，选择一个能匹配当前 goal 的实例。`enter_forall_with_assumptions` 还会在启用相应机制时构造该 binder 的 placeholder region assumptions；这里传入的 `param_env` 用于计算它们，并不直接等于这些 assumptions。

### 6. leak check 的当前地位

`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` 的 `evaluate_added_goals_and_make_canonical_response`（省略目标求值与响应构造）：

```rust
let goals_certainty = match self.delegate.cx().assumptions_on_binders() {
    true => {
        let certainty = self.eagerly_handle_placeholders()?;
        certainty.and(goals_certainty)
    }
    false => {
        self.delegate.leak_check(self.max_input_universe).map_err(|NoSolution| {
            trace!("failed the leak check");
            NoSolution
        })?;
        goals_certainty
    }
};
```

后一分支只检查 query 内进入的新 universe。`compiler/rustc_infer/src/infer/region_constraints/leak_check.rs` 的 `RegionConstraintCollector::leak_check` 注释指出，它会识别例如不同 placeholder 被迫相关、以及 `P: R` 且 R 所在 universe 不能命名 P 等模式；实际实现构造 region-constraint 图并分析 SCC。当前 `InferCtxt::leak_check` 注释也明确：这个早期检查处于过渡期，合法的子类型错误最终还会由后续 region checking 检出。启用 binder assumptions 时，上面的 next-solver 分支改走 `eagerly_handle_placeholders`，因此不要把“每次高阶 goal 都调用同一个 leak_check 函数”当作不变量。

## 正文

### 一、`for<'a>` 究竟承诺什么

`for<'a> fn(&'a u32)` 可接收调用方提供的任意有效 `'a`；它不是某次调用里固定的、只是还不知道是哪一个的 `'a`。例如：

```rust
fn accepts_any(f: for<'a> fn(&'a u32)) {
    let local = 7;
    f(&local);
}
fn only_static(_: &'static u32) {}
// accepts_any(only_static); // 不满足：only_static 不能接收上面的短借用
```

进入量词前，`'a` 在 IR 中是 bound region。要**证明**承诺，则创建新 universe U1，给它一个刚性名称 `P_a@U1`，证明这一个任意代表。由于它没有被挑成 `'static` 的自由，若对它成立才可推广到所有 `'a`。要**使用**承诺，则可选择某个实例，如让它接受 `&'local u32`。

### 二、goal、assumption 和 impl candidate 的方向

设：

```rust
trait Take<X> {}
struct Any;
impl<'a> Take<&'a u32> for Any {}
struct Static;
impl Take<&'static u32> for Static {}
```

目标 `Any: for<'a> Take<&'a u32>`：goal 一侧先用 U1 的 `P_a` 变成 `Any: Take<&P_a u32>`；泛型 impl 的 `'a` 是候选可选的实例，创建 `?r@U1`，统一后可取 `?r = P_a`，所以可成功。这里的 impl 泛型参数实例化与 goal binder 实例化是不同操作，不能都叫“展开 `'a`”后忽略方向。

目标 `Static: for<'a> Take<&'a u32>`：唯一 impl 只提供 `&'static u32`。若强迫它适应任意 `P_a`，就需要不被许可的 `P_a = 'static`（在本例的 invariant trait-arg 匹配中）；这不能证明全称目标。反过来，若 ParamEnv 已有 `Any: for<'a> Take<&'a u32>` 这一 assumption，要证明 `Any: Take<&'x u32>`，可以把 assumption 实例化成 `?r` 并取 `?r = 'x`。单个 `Take<&'static u32>` assumption 则不推出高阶 goal。

带 where-clause 的候选还会生成嵌套 goal。例如 `impl<X, F> Take<X> for F where F: Other<X>` 对高阶 goal 匹配后，`X` 可能含该 goal 的 placeholder；这类约束必须在可正确处理 placeholder 的作用域和 response 中验证，不能将 U1 的名称写进 U0 的永久答案。

### 三、高阶子类型：两次“翻转”要分别记

检查 `fn(&'static u32) <: for<'a> fn(&'a u32)`：

1. 右侧是 super 的全称目标；打开它得到 `fn(&P_a u32)`，`P_a@U1`。
2. 函数参数逆变，因此必须证明 `&P_a u32 <: &'static u32`。
3. 共享引用对 region 协变，因此需要 `P_a: 'static`。任意 `P_a` 没有这项保证，关系失败。

反向的 `for<'a> fn(&'a u32) <: fn(&'static u32)` 可成立：目标只要一个 `'static` 实例；来源的 binder 可实例化为 `'static`。这说明“函数参数逆变”和“来源 binder 可选择实例”是两个不同层级的规则。真正的函数 item 到函数指针转换、调用点 coercion 还会经过其他检查；这里仅分析指定的类型关系。

对两个 binder 的关系 `for<'a> fn(&'a T) <: for<'b> fn(&'b T)`，右侧 `'b` 先变 U1 中的 `P_b`，左侧 `'a` 再在 U1 中变 `?r`，取 `?r = P_b` 即可。把名字 `'a`/`'b` 改写不改变类型（alpha-equivalence）。若在 U0 先创建 `?r@U0` 再打开右侧 binder，`?r@U0` 不能取 `P_b@U1`；创建顺序是算法正确性的一部分。

### 四、嵌套 binder、de Bruijn 与 universe

考虑概念形态 `for<'a> fn(for<'b> fn(&'a u32, &'b u32))`。在内层 `for<'b>` 的值中，`'b` 是 `D0`，向外跨一层才找到 `'a`，所以它是 `D1`。进入外层量词后，`'a` 可成为 U1 的 placeholder；之后再进入内层量词时，`'b` 可成为 U2 的 placeholder。注意 de Bruijn depth 描述“IR 中隔了几层 binder”，universe 描述“实例化之后哪些新名称可以被命名”，它们不是同一个计数器。

名字作用域可用词法作用域作类比：U0 的旧变量不能依赖 U1 的新名称；U1 中新建的变量可以引用 U1 和祖先 U0 的名称。实现常用 universe index 做可命名性判断，但同一个 index 并不足以识别 placeholder，还需 bound identity。

### 五、leak、response 与后续 region 检查

如果尝试把 U1 的 `P_a` 固定进 U0 中已有 `?r0` 的解，或迫使任意 `P_a` 等于 `'static`，就把任意量词偷换成某个特例。`leak_check` 是尽早发现一类不合法 region 约束的机制；它不是赋予 placeholder 唯一性的地方，也不是所有 region 错误的唯一最终判官。当前 rustc 会在不同实现路径中使用 nameability、早期 leak check、placeholder assumptions 处理以及后续 region solver 检查。

对于 trait solver，candidate 的临时匹配发生在 probe/局部求解中；向外返回 canonical response 时，不能把仅在内部新 universe 才可命名的 placeholder 当作外层具体解。响应可以记录合法的变量解、region constraints 和 certainty，调用方再应用它们。第 10–12 章的“候选结果通过 response 合并而非直接提交”在高阶 goal 中仍然成立，只是这里多了一层量词/作用域合法性。

## 常见误区

1. “`for<'a>` 只是一个未解的 `?r`”：前者是对任意实例的承诺，后者代表可选择的一个解。
2. “两边 binder 都换成 placeholder”：子类型/goal 证明有量词方向；可选择的一边通常用 infer vars。
3. “`P@U1` 仅凭 U1 就唯一”：还须 bound variable identity；同一 universe 可容纳多个 placeholder。
4. “只要有 `R: P` 就是 leak”：`R` 可能有合法的更长解；当前早期检查的模式不是简单的“碰到 placeholder 就拒绝”。
5. “leak check 是唯一错误检查”：当前源码明确区分过渡性的早期 leak 检查与后续 region checking，next solver 还存在 binder assumptions 分支。
6. “`skip_binder` 等于实例化”：它不替换变量；误把 bound var 当自由变量会造成作用域错误。

## 本章小结

先辨认量词：要证明全称目标，就由对方选任意 placeholder；要使用已有全称前提或实例化候选，就由当前求解选择 infer-var 实例。高阶子类型遵循 `∀super. ∃sub`，再把函数参数逆变等内部 variance 逐层应用。de Bruijn 定位未打开的 binder，universe 约束打开之后的名称可见性；leak/nameability/region checking 共同防止局部 placeholder 逃逸。下一章会把这套高阶 region 约束接到 MIR NLL 的求解过程上。
