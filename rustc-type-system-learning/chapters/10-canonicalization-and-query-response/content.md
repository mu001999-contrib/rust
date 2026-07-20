---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "10"
document: content
status: in_progress
updated_at: 2026-08-12
---

# 10. Canonicalization 与查询响应

## 学习目标

完成本章后，应当能够：

1. 解释为什么包含 `?T0`、`?R0` 的本地推理状态不能直接作为全局 query key。
2. 手算 inference variable、free region 与 placeholder 到 canonical variable 的替换。
3. 读懂 `Canonical { value, max_universe, var_kinds }` 和 `OriginalQueryValues` 的对应关系。
4. 区分 existential canonical vars 与 placeholder canonical vars。
5. 追踪 canonical input 在查询内部如何实例化为 fresh inference variables / placeholders。
6. 读懂 response 中 `var_values`、`certainty` 和 external/region constraints 的职责。
7. 追踪 response 如何实例化回调用方，并把求解结果提交到原 `InferCtxt`。
8. 区分 old canonical query response 与 new solver `Canonical<Response>` 的字段组织。

## 前置知识

- 第 02 章：binder、bound variable 与遍历顺序。
- 第 04 章：universe、placeholder 与 nameability。
- 第 05 章：inference variable、unification table 与 `FreshTy`。
- 第 08 章：goal、obligation、certainty 与 fulfillment。
- 第 09 章：normalization 输出变量及 `TypingMode`。

## 核心心智模型

不同 `InferCtxt` 中的变量编号只有本地意义：

```text
调用方 A 的 ?T0
调用方 B 的 ?T0
```

两者名字相同，不代表同一个变量；反过来，两个结构相同的问题也可能使用不同编号：

```text
A: Vec<?T0>: Clone
B: Vec<?T37>: Clone
```

若直接按原始 IR 缓存，它们会成为不同 query key，而且缓存结果会引用某个已经离开的 `InferCtxt`。canonicalization 把本地身份替换为按首次出现顺序编号的 canonical vars：

```text
Vec<?T0>: Clone   ─┐
                   ├─>  Vec<^0>: Clone
Vec<?T37>: Clone  ─┘
```

同时调用方保留反向映射：

```text
OriginalQueryValues:
  ^0 -> ?T0
```

一次完整查询是：

```text
调用方 InferCtxt
  goal: Vec<?T0>: Trait<'?R0>
  │
  ├─ canonicalize_query
  │    value: Vec<^0>: Trait<'^1>
  │    var_kinds: [Ty(U0), Region(U0)]
  │    original_values: [ ?T0, '?R0 ]
  │
  ▼
可缓存 canonical query
  │
  ├─ 在查询 InferCtxt 中 instantiate_canonical
  │    ^0 -> ?Q0
  │    '^1 -> '?Q0
  │
  ├─ solve
  │    例如 ?Q0 = u32
  │
  ├─ canonicalize_response
  │    输入变量结果: [u32, ...]
  │    + certainty
  │    + region/external constraints
  │
  ▼
调用方应用 response
  │
  ├─ response canonical vars 实例化回本地命名空间
  ├─ 原始值与 response.var_values 做 eq
  │    ?T0 = u32
  └─ 注册 region / opaque / normalization constraints
```

canonicalization 的要点不是把变量“求出”，而是把问题改写成：

```text
与创建它的 InferCtxt 无关
+ 可稳定比较和缓存
+ 仍能把答案映射回原变量
+ 保留变量种类和 universe 能力
```

## 源码地图

| 路径 | 关键符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_type_ir/src/canonical.rs` | `CanonicalQueryInput`、`Canonical`、`CanonicalVarKind`、`CanonicalVarValues` | canonical 数据模型 |
| `compiler/rustc_middle/src/infer/canonical.rs` | `OriginalQueryValues`、`QueryResponse`、`QueryRegionConstraints` | old query 的调用方映射和响应结构 |
| `compiler/rustc_infer/src/infer/canonical/canonicalizer.rs` | `canonicalize_query`、`canonicalize_response`、`Canonicalizer` | old/infcx canonicalization 的遍历与去重 |
| `compiler/rustc_infer/src/infer/canonical/mod.rs` | `instantiate_canonical`、`instantiate_canonical_var` | canonical input 在新推理上下文中实例化 |
| `compiler/rustc_infer/src/infer/canonical/query_response.rs` | `make_canonicalized_query_response`、`instantiate_query_response_and_region_obligations` | old query response 的打包与回放 |
| `compiler/rustc_type_ir/src/solve/mod.rs` | `QueryInput`、`Response`、`ExternalConstraintsData`、`Certainty` | new solver canonical input/response |
| `compiler/rustc_next_trait_solver/src/canonical/mod.rs` | `canonicalize_goal`、`instantiate_and_apply_query_response` | new solver 查询边界的完整流程 |
| `compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs` | `make_canonical_response`、`compute_external_query_constraints` | solver 如何生成 response |
| `compiler/rustc_middle/src/ty/mod.rs` | `TypingEnv`、`TypingModeEqWrapper` | `TypingMode` 如何进入 query key |

## 源码精读

### 1. `Canonical<V>` 保存值、变量声明与 universe 上界

位置：`compiler/rustc_type_ir/src/canonical.rs`，`Canonical`。

```rust,ignore
pub struct Canonical<I: Interner, V> {
    pub value: V,
    pub max_universe: UniverseIndex,
    pub var_kinds: I::CanonicalVarKinds,
}
```

三部分分别回答：

```text
value
  替换后的查询或响应；其中使用 ^0、^1 等 canonical bound vars

