---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "08"
document: content
status: completed
updated_at: 2026-08-02
---

# 08. Obligation 与 Fulfillment

## 学习目标

完成本章后，应当能够：

1. 区分 `Predicate`、`Goal`、`Obligation` 与 `FulfillmentContext`。
2. 解释 `Obligation` 为什么要携带 `cause`、`param_env`、`predicate` 与 `recursion_depth`。
3. 追踪一个 obligation 从注册、选择、产生 nested obligations 到完成或报错的生命周期。
4. 解释 `ObligationForest` 中 `Unchanged`、`Changed(children)`、`Error(e)` 的含义。
5. 区分“暂时 ambiguous 留在队列里”和“最终 ambiguity 当作错误报告”。
6. 理解 `ObligationCause` / derived cause 如何服务诊断。
7. 把第 07 章的 `ParamEnv + Predicate` 心智模型接到 fulfillment 过程。

## 前置知识

- 第 05 章的 inference variables、snapshot 与 `resolve_vars_if_possible`。
- 第 06 章的 type relation 会产生 obligations。
- 第 07 章的 `Predicate`、`Clause`、`ParamEnv`、impl candidate 与 nested goals。

## 核心心智模型

第 07 章里我们把证明问题写成：

```text
Goal = ParamEnv + Predicate
```

第 08 章要加上类型检查器真正处理工作项时需要的上下文：

```text
Obligation
  = cause
  + param_env
  + predicate
  + recursion_depth
```

可以把它理解为：

```text
Predicate
  = 要证明什么

Goal
  = 在哪个 ParamEnv 下证明什么

Obligation
  = 为什么要证明、在哪里报错、递归深度是多少、在什么 ParamEnv 下证明什么

FulfillmentContext
  = 一批尚未完成的 obligations 的工作队列/森林
```

一次典型流程是：

```text
typeck / relation / normalization
  产生 PredicateObligation
  ↓
ObligationCtxt::register_obligation
  ↓
TraitEngine / FulfillmentContext
  ↓
ObligationForest::register_obligation
  ↓
try_evaluate_obligations 或 evaluate_obligations_error_on_ambiguity
  ↓
process_obligation
  ├─ Changed([])
  │   当前 obligation 完成
  ├─ Changed(children)
  │   当前 obligation 浅层成功，children 成为 nested obligations
  ├─ Unchanged
  │   信息不足，保留等待 inference 继续推进
  └─ Error(e)
      失败，带 backtrace 进入错误报告
```

一个很小的例子：

```rust,ignore
fn duplicate<T: Clone>(x: Vec<T>) -> Vec<T> {
    x.clone()
}
```

检查 `x.clone()` 时会产生类似：

```text
Obligation {
  cause: method call at x.clone(),
  param_env: [T: Sized, T: Clone],
  predicate: Vec<T>: Clone,
  recursion_depth: 0,
}
```

选择 `impl<T: Clone> Clone for Vec<T>` 后，它浅层成功，但产生 nested obligation：

```text
Obligation {
  cause: derived from proving Vec<T>: Clone,
  param_env: [T: Sized, T: Clone],
  predicate: T: Clone,
  recursion_depth: 1,
}
```

`T: Clone` 再由 `ParamEnv` candidate 完成。这里最重要的是：nested obligation 沿用同一个 `param_env`，但 `cause` 和 `recursion_depth` 会反映它是从父 obligation 推导出来的。

## 源码地图

| 路径 | 关键符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_infer/src/traits/mod.rs` | `Obligation`、`PredicateObligation`、`Obligation::as_goal` | obligation 的核心数据结构 |
| `compiler/rustc_trait_selection/src/traits/engine.rs` | `ObligationCtxt` | 类型检查中注册、归一化、关系检查后收集 obligations 的门面 |
| `compiler/rustc_trait_selection/src/traits/fulfill.rs` | `FulfillmentContext`、`PendingPredicateObligation`、`FulfillProcessor` | old solver fulfillment 主流程 |
| `compiler/rustc_data_structures/src/obligation_forest/mod.rs` | `ObligationForest`、`ProcessResult` | pending obligations 的树/森林结构 |
| `compiler/rustc_trait_selection/src/traits/select/mod.rs` | `SelectionContext` | old solver 的 trait selection 候选选择 |
| `compiler/rustc_infer/src/traits/util.rs` | `Elaboratable for PredicateObligation` | elaboration 时如何保留 cause/param_env 并派生 cause |
| `compiler/rustc_infer/src/infer/relate/type_relating.rs` | relation 产生 obligations | 类型关系检查如何把 deferred predicates 交给 fulfillment |

## 源码精读

### 1. `Obligation` 是带诊断上下文和深度的 goal

位置：`compiler/rustc_infer/src/traits/mod.rs`，`Obligation`。

核心结构是：

```rust,ignore
pub struct Obligation<'tcx, T> {
    pub cause: ObligationCause<'tcx>,
    pub param_env: ty::ParamEnv<'tcx>,
    pub predicate: T,
    pub recursion_depth: usize,
}
```

其中：

```text
cause
  为什么要证明它；错误应该指向哪里；derived obligation 如何回溯父原因

