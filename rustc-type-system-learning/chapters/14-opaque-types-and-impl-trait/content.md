---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "14"
document: content
status: completed
updated_at: 2026-09-06
---

# 14. Opaque Types 与 impl Trait

## 学习目标

1. 区分 APIT、RPIT、TAIT 与 RPITIT。
2. 解释 opaque identity、generic args 与 hidden type 的关系。
3. 追踪定义域内的 hidden-type 推断、bounds 检查与一致性约束。
4. 区分 capture、outlives bound 和 borrow checking。
5. 结合 TypingMode 解释何时可以定义、保持抽象或展开 opaque。

## 前置知识

第 05 章的 inference storage 和 probe；第 09–10 章的 alias、normalization、canonical response；第 12–13 章的 candidate、item bounds 与 associated types。

## 核心心智模型

返回位置的 `impl Trait` 对外提供一个有声明 bounds 的抽象类型身份；定义方通过函数体等定义性使用确定 hidden type。调用方通常依赖公开契约使用它，而不是随意把它当作那个具体类型。

本章默认 Rust 2024 edition 与当前 next solver。TAIT 单列为 nightly 特性；不把源码中的实验性机制当作稳定语法。

## 源码地图

| 路径 | 符号 | 用途 |
|---|---|---|
| `compiler/rustc_type_ir/src/opaque_ty.rs` | `OpaqueTypeKey`、`iter_captured_args` | 身份、参数与捕获参数筛选 |
| `compiler/rustc_next_trait_solver/src/solve/project_goals/opaque_types.rs` | `normalize_opaque_type` | 按 TypingMode 定义/保持 rigid/展开 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` | `register_hidden_type_in_storage`、`add_item_bounds_for_hidden_type` | 登记候选 hidden type 与 bounds 子目标 |
| `compiler/rustc_infer/src/infer/opaque_types/table.rs` | `OpaqueTypeStorage` | provisional hidden type 和 snapshot 支持 |
| `compiler/rustc_hir_analysis/src/collect/type_of.rs` | `type_of_opaque`、`type_of_opaque_hir_typeck` | 区分 HIR 结果与经 borrowck 的定义结果 |
| `compiler/rustc_borrowck/src/region_infer/opaque_types/mod.rs` | `compute_definition_site_hidden_types` | 借用检查后定义端 hidden type 与 region 对应 |
| `compiler/rustc_ty_utils/src/assoc.rs` | `associated_type_for_impl_trait_in_trait` | RPITIT 的匿名关联项 |
| `compiler/rustc_hir_analysis/src/variance/mod.rs` | `variance_of_opaque` | opaque 捕获与 lifetime 参数 variance |

## 源码精读

### 1. Opaque identity 与 hidden type 分开保存

`compiler/rustc_type_ir/src/opaque_ty.rs::OpaqueTypeKey`，省略属性：

```rust,ignore
pub struct OpaqueTypeKey<I: Interner> {
    pub def_id: I::LocalOpaqueTyId,
    pub args: I::GenericArgs,
}
```

opaque 的声明身份与实例化参数构成局部推理存储的 key，hidden type 是关联的值。不要把 key 本身理解成“hidden type 的 DefId”。

同文件 `iter_captured_args` 依据 variance 过滤：Invariant 参数保留，Bivariant lifetime 参数忽略。因而“args 中出现一个 lifetime”与“这个 lifetime 被 opaque 捕获”不是总能画等号。

`compiler/rustc_infer/src/infer/opaque_types/table.rs::OpaqueTypeStorage` 的核心字段：

```rust,ignore
opaque_types: FxIndexMap<OpaqueTypeKey<'tcx>, ProvisionalHiddenType<'tcx>>,
duplicate_entries: Vec<(OpaqueTypeKey<'tcx>, ProvisionalHiddenType<'tcx>)>,
```

存储还保存 duplicate entries，供后续检查全部使用；它不是只靠一个简化 map 就完成 opaque 验证。推理状态与 snapshot/probe 协作，canonical response 也能通过 external opaque constraints 带出相关结果。

### 2. 定义域内登记 hidden type，并与已有值关联

`solve/project_goals/opaque_types.rs::normalize_opaque_type`，省略参数结构性归一化及其他模式分支：

```rust,ignore
let opaque_type_key = ty::OpaqueTypeKey { def_id, args: normalized_args };
if let Some(prev) = self.register_hidden_type_in_storage(opaque_type_key, expected) {
    self.eq(goal.param_env, expected, prev)?;
} else {
    // ……根据 HIR typeck / borrowck 前模式初始化……
}
```

这里的 `expected` 是当前 opaque projection relation 的另一侧，可能由函数返回表达式的类型提供。定义性使用允许它参与确定 hidden type；同一 key 已有值时，需要与前值相等。

这与第 13 章内部 `NormalizesTo` 隔离 expected term 的用途不同：关联类型 projection 通常是在读取并展开既有定义；opaque 在允许定义的上下文中是在建立自己的 hidden-type 定义。不能把“所有 alias 的 expected 都只能等展开完再用”当成统一规则。

### 3. hidden type 必须满足声明 bounds

`solve/eval_ctxt/mod.rs::add_item_bounds_for_hidden_type`，函数核心：

```rust,ignore
let mut goals = Vec::new();
self.delegate.add_item_bounds_for_hidden_type(
    opaque_def_id,
    opaque_args,
    param_env,
    hidden_ty,
    &mut goals,
);
self.add_goals(GoalSource::AliasWellFormed, goals)?;
```

例如 `impl Iterator<Item = u32>` 的 hidden type 是 Range<u32>，就需要验证相应 Iterator 要求和 Item 等式。登记一个 hidden type 与证明它满足公开契约是关联但不同的步骤。

### 4. TypingMode 决定定义权限与展开策略

`normalize_opaque_type` 的 Typeck 分支先查本地 opaque 是否在 `defining_opaque_types` 集合中；不在时，将 alias 以 `IsRigid::Yes` 参与关系检查，而不是用任意 expected 重定义它。

PostAnalysis/Codegen 分支的主体为：

```rust,ignore
let actual = cx.type_of(def_id.into()).instantiate(cx, opaque_ty.args);
let actual = self.normalize(GoalSource::Misc, goal.param_env, actual)?;
self.eq(goal.param_env, expected, actual)?;
self.evaluate_added_goals_and_make_canonical_response(Certainty::Yes)
    .map_err(Into::into)