var_kinds[i]
  ^i 是 type / int / float / region / const，
  是 existential variable 还是 placeholder，并属于哪个 universe

max_universe
  canonical value 引用的最大 universe；实例化时需在目标 InferCtxt
  中建立相应 universe 层级
```

canonical vars 虽然在 Type IR 中借用 `Bound` / `ReBound` 表示，但通过：

```text
BoundVarIndexKind::Canonical
```

与普通 binder 的 de Bruijn-bound variables 区分。可概念化写作 `^0`、`'^1`，它们不表示“源代码中出现了一个 `for<>` binder”，而表示 canonical value 的参数槽。

`CanonicalQueryInput` 还携带：

```rust,ignore
pub struct CanonicalQueryInput<I, V> {
    pub canonical: Canonical<I, V>,
    pub typing_mode: TypingModeEqWrapper<I>,
}
```

第 09 章已经看到，同一 goal 在不同 `TypingMode` 中可能对 opaque、specialization 得出不同结果。因此 `TypingMode` 也是 query key 的一部分。

### 2. `CanonicalVarKind` 保留变量的逻辑能力

位置：`compiler/rustc_type_ir/src/canonical.rs`，`CanonicalVarKind`。

主要 variants 是：

```rust,ignore
Ty { ui, sub_root }
Int
Float
PlaceholderTy(...)
Region(ui)
PlaceholderRegion(...)
Const(ui)
PlaceholderConst(...)
```

这里不能只记录“第几个变量”，因为以下变量能力不同：

```text
Ty(U0)
  existential type variable，可在允许范围内被求解为具体类型

Region(U0)
  existential region variable

PlaceholderTy(P@U1)
  universal name，不能被 solver 任意赋值

Int / Float
  只能被相应数值类型实例化

Const(U0)
  const inference variable
```

`is_existential()` 明确把 `Ty/Int/Float/Region/Const` 归为 existential，把三类 placeholder 归为 universal。

`Ty { ui, sub_root }` 中的 `sub_root` 保存第 05 章 `sub_unification_table` 的相关性：若两个输入 type vars 已处于同一个 subtype-relation component，canonicalization 不能把这层关系丢掉。实例化时 `instantiate_canonical_var` 会调用 `sub_unify_ty_vids_raw` 恢复它。

#### canonical 槽编号与本地 inference ID 是两套编号

假设 canonical input 的变量表是：

```text
var_kinds[0] = Ty(U0)
var_kinds[1] = Region(U0)
```

那么 canonical value 中分别写作：

```text
^0     // canonical slot 0，是 type
'^1    // canonical slot 1，是 region
```

这里 canonical slot 对 type、region 和 const 使用同一张 `var_kinds` 表，因此下标依次是 0、1。

实例化时则根据 `CanonicalVarKind` 分派到不同的本地变量分配器：

```text
Ty(U0)      -> next_ty_vid_in_universe      -> ?Q0@U0
Region(U0)  -> next_region_var_in_universe  -> '?Q0@U0
```

`TyVid` 与 `RegionVid` 是不同的 newtype，并使用独立的编号空间；两套空间的第一个变量都编号为 0。因此：

```text
?Q0   = TyVid(0)，嵌在 TyKind::Infer(TyVar(...)) 中
'?Q0  = RegionVid(0)，嵌在 RegionKind::ReVar(...) 中
```

它们不会冲突，类似于 `type_table[0]` 和 `region_table[0]`。

`Const` 也有独立的 `ConstVid` 和 const unification table。例如：

