---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "13"
document: content
status: completed
updated_at: 2026-09-06
---

# 13. Associated Types 与 GAT

## 学习目标

1. 区分普通 associated type 与具有 own generics 的 GAT。
2. 拆分 projection args 中的 trait 前缀和 GAT own args，追踪到 impl 定义的替换。
3. 区分关联类型输出 bounds、使用条件和投影相等约束。
4. 理解 `where Self: 'a` 的作用与 required bounds 检查。
5. 追踪 projection normalization、item-bound candidate 和抽象 projection 的保留。

## 前置知识

第 02–03 章的 Binder、EarlyBinder、parent generics 与 rebase；第 07 章的 predicates/item bounds；第 09–10 章的 normalization 与 canonical response；第 11–12 章的 goal 和候选求值。

## 核心心智模型

普通关联类型为一份 trait 实现提供一个类型成员；GAT 为这份实现提供一个可带参数的类型成员族。投影本身记录“访问哪个关联成员，传入哪些参数”，具体类型在相应求解环境下通过 normalization 得到，或以抽象 projection 保留。

语言层面的声明、定义与参数规则参见 [Rust Reference：Associated types](https://doc.rust-lang.org/reference/items/associated-items.html#associated-types)。本章 IR 与 solver 细节以当前检出源码为准。

## 源码地图

| 路径 | 符号 | 观察点 |
|---|---|---|
| `compiler/rustc_type_ir/src/ty_kind.rs` | `AliasTy::trait_ref_and_own_args`、`trait_ref` | 从 projection 中拆分父 trait 与 own args |
| `compiler/rustc_hir_analysis/src/collect/item_bounds.rs` | `associated_type_bounds`、`item_bounds` | 给投影本身建立 bounds 并 elaboration |
| `compiler/rustc_hir_analysis/src/check/wfcheck.rs` | `check_gat_where_clauses` | 从 trait 内使用推导 required outlives bounds |
| `compiler/rustc_hir_analysis/src/check/compare_impl_item.rs` | `check_type_bounds`、`compare_type_predicate_entailment` | impl 定义满足 trait 的输出保证与条件契约 |
| `compiler/rustc_next_trait_solver/src/solve/project_goals/mod.rs` | `normalize_associated_term` | Projection 到内部 NormalizesTo |
| `compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs` | `consider_impl_candidate`、`translate_args` | 匹配 impl、证明 GAT 条件、替换 RHS |
| `compiler/rustc_next_trait_solver/src/solve/assembly/mod.rs` | `assemble_alias_bound_candidates`、`assemble_and_merge_candidates` | abstract alias bounds 与 normalization 来源策略 |
| `tests/ui/generic-associated-types/must-prove-where-clauses-on-norm.rs` | UI test | normalization 后仍需检查 GAT 使用条件 |

## 源码精读

### 1. Trait args 与 GAT own args

`compiler/rustc_type_ir/src/ty_kind.rs::AliasTy::trait_ref_and_own_args`，完整函数体：

```rust,ignore
pub fn trait_ref_and_own_args(self, interner: I) -> (ty::TraitRef<I>, I::GenericArgsSlice) {
    let AliasTyKind::Projection { def_id } = self.kind else { panic!("expected a projection") };

    interner.trait_ref_and_own_args_for_alias(def_id.into(), self.args)
}
```

同文件 `trait_ref` 只返回 `.0`，因此会丢掉 own args。例如 `<T as Lend>::Item<'x>` 拆成 `T: Lend` 和 `['x]`；匹配 trait impl 需要前者，实例化 GAT RHS 仍需后者。

### 2. 输出 bounds 作用于 projection 类型

`compiler/rustc_hir_analysis/src/collect/item_bounds.rs::associated_type_bounds`，省略后续 implicit bounds 和父 trait where-clause 收集：

```rust,ignore
let item_ty = Ty::new_projection_from_args(
    tcx,
    ty::IsRigid::No,
    assoc_item_def_id.to_def_id(),
    GenericArgs::identity_for_item(tcx, assoc_item_def_id),
);

let icx = ItemCtxt::new(tcx, assoc_item_def_id);
let mut bounds = Vec::new();
icx.lowerer().lower_bounds(
    item_ty,
    hir_bounds,
    &mut bounds,
    ty::List::empty(),
    filter,
    OverlappingAsssocItemConstraints::Allowed,
);
```

`type Item<'a>: Clone` 的 bound 因而描述 `<Self as Lend>::Item<'a>: Clone`，不是 `Self: Clone`。`item_bounds` query 在 explicit bounds 上进行 elaboration；solver 可以用这些 item bounds 为抽象 alias 提供候选。

### 3. normalization 中显式登记 GAT 的使用条件

`compiler/rustc_next_trait_solver/src/solve/normalizes_to.rs::consider_impl_candidate`，省略前面的 impl 匹配和 impl where-clause 求值：

```rust,ignore
ecx.add_goals(
    GoalSource::AliasWellFormed,
    cx.own_predicates_of(alias_def_id.into())
        .iter_instantiated(cx, goal.predicate.alias.args)
        .map(Unnormalized::skip_norm_wip)
        .map(|pred| goal.with(cx, pred)),
)?;
```

这次使用的是 trait 中关联项的 own predicates 与完整 projection args。它和先前使用 `impl_args` 证明 impl predicates 是两个步骤；nested goals 都继承当前 goal 的 ParamEnv。

即使 RHS 最后变为 `()`，原先 GAT 的 outlives 等使用条件也要验证。该位置的注释与 `must-prove-where-clauses-on-norm.rs` 测试专门解释这一 soundness 要求。

### 4. 从 trait projection args 映射到 impl item args

`solve/normalizes_to.rs::translate_args`，以下是关联项就定义在当前选中 impl 中的分支：

```rust,ignore
// Same impl, no need to fully translate, just a rebase from
// the trait is sufficient.
goal.predicate.alias.args.rebase_onto(cx, impl_trait_ref.def_id.into(), impl_args)
```

trait 前缀被替换成匹配出的 impl args，GAT own args 保留。若定义来自 specialization 中其他 impl，函数还会匹配实际定义者的 trait-ref 并翻译参数，不能机械复用当前 impl args。

随后 `consider_impl_candidate` 的 ProjectionTy 分支取出 RHS 并归一化：

```rust,ignore
let t = cx.type_of(target_item_def_id.into()).instantiate(cx, target_args);
let t = ecx.normalize(GoalSource::Misc, goal.param_env, t)?;
t.into()
```

这里 `target_item_def_id` 指向实际提供定义的关联项；最初 projection 中的关联项标识指向 trait 声明。两者职责不同。

## 正文

### 1. 普通关联类型与 GAT：一个类型成员与一个类型族

```rust,ignore
trait IteratorLike {
    type Item;
}

trait Lend {
    type Item<'a>: Clone + 'a where Self: 'a;
    fn lend<'a>(&'a self) -> Self::Item<'a>;
}
```

`IteratorLike::Item` 没有 own generics。固定 trait-ref 后，成员类型由实现提供。`Lend::Item` 则额外接受 `'a`，同一份 `Self: Lend` 实现可提供一族随借用 lifetime 变化的类型。GAT 也可带类型与 const 参数。

概念上可写成 `Item(Self, 'a)`，但它不是 Rust 中可随意传递的通用高阶类型函数，也不保证对每个 lifetime 都无条件可用；其定义域受 where-clauses 限制。

GAT 声明中的 `'a` 属于 item own generics，通过完整 GenericArgs 实例化，关联定义通过 EarlyBinder 替换。方法签名中的 `for<'a>`/late-bound `'a` 则属于 Binder 量化。当方法的 `'a` 出现在 `Self::Item<'a>` 内时，它是传给 GAT 的一个参数；不能因为都叫 `'a`，就把 GAT 泛型定义和方法 binder 当成同一个 binder。

### 2. 完整可运行例子：输出借用跟随这次调用

```rust
trait Lend {
    type Item<'a>: Clone + 'a where Self: 'a;
    fn lend<'a>(&'a self) -> Self::Item<'a>;
}

struct Cell<T>(T);

impl<T> Lend for Cell<T> {
    type Item<'a> = &'a T where Self: 'a;

    fn lend<'a>(&'a self) -> Self::Item<'a> {
        &self.0
    }
}

fn duplicate<'a, L: Lend>(value: &'a L) -> (L::Item<'a>, L::Item<'a>) {
    let item = value.lend();
    (item.clone(), item)
}

fn main() {
    let cell = Cell(42u32);
    let (a, b) = duplicate(&cell);
    assert_eq!((*a, *b), (42, 42));
}
```

固定 `L = Cell<u32>` 后，`<Cell<u32> as Lend>::Item<'x>` 可归一化为 `&'x u32`。不同调用可以传入不同借用 region，没有要求它们全部是 `'static`。

generic `duplicate` 中，编译器只知道 `L: Lend`，并不知道其 Item 一定是引用。能调用 `.clone()`，是因为声明保证了投影类型实现 Clone。对具体 Cell 的实现，`&T` 本身实现 Clone，因此实现不需要额外要求 T: Clone。

### 3. 三种约束分别回答什么？

对 `type Item<'a>: Clone + 'a where Self: 'a`：

| 写法 | 语义 | 主要用途 |
|---|---|---|
| `where Self: 'a` | 使用这份 GAT 实例的前提 | 形成/使用投影时验证，impl 定义在契约允许的前提下检查 |
| `: Clone + 'a` | 合法 Item 实例向用户保证的性质 | impl 验证输出满足 bounds；使用方可利用 item bounds |
| `<L as Lend>::Item<'x> = U`（概念化） | 这份投影具体等于哪个类型 | projection equality/normalization |

前提与保证不能互换。`Self: 'a` 不会自动指定 Item 的 RHS；只有 `: Clone` 也不能推出 Item = 某个特定 Clone 类型。

当前 `associated_type_bounds` 将输出 bounds 收集为针对 projection 的 clauses；`check_type_bounds` 验证 impl 提供的关联类型满足这些保证，`compare_type_predicate_entailment` 检查 impl 条件与 trait 契约的关系。impl 可以使用 trait 已允许的前提，不能随意要求额外、更强的条件来缩小 trait 声明允许的使用范围。

### 4. 为什么要写 `where Self: 'a`？

`&'a self` 的良构性提供 `Self: 'a`。对这样的 lending 方法，在返回 `Self::Item<'a>` 时，可以使用这一条件。

将条件写到 GAT 声明上，允许 impl 在定义 RHS 时利用它。例如 `Self = Cell<T>` 时，该条件保证这里所需的 T outlives `'a`，从而 `&'a T` 良构。它不是“Self 值必须真的活到程序结束”，也不是给 Item 强制加 `'static`。

当前 rustc 会根据 trait 内方法/其他关联项对 GAT 的使用检查 required bounds。对本例省略该 where-clause，会诊断 missing required bound on Item。不是所有带 lifetime 的 GAT 都一律要求 `Self: 'a`：要求取决于具体使用与可证明的 outlives 关系。

源码 `wfcheck.rs::check_gat_where_clauses` 会遍历 GAT，分析其他关联项的使用，合并其 required bounds，并循环处理关联项之间的传播。方法签名分析还考虑输入类型良构带来的 implied bounds；不能把算法概括为“看到一个引用就无条件加 Self: 'a”。

### 5. 手算完整参数表及 rebase

```rust
trait Family<P> {
    type Out<'a, Q, const N: usize> where Self: 'a, P: 'a;
}
struct Wrap<T>(T);
impl<T, P> Family<P> for Wrap<T> {
    type Out<'a, Q, const N: usize> = (&'a T, &'a P, [Q; N])
    where Self: 'a, P: 'a;
}
```

投影 `<Wrap<u16> as Family<u32>>::Out<'x, bool, 4>`：

```text
trait 声明端泛型槽位：Self#0, P#1, 'a#2, Q#3, N#4
完整 projection args：[Wrap<u16>, u32, 'x, bool, 4]
trait-ref：Wrap<u16>: Family<u32>
GAT own args：['x, bool, 4]
```

匹配 impl 得到 T = u16、P = u32；其 parent args 是 `[u16, u32]`，然后 rebase 得到 impl 关联项的完整 args `[u16, u32, 'x, bool, 4]`。用这些 args 实例化 RHS：

```text
(&'x u16, &'x u32, [bool; 4])
```

这次所用的显式 GAT 条件是 `Wrap<u16>: 'x` 与 `u32: 'x`。这两个具体类型的 outlives 条件可以成立；一般情形下则必须证明，不能只因为已经知道 RHS 就丢掉。

注意 trait 的 Self 槽不等于 impl 的 T 槽；二者之间的对应来自 header relation。GAT own args 则在替换父前缀时保留下来。

### 6. 三条常见 normalization/证明路径

第一条：Self 和 trait 参数足够具体，使用 impl candidate 匹配、验证条件、找到关联项定义、translate args、实例化 RHS，再归一化 RHS 中的 alias。

第二条：环境已有 projection equality，例如 `T: Iterator<Item = u32>` 提供 trait clause 和 `<T as Iterator>::Item == u32` projection clause。可以借助环境给出 Item 的类型；不需要知道具体 T 最终是哪一种 Iterator。

第三条：只知道 `T: Iterator`，没有足以确定 Item 的等式。此时 Item 可以保留为抽象 rigid projection；“实现 Iterator”并不告诉你 Item = u32。若关联项声明保证某个 bound，仍可用 item/alias-bound candidate 证明该性质，而无需先得到具体 RHS。

这里“按需”指按类型检查需求求解 projection，不是在运行时才求类型。当前 next solver 同时使用 eager normalization：例如注册某些 goal 时先 normalize，候选搜索前先 structurally normalize Self。能够保持抽象的 rigid alias与尚有歧义的 inference/projection goal，也要分别理解。

### 7. Projection equality 不应反向决定 normalization 候选

`normalize_associated_term` 为内部 `NormalizesTo` 创建独立、未约束的输出变量；归一化获得输出后，才与外部 expected term 做 relation。这沿用第 09 章的约束：不能通过希望得到的输出类型，随意决定某个 alias 应该怎样展开。

GAT 不保证单射。例如 `impl Map for () { type Out<X> = u8; }` 对不同 X 都返回 u8，所以即使已经知道 `<() as Map>::Out<?X> == u8`，也不能只凭该等式唯一反推出 X。

当前实现还对部分环境候选场景的未约束 GAT own type/const args 做保守处理；参见 `compute_normalizes_to_goal` 的 ambiguity 回调和 `tests/ui/generic-associated-types/no-incomplete-gat-arg-inference.rs`。不要推广成“任意 GAT 参数未知都必定失败”，也不要假设“给出期待输出就总能推断所有输入”。

### 8. 验证记录与进一步定位

2026-09-05，本机 `rustc 1.99.0-nightly (d453bdd8f 2026-08-14)`，使用 `-Znext-solver --edition=2024`：

- Lend/Cell/duplicate 示例编译运行通过，返回的两个借用值均为 42。
- Family/Wrap 示例以局部引用构造投影类型，编译运行通过，输出数组长度为 4。
- 省略 Lend::Item 的 `where Self: 'a` 的对照程序产生 missing required bound 诊断。

复杂 HRTB 与 GAT、placeholder/universe 的交互留到第 17 章；借用的具体存活点集合与 NLL 留到第 18–19 章。这里先掌握投影参数、声明保证、使用条件与 normalization 的分工。

## 常见误区

- GAT own generics 与方法 Binder 分层建模，即使源码 lifetime 名称相同。
- `trait_ref()` 仅保留 trait 参数，GAT own args 需要另外保留。
- 输出 bounds 是关联项向用户提供的保证，where-clause 是使用的前提。
- 普通抽象投影可以在不知道 RHS 时具有已知 bounds。
- normalization 验证 GAT 条件，即使 RHS 不再含 lifetime。
- canonical query 的具体输入含环境和模式；同一个 projection 名称不意味着所有上下文可得到同一个具体输出。

## 本章小结

GAT 把“关联类型”扩展成有 own args 和使用条件的类型族。理解一次投影，依次确定 trait-ref、own args、合法使用前提、输出 bounds，以及可以使用哪个来源进行 normalization；匹配 impl 后，通过 rebase/translate 把声明端参数映射到实际 RHS。