```

此时使用已获得的 hidden type 定义，并按 opaque args 实例化。类型抽象并不意味着 codegen 不知道真实布局和代码。

## 正文

### 1. 四种语法位置分别在做什么？

| 名称 | 示例 | 核心含义 |
|---|---|---|
| APIT | `fn take(x: impl Clone)` | 匿名泛型参数，调用者选择满足 bounds 的输入类型 |
| RPIT | `fn make() -> impl Clone` | 返回位置 opaque，定义方决定具体 hidden type |
| TAIT | `type Hidden = impl Clone` | 命名 opaque alias，多处共享该身份；当前需要 nightly feature |
| RPITIT | trait 方法返回 `impl Iterator` | trait 方法的匿名关联类型，实现方提供具体返回类型 |

APIT 不是 RPIT 式的 hidden-type 定义。APIT 与命名泛型参数在可显式提供泛型实参等方面也有区别。RPIT/RPITIT 的语言语义参见 [Rust Reference：impl Trait](https://doc.rust-lang.org/reference/types/impl-trait.html)。

### 2. RPIT：同一 opaque 的返回类型要一致

```rust
fn numbers() -> impl Iterator<Item = u32> {
    0..3
}

fn main() {
    assert_eq!(numbers().sum::<u32>(), 3);
}
```

概念化为：函数签名返回 O，O 的 bounds 是 Iterator<Item=u32>，函数体约束 hidden(O)=Range<u32>。调用方能调用 Iterator 的方法，而不能仅根据隐藏实现就把 `numbers()` 赋给显式的 `Range<u32>` 变量。

若一个函数的 if 分支返回 Range<u32>，另一个返回 Vec<u32>::IntoIter，即使都实现 Iterator<Item=u32>，也不能让同一个 RPIT 在运行时切换为两种具体类型。它不是隐式的 enum 或 dyn trait object。

对泛型函数，hidden type 可以是包含捕获泛型参数的类型表达式，例如 `Once<T>`。这与“同一组泛型实参下，按运行时分支任选类型”不同。

两个不同函数各自声明的 RPIT 有不同 opaque 身份，即使隐藏实现恰好都是 Range<u32>，也不因此成为调用方可自由互换的同一个抽象类型。

### 3. 从返回表达式到经验证的 hidden type

主要阶段：

1. 定义处 lowering/类型收集建立 opaque item、args 和声明 bounds。
2. HIR typeck 中，返回值与返回 opaque 的关系产生 hidden-type 约束；定义权限由当前 TypingMode 携带的集合限定。
3. 登记 provisional hidden type，验证已有定义的一致性与 item bounds，求解相关 nested goals。
4. MIR borrowck 继续处理其中的 region，检查捕获与定义性使用，将结果映射到定义端参数，形成后续 query 使用的 hidden type。
5. 后续分析/codegen 可以在相应模式下读取定义并实例化；普通外部类型检查仍遵守 opaque 的抽象边界。

源码 `type_of_opaque_hir_typeck` 与 `type_of_opaque` 分别选择 HirTypeck 和 MirBorrowck 的收集路径；`region_infer/opaque_types/mod.rs` 负责进一步求出 definition-site hidden types。因此不能把第一次 storage 登记的类型视为所有 lifetime 已最终确定的结果。

这也不是纯词法规则“写在同一个 module 就随时能重定义”：local DefId、声明来源、defining-use args、指定定义范围和模式都参与判断。

### 4. Opaque 对外隐藏什么？

对普通 trait 能力，使用方依据声明 bounds 及其逻辑后果。hidden type 恰好实现某个额外普通 trait，并不自动把该 trait 作为公开契约暴露出来。

auto traits（例如 Send/Sync）有专门的泄露/推导规则，不能把 opaque 简化为“只能知道文本显式写出的所有 trait”。本章已验证 `numbers()` 即使返回声明没有 `+ Send`，其结果也可传给 `fn assert_send<T: Send>(_: T)`。这是 auto-trait 行为，不等于公开 hidden type 与 Range<u32> 的类型相等性。

在泛型 RPITIT 使用方，能否要求 Send 还取决于公开契约与上下文，不能因为某一个具体实现可 Send 就推导所有实现都可 Send。

### 5. Capture 是 hidden type 可以依赖哪些参数

本章示例使用 edition 2024。默认 RPIT 会捕获作用域内泛型参数；2024 之前，free function/inherent 方法中未出现在返回 bounds 的 lifetime 默认捕获规则有所不同。可用 `use<...>` 显式限制捕获集合；当前还要求列出作用域内的 type/const 参数等，不能将它理解为任意省略参数都合法的列表。规则见 [Reference：Capturing](https://doc.rust-lang.org/reference/types/impl-trait.html#capturing)。

```rust
fn chars<'a>(s: &'a str) -> impl Iterator<Item = char> + use<'a> {
    s.chars()
}