```text
var_kinds:
  0: Ty(U0)
  1: Region(U0)
  2: Const(U0)

instantiation:
  ^0  -> ?QTy0@U0       // TyVid(0)
  '^1 -> '?QRegion0@U0  // RegionVid(0)
  ^2  -> ?QConst0@U0    // InferConst::Var(ConstVid(0))
```

三者的本地编号都可以是 0，因为分别索引 type、region、const 三套存储。`CanonicalVarKind::PlaceholderConst` 则实例化为 `ConstKind::Placeholder`，不会分配 `ConstVid`。

#### `RePlaceholder` 的身份：`universe + bound`

placeholder 也需要区分身份，但不使用 `RegionVid`。当前 Type IR 中：

```text
PlaceholderRegion = Placeholder<BoundRegion>

PlaceholderRegion {
  universe: UniverseIndex,
  bound: BoundRegion {
    var: BoundVar,
    kind: BoundRegionKind,
  },
}
```

因此一个 region placeholder 的完整存储身份包含 universe 和 bound name。同一 universe 内的两个 bound regions 通过不同 `BoundVar` 区分；不同 universe 中即使 `BoundVar` 相同，也仍是不同 placeholder。

例如实例化：

```rust,ignore
for<'a, 'b> fn(&'a u32, &'b u32)
```

进入同一个新 universe `U1` 后，可概念化为：

```text
'a -> RePlaceholder(U1, BoundRegion(var=0, kind=...))
'b -> RePlaceholder(U1, BoundRegion(var=1, kind=...))
```

另一次进入 binder 若创建 `U2`，则：

```text
RePlaceholder(U1, var=0) != RePlaceholder(U2, var=0)
```

canonical input 中的：

```text
var_kinds[1] = PlaceholderRegion(P@U1)
canonical occurrence = '^1
```

在 query-local `InferCtxt` 中实例化时执行的是：

```text
map U1 -> query-local U1'
保留 bound
生成 RePlaceholder(U1', bound=P.bound)
```

它不会从 region inference table 分配 `RegionVid`，因为 placeholder 是 rigid universal name，不是带待求解值的 existential variable。它的身份直接编码在 region IR 中，而 `ReVar(RegionVid)` 的身份来自 region inference table。

#### `max_universe` 是最高层级，不是唯一 universe

`max_universe = U3` 不表示 canonical value 只能包含一个 `U3`，而表示这份 canonical 数据需要的 universe 层级上界是 `U3`。rustc 的 `UniverseIndex` 按顺序创建，每个新 universe 都扩展此前所有 universe：

```text
U0 < U1 < U2 < U3
```

因此只要知道最高编号，就知道实例化时需要建立 `U0..=U3` 的映射。各变量具体属于哪一层仍由 `CanonicalVarKind` 保存：

```text
Canonical {
  max_universe: U3,
  var_kinds: [
    Ty(U0),
    PlaceholderRegion(P0@U1),
    Ty(U2),
    PlaceholderConst(C0@U3),
  ],
}
```

实例化到另一个 `InferCtxt` 时，可建立：

```text
canonical U0 -> local Ux
canonical U1 -> local Ux+1
canonical U2 -> local Ux+2
canonical U3 -> local Ux+3
```

然后每个 `var_kind` 使用自己的 universe 映射。这样保持：

```text
U0.can_name(U0) = true
U0.can_name(U1) = false
U2.can_name(U1) = true
```

通用/old `instantiate_canonical` 的实现正是从当前 universe 开始，再循环创建 `1..=max_universe` 的 fresh universes。

当前 next solver 还对 query 边界做了相对化：

```text
canonical input:
  调用方已有的 placeholders/inference vars 被作为 root-level 输入处理，
  因而 input 的 max_universe 通常是 U0

canonical response:
  调用方进入查询前已有的 universe 被折回 U0，
  max_universe 只保留查询内部新创建的 universe 深度
```

例如 response 的 `max_universe = U2` 表示应用响应前，需要在调用方当前 universe 之上再创建两层，然后实例化 response 中属于 U1、U2 的 placeholders 或 existential variables。它仍然表示层级范围，不是一个可供所有变量共享的单独 universe。

#### nameability 与词法作用域的类比

nameability 可以类比词法作用域中的名字可见性：外层作用域先存在，进入内层作用域后获得新名字；外层定义的值不能把只在内层有效的名字捕获并带到外面。

逻辑量词形式更精确：

```text
exists ?T@U0.
  forall P@U1.
    ...
```

`?T@U0` 在进入 `forall` 之前就已存在，因此它的解不能包含后来引入的 `P@U1`：

```text
?T@U0 = P@U1       // 不满足 nameability
?T@U0 = &P@U1      // 同样不满足
```