param_env
  在哪些 caller bounds 下证明

predicate
  要证明的命题

recursion_depth
  从父 obligation 派生了多少层，用于终止和 overflow 控制
```

`Obligation::as_goal` 会丢掉 `cause` 和 `recursion_depth`：

```rust,ignore
solve::Goal { param_env: self.param_env, predicate: self.predicate }
```

这说明 `Goal` 是 solver 可以消费的逻辑问题，而 `Obligation` 是 typeck/fulfillment 需要管理和诊断的工作项。

注意 `Obligation` 的 `PartialEq` / `Hash` 忽略 `cause` 和 `recursion_depth`，只比较：

```text
param_env + predicate
```

原因是同一个逻辑 obligation 可能从多个源码位置或多条派生路径出现；缓存和去重更关心“证明问题本身”。

### 2. `ObligationCtxt` 是注册 obligations 的门面

位置：`compiler/rustc_trait_selection/src/traits/engine.rs`，`ObligationCtxt`。

`ObligationCtxt` 包住：

```rust,ignore
pub struct ObligationCtxt<'a, 'tcx, E> {
    pub infcx: &'a InferCtxt<'tcx>,
    engine: RefCell<Box<dyn TraitEngine<'tcx, E>>>,
}
```

常见入口包括：

```rust,ignore
register_obligation(obligation)
register_obligations(obligations)
register_infer_ok_obligations(infer_ok)
register_bound(cause, param_env, ty, trait_def_id)
normalize(...)
eq(...)
sub(...)
evaluate_obligations_error_on_ambiguity()
```

这解释了为什么很多 typeck 操作返回 `InferOk { value, obligations }`：

```text
当前操作本身产生一个 value
同时产生一批必须稍后证明的 obligations
```

例如 `normalize`、`eq`、`sub` 都会把返回的 obligations 注册进当前 `ObligationCtxt`。这也是第 06 章 relation 和第 07 章 predicate 进入 fulfillment 的主要桥梁。

### 3. `FulfillmentContext` 把 obligations 放进森林

位置：`compiler/rustc_trait_selection/src/traits/fulfill.rs`，`FulfillmentContext`。

old solver 的 fulfillment context 主要保存：

```rust,ignore
pub struct FulfillmentContext<'tcx, E> {
    predicates: ObligationForest<PendingPredicateObligation<'tcx>>,
    usable_in_snapshot: usize,
    _errors: PhantomData<E>,
}
```

`PendingPredicateObligation` 是：

```rust,ignore
pub struct PendingPredicateObligation<'tcx> {
    pub obligation: PredicateObligation<'tcx>,
    pub stalled_on: Vec<TyOrConstInferVar>,
}
```

`stalled_on` 记录 obligation 上次因为哪些 inference variables 没有进展而卡住。下一轮如果这些变量没有变化，就可以跳过，避免重复做昂贵的 selection。

`ForestObligation::CacheKey` 明确包含：

```rust,ignore
ty::ParamEnvAnd<'tcx, ty::Predicate<'tcx>>
```

源码注释也说明原因：`ParamEnv` 会影响 fulfillment 成功或失败。这正好回扣第 07 章：同一个 `T: Clone` 在 `[T: Clone]` 和 `[]` 下结果不同。

### 4. `ObligationForest` 的三种处理结果

位置：`compiler/rustc_data_structures/src/obligation_forest/mod.rs`，`ProcessResult`。

`ProcessResult` 有三种：

```rust,ignore
pub enum ProcessResult<O, E> {
    Unchanged,
    Changed(ThinVec<O>),
    Error(E),
}
```

语义是：

```text
Unchanged
  当前信息不足，既不能证明也不能报错；保留 pending。

Changed([])
  当前 obligation 已完成，没有子 obligations。

Changed(children)
  当前 obligation 浅层成功，但依赖 children；children 注册为它的子节点。