fn length<'a>(s: &'a str) -> impl Copy + use<> {
    s.len()
}
```

chars 的 hidden type 为包含 `'a` 的 Chars<'a>，因此需要允许捕获 `'a`。length 的 hidden type 为 usize，`use<>` 明确不捕获这个输入 lifetime，返回结果可以在输入借用结束后继续使用。

`use<'a>` 与 `+ 'a` 不是同一回事：前者允许 hidden type 使用该参数，后者要求返回类型满足 outlives `'a`。捕获某个参数也不等于 hidden type 必须实际包含它；默认过宽的捕获可能影响调用方可依赖的关系。

capture 不是运行时“复制/移动变量到闭包”的同义词。closure 的值捕获与 opaque 的泛型参数捕获处在不同层次，尽管返回闭包可能同时涉及两者。

### 6. TAIT：多处使用同一个命名 opaque

以下单独使用 nightly 特性：

```rust
#![feature(type_alias_impl_trait)]

type Numbers = impl Iterator<Item = u32>;

#[define_opaque(Numbers)]
fn make() -> Numbers {
    0..3
}

fn main() {
    assert_eq!(make().sum::<u32>(), 3);
}
```

Numbers 命名的是一个 opaque 身份，定义性使用将它约束为 Range<u32>。多处声明返回 Numbers，是引用同一个身份，不像多个独立 RPIT 各自创建抽象返回类型。