进入 U1 后新建的 existential variable 则可以命名 U1 以及更外层的名字：

```text
?X@U1 = P@U1       // nameability 允许
?X@U1 = SomeType@U0
```

因此“U0 不能依赖 U1”可以作为简写，但这里的“依赖”专指：U0 inference variable 的求解值不能包含 U1 才引入的 placeholder。整个 U1 求解上下文仍然可以同时建立涉及 U0 变量和 U1 placeholder 的约束；这些约束最终必须通过 leak/nameability 检查。

这个顺序不是 region outlives 关系：

```text
U0 < U1
```

不等于某个 lifetime `'u0: 'u1` 或 `'u1: 'u0`。universe 描述逻辑名字的作用域和量词依赖顺序；outlives 描述 region 集合/存活范围之间的约束。

如果 canonical input 中再出现一个 type 和一个 region，映射可能是：

```text
^0  -> ?Q0
'^1 -> '?Q0
^2  -> ?Q1
'^3 -> '?Q1
```

所以 canonical 下标表示“在统一 canonical 参数表中的位置”，本地 inference 下标表示“在该变量种类自己的推理表中的位置”；不能从 canonical 下标直接推导本地 `Vid`。

### 3. canonicalization 按首次出现顺序去重

位置：`compiler/rustc_infer/src/infer/canonical/canonicalizer.rs`，`Canonicalizer::canonical_var`。

设输入为：

```text
(?T7, Vec<?T3>, ?T7, &'?R4 ?T3)
```

假设遍历顺序依次遇到 `?T7`、`?T3`、`?T7`、`'?R4`，结果为：

```text
canonical value:
  (^0, Vec<^1>, ^0, &'^2 ^1)

var_kinds:
  [Ty(U0), Ty(U0), Region(U0)]

OriginalQueryValues.var_values:
  [?T7, ?T3, '?R4]
```

同一个原值再次出现时复用同一个 canonical index。`Canonicalizer::canonical_var` 会在线性小数组或 lookup table 中查找原 `GenericArg`，保证：

```text
相同输入变量 -> 相同 canonical var
不同输入变量 -> 不同 canonical var
canonical index -> 按首次访问顺序分配
```

canonicalizer 还先查询 type/const unification root，并 opportunistically resolve 已知变量：

```text
?T0 -> u32
```

则 canonical value 中直接出现 `u32`，不会再为 `?T0` 分配 canonical var。

### 4. query canonicalization 也替换 free regions

位置：`InferCtxt::canonicalize_query`。

query canonicalization 不只替换 inference vars，还把查询值中的 free regions 替成 canonical vars。例如：

```text
T: Trait<'static>
```

在 value 部分可被改写为：

```text
T: Trait<'^0>
OriginalQueryValues: ['static]
```

这样 query key 更一般，也不会把调用方 region identity 直接带入缓存。

当前 old canonicalizer 对 `ParamEnv` 有一个兼容性细节：缓存 canonical `ParamEnv` 时保留其中的 `'static`，而 value 使用 `CanonicalizeAllFreeRegions`。因此手算具体源码输出时，需要区分 region 出现在 `ParamEnv` 还是 query value；核心模型仍是“free region 的原值保存在 `OriginalQueryValues`，响应回来后映射复原”。

canonical response 的策略不同：它只 canonicalize 尚未解决的 inference vars / placeholders，通常保留响应中新出现的合法 free regions，如 impl 明确要求的 `'static`。这是因为 response 中的 `'static` 是求解结果，不应再被误认成某个输入变量。

### 5. input canonical vars 在查询内部重新实例化

位置：`compiler/rustc_infer/src/infer/canonical/mod.rs`，`instantiate_canonical`、`instantiate_canonical_var`。

查询实现不能直接拿 `^0` 做普通 inference，因此会在自己的 `InferCtxt` 中建立实例化：

```text
canonical input:
  value: Vec<^0>: Trait<'^1>
  var_kinds: [Ty(U0), Region(U0)]

query-local instantiation S:
  ^0  -> ?Q0@U0
  '^1 -> '?Q0@U0

instantiated goal:
  Vec<?Q0>: Trait<'?Q0>
```

规则是：

```text
existential CanonicalVarKind
  -> query-local fresh inference variable

Placeholder* CanonicalVarKind
  -> query-local placeholder，保留 universal 语义
```

`max_universe` 用于在查询的 `InferCtxt` 中重建 universe 层级。root universe 内容实例化到当前 universe，更高 canonical universes 对应创建 fresh universes。

这一步使同一个 canonical query 可以在独立推理上下文中安全运行。