Error(e)
  当前 obligation 失败；整棵 obligation tree 进入错误路径，并保留 backtrace。
```

这就是为什么 “select 到一个 impl” 不等于整条 obligation 立刻完成。它通常只是 `Changed(children)`：impl head 匹配成功，但 impl where-clauses 还要作为 children 继续证明。

### 5. trait obligation 如何处理

位置：`compiler/rustc_trait_selection/src/traits/fulfill.rs`，`FulfillProcessor::process_trait_obligation`。

核心分支是：

```rust,ignore
match self.selcx.poly_select(&trait_obligation) {
    Ok(Some(impl_source)) => {
        ProcessResult::Changed(mk_pending(obligation, impl_source.nested_obligations()))
    }
    Ok(None) => {
        stalled_on = args_infer_vars(...);
        ProcessResult::Unchanged
    }
    Err(selection_err) => {
        ProcessResult::Error(FulfillmentErrorCode::Select(selection_err))
    }
}
```

所以：

```text
Ok(Some(selection))
  找到了证明来源，产生 nested obligations。

Ok(None)
  信息不足，通常是存在未解析 inference vars；记录 stalled_on。

Err(...)
  已知无法证明，转成 fulfillment error。
```

`mk_pending` 会对子 obligation 调：

```rust,ignore
o.set_depth_from_parent(parent.recursion_depth)
```

因此 nested obligations 会继承父 obligation 的深度信息：

```text
parent depth = 0
child depth  = 1
grandchild   = 2
```

### 6. impl where-clauses 如何进入 `ImplSource::nested`

位置：

- `compiler/rustc_trait_selection/src/traits/select/confirmation.rs`，`confirm_impl_candidate`、`vtable_impl`
- `compiler/rustc_trait_selection/src/traits/select/mod.rs`，`impl_or_trait_obligations`
- `compiler/rustc_middle/src/traits/mod.rs`，`ImplSource::nested_obligations`

`process_trait_obligation` 看到的是：

```rust,ignore
Ok(Some(impl_source)) => {
    ProcessResult::Changed(mk_pending(
        obligation,
        impl_source.nested_obligations(),
    ))
}
```

所以 `T: Clone` 不是在 `process_trait_obligation` 这一行凭空产生的。它已经被装在 `impl_source` 里面。

对 user-defined impl，`poly_select` 确认候选时会走：

```text
confirm_impl_candidate
  -> rematch_impl
  -> vtable_impl
  -> impl_or_trait_obligations
  -> ImplSourceUserDefinedData { nested: impl_obligations }
```

`impl_or_trait_obligations` 会读取 impl 或 trait definition 上的 predicates：

```rust,ignore
let predicates = tcx.predicates_of(def_id);
let predicates = predicates.instantiate_own(tcx, args);

for (index, (predicate, span)) in predicates.into_iter().enumerate() {
    let clause = normalize_with_depth_to(..., predicate, &mut obligations);
    obligations.push(Obligation {
        cause,
        recursion_depth,
        param_env,
        predicate: clause.as_predicate(),
    });
}
```

考虑：

```rust,ignore
impl<T: Clone> Clone for Vec<T> { ... }
```

确认 `Vec<U>: Clone` 时：

```text
1. rematch_impl 得到 impl args:
   T -> U

2. impl_or_trait_obligations 读取 impl predicates:
   T: Clone

3. instantiate_own(args):
   U: Clone

4. 构造 nested Obligation:
   Obligation {
     cause: derived from proving Vec<U>: Clone via this impl,
     param_env: 父 obligation 的 param_env,
     predicate: U: Clone,
     recursion_depth: parent + 1,
   }

5. vtable_impl 返回：
   ImplSource::UserDefined {
     impl_def_id,
     args,
     nested: [U: Clone obligation],
   }

6. process_trait_obligation 把 nested 交给 ObligationForest：
   Changed([U: Clone])
```

这也解释了 `poly_select(Vec<T>: Clone)` 为什么会“产生” nested obligations：严格说它返回的是一个带 nested obligations 的 `ImplSource`；fulfillment 只是把这些 nested obligations 作为 children 挂到 obligation forest 中。

### 7. ambiguity 什么时候不是错误，什么时候是错误

位置：`compiler/rustc_trait_selection/src/traits/engine.rs`，`try_evaluate_obligations` 与 `evaluate_obligations_error_on_ambiguity`。

两者都处理 pending obligations，但语义不同：

```text
try_evaluate_obligations
  Ok: 移除
  Err: 返回错误
  Ambiguous: 保留，等待更多 inference 信息