当前 TAIT 要求用 `#[define_opaque(...)]` 显式标明相关定义项，参见 [Unstable Book：type_alias_impl_trait](https://doc.rust-lang.org/unstable-book/language-features/type-alias-impl-trait.html)。多个定义性使用仍要给出一致的 hidden type。`fn take(x: Numbers)` 接受的是该特定 opaque 类型，不等于 `fn take(x: impl Iterator<Item=u32>)` 可以接受任意满足 bound 的输入。

### 7. RPITIT：与上一章匿名关联类型相接

```rust
trait Source {
    fn values(&self) -> impl Iterator<Item = u32> + '_;
}

struct Data(Vec<u32>);

impl Source for Data {
    fn values(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().copied()
    }
}
```

概念上相当于 trait 有一个按 Self 和方法相关参数实例化的匿名关联类型，bounds 为 Iterator<Item=u32>，方法返回该投影。本例借用了 self，因而可以类比上一章的 lifetime GAT；这是概念类比，实际 synthetic generics/capture 由编译器构造，不是可逐字照抄的手写 desugaring。

`compiler/rustc_ty_utils/src/assoc.rs::associated_type_for_impl_trait_in_trait` 使用 `create_def(..., DefKind::AssocTy, ...)` 创建匿名关联项，并记录 RPITIT 数据。不同 Self 的不同 trait impl 可以提供不同的返回类型，和单个具体实现的各返回分支必须相容并不冲突。

`async fn` 返回隐藏 Future，也会涉及相同主题；trait 中 async fn 与 RPITIT 的关系可沿这些 synthetic associated items 继续阅读，不能将 Future 等同于 Box<dyn Future>。

### 8. TypingMode 的主要分支

| 模式 | 当前源码中的 opaque projection 处理 |
|---|---|
| Typeck | 在 defining 集合内可约束 hidden type；其他 opaque 保持 rigid |
| PostTypeckUntilBorrowck | 允许的定义集合内利用 HIR 结果初始化并继续关联 region/类型 |
| PostBorrowck | 对允许展开的 defined 集合读取已检查定义；其他保持 rigid |
| PostAnalysis / Codegen | 读取 type_of 并实例化实际 hidden type |
| Coherence | 检查假设 hidden type 的 bounds，并保留专门的 ambiguity；不凭此永久定义 opaque |

具体还存在 erased-mode 查询与 rerun 机制，见同一 `normalize_opaque_type` 函数。上述表是主要分支，不把“所有非 Typeck 情形一律 reveal”当成规则。

## 常见误区

- Opaque identity 与 hidden type 分开；bounds 相同也不意味着 opaque 身份相同。
- APIT 由调用者选择类型，RPIT 由定义方提供类型。
- 同一具体 RPIT 的返回分支要求一个一致的类型，而不是只需实现同一个 trait。
- capture、outlives 与闭包的值捕获是不同概念。
- generic hidden type 可以随泛型参数实例化，运行时分支不能任意改变类型。
- trait projection 读取既有定义的 normalization 与 opaque 定义性约束需要区分。

## 本章小结

以 opaque item 身份和参数为索引，定义方确定满足 bounds 的 hidden type；borrowck 继续验证 region 与捕获，使用方按公开契约和相应模式使用抽象类型，codegen 则可获取具体布局与实现。RPITIT 将这套机制接入 trait 的匿名关联类型，TAIT 提供可多处引用的命名身份。

验证：2026-09-06 使用本机 `rustc 1.99.0-nightly (d453bdd8f 2026-08-14)` 和 `-Znext-solver --edition=2024`。RPIT 求和、auto-trait Send、chars 精确捕获、length 结果越过输入借用作用域、RPITIT 调用以及 nightly TAIT 均编译运行通过；Range 与 Vec::IntoIter 两种分支返回类型的程序产生 E0308。