### 6. response.var_values 是“每个输入槽最后变成什么”

查询内部实例化：

```text
^0 -> ?Q0
```

若求解得到：

```text
?Q0 = u32
```

响应在 canonicalize 前会包含：

```text
var_values: [u32]
```

若没有推进：

```text
?Q0 仍未约束
```

canonical response 会把这个 query-local 变量再次 canonicalize：

```text
response.var_kinds: [Ty(U0)]
response.value.var_values: [^0]
```

因此必须区分两组 canonical vars：

```text
input canonical vars
  描述传入问题的参数槽

response canonical vars
  描述响应中仍未解决或新产生的变量
```

它们都从 0 编号，但属于不同 `Canonical<...>`，不能按数字直接认作同一个变量。输入与响应的联系由 response 内的 `var_values` 建立。

### 7. old query response 的字段

位置：`compiler/rustc_middle/src/infer/canonical.rs`，`QueryResponse<R>`。

```rust,ignore
pub struct QueryResponse<'tcx, R> {
    pub var_values: CanonicalVarValues<'tcx>,
    pub region_constraints: QueryRegionConstraints<'tcx>,
    pub certainty: Certainty,
    pub opaque_types: Vec<(OpaqueTypeKey<'tcx>, Ty<'tcx>)>,
    pub value: R,
}
```

含义是：

```text
var_values
  canonical input 中每个变量经过求解后的值

region_constraints
  查询内部产生但必须由调用方环境满足/登记的 outlives 或 region equality

certainty
  Proven 或 Ambiguous

opaque_types
  查询中新定义/约束的 opaque hidden types

value
  该具体 query API 的业务返回值
```

随后整个 `QueryResponse<R>` 再包进 `Canonical<...>`，使响应本身不引用 query-local inference vars。

`make_canonicalized_query_response` 会先推进 fulfillment：

```text
true error       -> Err(NoSolution)
仍有 ambiguity   -> response certainty = Ambiguous
全部证明完成     -> response certainty = Proven
```

并抽取 region obligations/constraints，最后调用 `canonicalize_response`。

### 8. new solver response 把附加结果集中为 external constraints

位置：`compiler/rustc_type_ir/src/solve/mod.rs`，`Response`、`ExternalConstraintsData`。

new solver 使用：

```rust,ignore
pub struct Response<I> {
    pub certainty: Certainty,
    pub var_values: CanonicalVarValues<I>,
    pub external_constraints: I::ExternalConstraints,
}
```

其中：

```text
ExternalConstraintsData {
  region_constraints,
  opaque_types,
  normalization_nested_goals,
}
```

和 old response 的共同骨架仍是：

```text
canonical var assignments
+ certainty
+ 必须带回调用方的外部副作用/约束
```

new solver 的 `Certainty` 是：

```text
Yes
Maybe(MaybeInfo)
```

`MaybeInfo` 还能区分普通 ambiguity、overflow 等原因。它比 old `Proven/Ambiguous` 承载更多 solver 状态。

### 9. response 如何回到调用方

位置：

- old：`InferCtxt::instantiate_query_response_and_region_obligations`
- new：`instantiate_and_apply_query_response`

调用方原状态：

```text
OriginalQueryValues:
  input ^0 -> caller ?T7
```

响应：

```text
response.var_values for input ^0:
  u32
```

应用过程：

```text
1. 为 response 自己的 canonical vars 选择调用方实例化值

2. 实例化 response

3. 对每个输入槽：
   eq(original_value, response_value)

   eq(?T7, u32)
   -> caller unification table: ?T7 = u32

4. 注册 region constraints

5. 注册新 opaque types

6. old query 返回业务 value + 可能由 eq 产生的 obligations；
   new solver 还返回 normalization nested goals 与 certainty
```

这说明 query 不能直接修改调用方 `InferCtxt`：它先返回一份声明式 response，调用方再显式应用。这样缓存中的 response 才能被多个调用方安全复用。

### 10. region constraint 为什么单独返回

假设查询内部证明某关系时产生：

```text
'^0: '^1
```

它不能只靠 `var_values` 表示，因为两个 region 可能都保持原值，但它们之间新增了 outlives 条件。

响应因此携带：

```text
QueryRegionConstraint {
  constraint: Outlives('^0, '^1),
  category,
  visible_for_leak_check,
}
```

映射回调用方：

```text
OriginalQueryValues:
  '^0 -> 'a
  '^1 -> 'b

instantiated constraint:
  'a: 'b
```

普通调用方会把它登记进 `InferCtxt` region constraints；NLL 路径可使用 `instantiate_nll_query_response_and_region_obligations`，直接把 query constraints 交给 MIR-based region inference。