evaluate_obligations_error_on_ambiguity
  Ok: 移除
  Err: 返回错误
  Ambiguous: 也当作错误返回
```

这对应 typeck 的两个阶段：

```text
推理还在进行中
  ambiguity 是“暂时不知道”，可以继续等待。

所有约束都应该收敛时
  ambiguity 是“无法决定”，必须报告错误。
```

### 8. cause 是错误诊断的骨架

`ObligationCause` 不参与逻辑证明，但它决定错误怎么讲给用户。

比如父 obligation：

```text
Vec<T>: Clone
cause: method call x.clone()
```

派生出的子 obligation：

```text
T: Clone
cause: derived from impl Clone for Vec<T>
```

如果最终 `T: Clone` 失败，诊断不能只说“`T: Clone` 不成立”，还要能解释：

```text
因为你调用了 x.clone()
需要 Vec<T>: Clone
标准库 impl 又要求 T: Clone
但当前环境无法证明 T: Clone
```

这就是 `ObligationCause`、derived cause 和 `ObligationForest` backtrace 一起工作的地方。

## 正文

### Obligation 与 Goal 的边界

`Goal` 很适合 solver：

```text
Goal {
  param_env,
  predicate,
}
```

但 typeck 还要回答：

```text
这个 obligation 从哪里来？
失败时指向哪个 span？
它是不是某个父 obligation 的派生结果？
递归到第几层了？
现在是否因为 inference var 卡住？
```

因此 fulfillment 处理的是 `Obligation` / `PendingPredicateObligation`，而不是裸 `Goal`。

### 一个 obligation 的生命周期

以 `Vec<T>: Clone` 为例：

```text
1. typeck 遇到 x.clone()

2. 创建 obligation：
   Vec<T>: Clone under ParamEnv [T: Clone]

3. register_obligation

4. FulfillmentContext 放入 ObligationForest root

5. process_obligation
   poly_select(Vec<T>: Clone)

6. selection 找到 impl<T: Clone> Clone for Vec<T>

7. 返回 Changed([T: Clone])

8. ObligationForest 把 T: Clone 注册为 child

9. 下一轮处理 T: Clone
   ParamEnv candidate 成功

10. child 完成后 parent tree 完成
```

如果第 9 步无法证明 `T: Clone`：

```text
child -> Error
parent tree -> Error with backtrace
```

如果第 5 步时 `Vec<?T>: Clone` 中 `?T` 还没确定：

```text
poly_select -> Ok(None)
process_obligation -> Unchanged
stalled_on = [?T]
```

等后续 `?T = u32` 后，`needs_process_obligation` 会看到变量变化，再尝试处理。

### 与 new solver 的关系

本章主要讲的是 old solver fulfillment，因为 `ObligationForest` 仍然是理解 obligation 生命周期的最佳切面。

new solver 更直接地围绕 `Goal` / `EvalCtxt` / candidates 工作，但 `Obligation` 仍是 typeck 层常见的工作项抽象：

```text
typeck / infer 层
  用 Obligation 保存 cause、param_env、predicate、depth

solver 层
  用 Goal 评估 param_env + predicate 的逻辑结果
```

因此可以这样连接：

```text
Obligation.as_goal()
  丢掉诊断和调度信息
  得到 solver 可证明的 Goal
```

## 常见误区

1. 把 `Predicate` 当作 obligation。

   `Predicate` 只是命题；`Obligation` 才是需要被处理、带上下文的工作项。

2. 以为 select 到 impl 就完成。

   select 到 impl 通常只表示浅层成功；impl where-clauses 会变成 nested obligations。

3. 以为 ambiguity 一定是错误。

   fulfillment 中途 ambiguity 可以保留 pending；最终阶段还 ambiguous 才报错。

4. 忽略 `ParamEnv`。

   obligation cache key 和第 07 章一样必须包含 `ParamEnv`，因为环境会改变证明结果。

5. 把 `cause` 当作证明逻辑的一部分。

   `cause` 不决定 predicate 是否成立，但决定错误链条和用户可理解性。

## 本章小结

`Obligation` 是 typeck 世界中的证明任务；它把第 07 章的 `ParamEnv + Predicate` 包上了原因、深度和诊断上下文。`FulfillmentContext` 把这些任务放进 `ObligationForest`，反复处理 pending obligations。每次处理可能完成、产生子 obligations、暂时卡住或失败。理解这条生命周期后，再看 normalization、projection、canonical query 和 trait solver candidate 时，就能分清“逻辑证明问题”和“编译器调度/诊断问题”。