因此 response 的两类主要产物是：

```text
var_values
  等式式的求解进展，例如 ?T = u32

external/region constraints
  不能简化为变量赋值的关系，例如 'a: 'b
```

## 正文

### 完整手算一：重复变量与缓存复用

调用方 A：

```text
goal: Pair<?T7, ?T7>: Trait
```

canonicalize：

```text
value:
  Pair<^0, ^0>: Trait

var_kinds:
  [Ty { ui: U0, sub_root: ^0 }]

original_values:
  [?T7]
```

调用方 B：

```text
goal: Pair<?T42, ?T42>: Trait
```

得到相同 canonical key：

```text
Pair<^0, ^0>: Trait
```

而：

```text
Pair<?T7, ?T8>: Trait
```

会变成：

```text
Pair<^0, ^1>: Trait
```

不会与前者误共用结果。canonicalization 保留变量相等/重复结构，同时移除本地 `TyVid` 身份。

### 完整手算二：query 求解输入变量

调用方：

```text
?T0@U0 = Unknown
goal: Vec<?T0>: ElementIsU32
```

canonical input：

```text
Canonical {
  value: Vec<^0>: ElementIsU32,
  var_kinds: [Ty(U0)],
  max_universe: U0,
}

OriginalQueryValues:
  [ ?T0 ]
```

查询内部：

```text
^0 -> ?Q0
goal: Vec<?Q0>: ElementIsU32
```

假设唯一 impl 使：

```text
?Q0 = u32
```

response：

```text
var_values for input vars: [u32]
certainty: Yes / Proven
external constraints: []
```

回到调用方：

```text
eq(?T0, u32)
caller table: ?T0 -> Known(u32)
```

### 完整手算三：没有推理进展

canonical input：

```text
goal: ^0: SomeTrait
```

查询内部 `?Q0` 无法唯一确定，且 goal ambiguous。响应可能概念化为：

```text
Canonical response {
  response var_kinds: [Ty(U0)],
  value.var_values: [^0],
  certainty: Maybe/Ambiguous,
}
```

应用时 response `^0` 可重新映射为原调用方 `?T0`，因此不会无意义地创建永久的新变量，也不会凭空给 `?T0` 增加等式。

注意 ambiguous response 仍可能携带部分约束，用于引导 inference；不能把 `Maybe` 简化理解为“response 一定完全为空”。new solver 只有在特定 overflow 等情况下才主动构造 no-constraints ambiguous response。

### canonicalization 与普通 binder

两者都用“变量声明 + bound occurrence”的结构，但量词来源不同：

```text
普通 Binder:
  来自类型本身的 forall/exists 作用域
  访问用 de Bruijn index

Canonical:
  来自 query 边界对本地变量的抽象
  occurrence 使用 BoundVarIndexKind::Canonical
  var_kinds 是 canonical 参数表
```

原有 binder 内的 `ReBound(Bound(...))` 会保留其 binder 结构；canonicalizer 只替换 free inference variables、placeholders 和按 mode 选中的 free regions。两种 bound var 不应混淆。

### canonicalization 与 `FreshTy`

`FreshTy` 主要用于本地、轻量的缓存/探测场景，把 inference var 匿名化以形成短期 key；它不表示完整的跨查询协议。

canonicalization 额外保存：

```text
CanonicalVarKind
universe
placeholder 的 universal 身份
sub_root 关系
OriginalQueryValues
canonical response
region/external constraints
```

因此只有 canonicalization 能完整支持：

```text
跨 InferCtxt 执行
+ 全局缓存
+ 将答案安全映射回调用方
```

### canonical query 的缓存意义

canonical key 把“变量叫什么”从问题中消除：

```text
?T0: Iterator
?T17: Iterator
```

都可成为：

```text
^0: Iterator
```

但它仍保留会改变语义的部分：

```text
ParamEnv
TypingMode
变量种类
变量重复关系
universe / placeholder 信息
goal predicate
predefined opaques（new solver QueryInput）
```

所以 canonicalization 不是把问题“模糊化”，而是删除不具语义的本地身份，同时精确保留会影响答案的结构。

### 什么时候触发 canonicalization：以 `Pair<?T7, ?T7>: Trait` 为例

这样的 goal 通常先由 type checking、normalization 或 fulfillment 产生。它进入 trait solver 时仍引用调用方 `InferCtxt` 中的本地变量：

```text
caller goal:
  Pair<?T7, ?T7>: Trait
```

在当前 next solver 中，根 goal 和递归产生的 nested goal 都通过 `evaluate_goal_raw` 求值。该函数先 eager-resolve 已知变量，再调用 `canonicalize_goal`，然后才把 canonical input 交给 search graph：

```text
canonical input:
  Pair<^0, ^0>: Trait

OriginalQueryValues:
  ^0 -> ?T7
```

两个位置必须共用 `^0`，因为它们原本就是同一个推理变量。这个重复关系是 goal 的语义信息，而 `T7` 这个本地编号不是。

search graph 随后通过 `enter_canonical` 在 solver 的求值上下文中实例化输入：

```text
query-local goal:
  Pair<?Q0, ?Q0>: Trait
```

到这里才进入 candidate assembly/evaluation：

```text
1. 根据 Trait 和刚性 self-type head `Pair` 查找相关 impl
2. 对每个 impl candidate 实例化 impl 泛型参数
3. 在 candidate probe/snapshot 中匹配 impl trait-ref 与当前 goal
4. 将 impl where-clauses 加为 nested goals
5. 合并成功 candidate 的 response
```

例如存在：

```rust,ignore
impl Trait for Pair<u32, u32> {}
```

匹配 candidate 会得到：

```text
?Q0 = u32
```

查询响应将该结果写成相对于 canonical input 的变量赋值：

```text
response.var_values[0] = u32
```

返回调用方后，根据 `OriginalQueryValues` 建立：

```text
?T7 = u32
```

因此可以把边界总结为：

```text
普通 goal（调用方变量）
  -> canonicalize
canonical goal（search graph 的稳定输入）
  -> instantiate/enter_canonical
query-local goal
  -> assemble、probe、evaluate candidates
canonical response
  -> instantiate and apply
调用方变量与外部约束得到更新
```

canonicalization 包围了 candidate search，但 candidate lookup 和 impl-head matching 本身使用的是实例化后的 query-local goal。它的主要职责是为 search graph 提供稳定的缓存键、递归/循环识别身份，以及隔离调用方和查询内部的推理变量。

同一个 `InferCtxt` 内的普通关系操作，例如直接执行 `eq(?T7, u32)`，只需使用 unification table，不需要为了这次局部统一建立 canonical query。是否需要 canonicalization，关键看操作是否跨越 solver/query 的可缓存、递归求值边界，而不是值中是否恰好出现 inference variable。

### 什么时候调用 `canonicalize_response`

`canonicalize_response` 不在每次 candidate 匹配或每次统一后调用，而是在一次 canonical goal 的求值准备形成可返回结果时调用。

进入 canonical input 时，`enter_canonical` 实例化输入变量并在 `EvalCtxt.var_values` 中保存对应关系。假设输入是：

```text
canonical goal:
  Pair<^0, ^0>: Trait
```

进入查询后可概念化为：

```text
query-local goal:
  Pair<?Q0, ?Q0>: Trait

EvalCtxt.var_values:
  [ ?Q0 ]
```

这里数组下标 `0` 对应 canonical input 的变量槽 `^0`。求解过程中 candidate relation 可能将 `?Q0` 统一为 `u32`，但数组仍可以持有对 `?Q0` 的间接引用：

```text
var_values[0] = ?Q0
unification table: ?Q0 -> u32
```

当 candidate 已添加完 nested goals 后，`evaluate_added_goals_and_make_canonical_response` 会依次：

```text
1. 求值已添加的 nested goals
2. 执行 placeholder/leak 相关检查
3. 合并 shallow certainty 与 nested-goal certainty
4. 收集 region、opaque type、normalization nested-goal 等外部约束
5. eager-resolve var_values 与 external constraints
6. 构造 Response
7. canonicalize_response
```

若 `?Q0 = u32`，第五步得到：

```text
resolved var_values:
  [ u32 ]
```

最终响应为：

```text
Canonical<Response> {
  value.var_values: [u32],
  value.certainty: Yes,
  value.external_constraints: ...,
  var_kinds: [],
}
```

如果输入变量只被约束成包含查询内部新变量的类型：

```text
?Q0 = Vec<?Q1>
```

则 eager-resolve 后仍含有不能离开 query-local `InferCtxt` 的 `?Q1`：

```text
[Vec<?Q1>]
```

response canonicalization 会将其改写为：

```text
response.value.var_values: [Vec<^0>]
response.var_kinds: [Ty(U0)]
```

这里 response 中出现的 `^0` 属于“响应自己的 canonical variable 表”，不等同于 input 中的 `^0`。两层位置关系是：

```text
response.var_values[0]
  数组位置 0：回答 input variable ^0 最终变成了什么

Vec<^0> 中的 ^0
  response canonical var：代表尚未确定、需要在调用方重新实例化的新变量
```

调用方应用 response 时，会为 response `^0` 创建本地 fresh variable `?R0`，然后建立：

```text
original input value ?T7 = Vec<?R0>
```

因此 response 必须再次 canonicalize：查询内部可能创建新的 inference variables 和 placeholders，它们同样不能直接泄漏到调用方或缓存中。即使所有输入变量都已经解析为具体类型，`Canonical<Response>` 仍是 search graph 统一使用的结果协议，并同时携带 certainty 与 external constraints。

#### response 记录求解约束，不通过“唯一实现类型”反推裸 self type

类型约束产生在 candidate evaluation、normalization 和 relation 中；`canonicalize_response` 只负责读取并重新编码这些已经产生的约束。它本身不搜索 impl，也不决定 inference variable 的值。

例如：

```text
?T0: Trait
```

canonicalize 后是：

```text
^0: Trait
```

即使当前 crate graph 中看起来只有：

```rust,ignore
impl Trait for u32 {}
```

solver 通常也不会据此推出 `?T0 = u32`。当前 next solver 在 candidate assembly 中发现 normalized self type 本身仍是 type variable 时，不会枚举普通 impl candidates，而是保留 ambiguity。这样也避免把“当前只看到一个 impl”当成 trait 的封闭类型集合。

此时 response 若保留无约束结果，可概念化为：

```text
input:                  ^0: Trait
query-local value:      ?Q0
resolved var_values:    [?Q0]
canonical response:     [^0] with Maybe/Ambiguous
caller result:          ?T0 remains unconstrained
```

能够返回具体输入变量结果的典型情况，是 goal 已经提供足够刚性结构，使 impl matching 合法地产生等式。例如：

```text
Pair<?T0>: Trait

impl Trait for Pair<u32> {}
```

流程为：

```text
Pair<?T0>: Trait
  -> Pair<^0>: Trait
  -> query-local Pair<?Q0>: Trait
  -> 根据刚性 head Pair 查到相关 impl
  -> impl-head matching 得到 ?Q0 = u32
  -> response.var_values[0] = u32
  -> 调用方应用 response：?T0 = u32
```

另一种常见情况是 normalization 的输出变量。例如输入类型已经足够确定时：

```text
<KnownIterator as Iterator>::Item == ?U
```

projection candidate 可以约束 `?U = u32`，response 再将这个求解结果带回调用方。

因此 response 的数据来源是：

```text
调用方提供 input variables
  -> 查询内部为它们创建 query-local representatives
  -> solver 对 representatives 施加约束
  -> response.var_values 记录它们求解后的值
```

它既不是调用方原值的简单回传，也不是 canonicalization 阶段通过扫描 impl 得到的类型；它是“输入变量经过一次 solver 求值后的状态”。

## 常见概念辨析

1. canonical variable 不是普通 inference variable。

   它是 query 参数槽；进入查询 `InferCtxt` 后才实例化为 fresh inference variable 或 placeholder。

2. canonical bound var 不等于源类型中的 binder-bound var。

   前者使用 `BoundVarIndexKind::Canonical`，后者使用 `BoundVarIndexKind::Bound(DebruijnIndex)`。

3. `OriginalQueryValues` 不是 query key 的一部分。

   它属于调用方，记录如何把缓存响应映射回当前 `InferCtxt`。

4. response `var_values` 不是 original-values 的简单副本。

   它表示查询结束时各输入变量的值；可能是具体类型，也可能引用 response 自己的 canonical vars。

5. `max_universe` 不是 canonical var 的数量。

   数量由 `var_kinds.len()` 给出；`max_universe` 描述 universe 层级上界。

6. certainty 与约束是两个维度。

   `Yes/Proven` 仍可能带 region constraints；`Maybe/Ambiguous` 也可能携带部分 inference constraints。

7. query response 不直接共享 query-local inference variables。

   response 会再次 canonicalize，然后在调用方实例化并通过等式/约束提交结果。

## 本章小结

canonicalization 是推理状态与可缓存查询之间的边界协议。它把本地 inference vars、placeholders 和 free regions替换为按首次出现编号的 canonical vars，同时保存 `var_kinds`、universe 信息和调用方的 `OriginalQueryValues`。查询在独立 `InferCtxt` 中把 existential canonical vars 实例化为 fresh inference vars、把 universal vars 实例化为 placeholders。求解后，response 用 `var_values` 表达输入变量的求解结果，并携带 certainty、region/opaque/normalization 等外部约束。调用方实例化 response、将结果与原始变量统一并登记外部约束，从而既能复用缓存，又不会混淆不同推理上下文。
