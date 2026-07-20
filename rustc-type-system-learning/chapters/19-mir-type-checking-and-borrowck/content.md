---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "19"
document: content
status: completed
updated_at: 2026-09-25
---

# 19. MIR Type Checking 与 Borrowck

## 学习目标

- 说明 MIR 已有类型，为什么 borrowck 仍需要一次 MIR type check。
- 区分三件事：收集类型/region 约束、求出 region 值、在具体位置检查借用冲突。
- 从 `BorrowSet` 找到一笔借用的 `kind`、`region`、`borrowed_place` 与创建位置，并追踪它何时进入/离开活跃借用数据流。
- 区分 `R1: R2` 的 outlives 约束、`T: 'r` 的 type test，以及 opaque hidden type 的 member constraint。
- 知道当前源码中 Polonius、诊断来源字段和常规 borrowck 的衔接点。

## 前置知识

第 18 章的主线足够：liveness 给 region 位置下界，`R1: R2` 使 `Values(R1)` 吸收 `Values(R2)`，SCC 传播后检查 universal/type 边界。本章不要求先补第 17 章的高阶量词专项。

## 核心心智模型

先对照两个程序：

```rust
// A：可以修改
let mut n = 1;
let r = &n;
println!("{r}");
n = 2;

// B：修改时 r 后面仍要使用，因而会报借用冲突
let mut n = 1;
let r = &n;
n = 2;
println!("{r}");
```

两段里 `n` 与 `r` 的基本类型都已知。差别不是“B 的类型突然变成了另一种”，而是求出的借用 region 是否覆盖写入 `n = 2` 的位置，以及当时是否有一笔与写入冲突的借用。

```text
已有类型的 MIR
  ├─ MIR typeck：重新建立/检查类型关系，收集 region 约束、type tests、liveness
  ├─ NLL region inference：按第 18 章规则求 Values(R)，检查边界
  └─ borrowck 数据流：列出借用 → 计算各位置活跃借用 → 检查访问是否冲突
                               ↑             ↑
                            BorrowSet     regioncx 的解
```

`BorrowSet` 是借用目录，不是“这些借用此刻全都活跃”的结论。`RegionInferenceContext` 说明 region 覆盖哪些位置；`Borrows` 数据流维护在当前控制流点仍在作用域内的借用；`MirBorrowckCtxt::check_access_for_conflict` 再结合访问种类和 `Place` 是否冲突决定是否报错。

## 源码地图

| 仓库路径 | 关键符号 | 本章用途 |
|---|---|---|
| `compiler/rustc_borrowck/src/lib.rs` | `borrowck_collect_region_constraints`、`borrowck_check_region_constraints`、`get_flow_results`、`MirBorrowckCtxt::check_access_for_conflict` | 主流程、数据流、访问冲突 |
| `compiler/rustc_borrowck/src/type_check/mod.rs` | `type_check`、`MirTypeckResults`、`MirTypeckRegionConstraints` | 在已定型 MIR 上生成约束 |
| `compiler/rustc_borrowck/src/borrow_set.rs` | `BorrowSet::build`、`BorrowData`、`GatherBorrows::visit_assign` | 收集借用及其身份 |
| `compiler/rustc_borrowck/src/dataflow.rs` | `Borrows::new`、`kill_loans_out_of_scope_at_location` | 借用作用域数据流 |
| `compiler/rustc_borrowck/src/nll.rs` | `compute_regions` | 从约束得到 region 解 |
| `compiler/rustc_borrowck/src/region_infer/mod.rs` | `TypeTest` | `T: 'r` 型检查项 |
| `compiler/rustc_borrowck/src/region_infer/opaque_types/member_constraints.rs` | `apply_member_constraints` | opaque hidden type 中 region 的候选归属 |
| `compiler/rustc_borrowck/src/polonius/mod.rs` | `PoloniusContext::compute_loan_liveness` | 可选的 Polonius next loan-liveness 路径 |
| `compiler/rustc_borrowck/src/constraints/mod.rs` | `OutlivesConstraint` | 约束来源、span/category 等诊断信息 |

补充阅读：[rustc dev guide：MIR type checker](https://rustc-dev-guide.rust-lang.org/borrow-check/type-check.html)、[Region inference](https://rustc-dev-guide.rust-lang.org/borrow-check/region-inference.html)、[Member constraints](https://rustc-dev-guide.rust-lang.org/borrow-check/region-inference/member-constraints.html)。指南概念可辅助理解；路径、类型和当前处理时机以本仓库检出的实现为准。

## 源码精读

### 1. 主流程先收集，再求解、检查

来自 `compiler/rustc_borrowck/src/lib.rs` 的 `borrowck_collect_region_constraints`（保留主干，省略 move/Polonius 初始化细节）：

```rust
let universal_regions = nll::replace_regions_in_mir(&infcx, &mut body_owned, &mut promoted);
let borrow_set = BorrowSet::build(tcx, body, locals_are_invalidated_at_exit, &move_data);
let MirTypeckResults {
    constraints,
    universal_region_relations,
    // 其余结果字段省略
    ..
} = type_check::type_check(
    &infcx, body, &promoted, universal_regions, &location_table,
    &borrow_set, &mut polonius_facts, &move_data, Rc::clone(&location_map),
);
```

这段是流程摘录，不是可独立编译的原函数：省略了 `body`、`location_table`、`move_data` 等变量的准备。顺序很重要：先重编号、收集 `BorrowSet`，再执行 MIR typeck。`BorrowSet` 是 typeck 的输入之一；typeck 返回约束而不是最终 region 解。

同文件的 `borrowck_check_region_constraints` 将这些约束交给 `nll::compute_regions` 得到 `regioncx`，之后调用 `get_flow_results` 并遍历结果检查 MIR 访问。当前 `root_cx.rs` 还会为嵌套 body 与 opaque type 协调收集/检查的时机，因此“每个 body 都严格一条直线立即执行完”只是入门模型。

### 2. MIR typeck 返回什么

来自 `compiler/rustc_borrowck/src/type_check/mod.rs` 的 `MirTypeckResults` 与 `MirTypeckRegionConstraints`（省略部分字段）：

```rust
pub(crate) struct MirTypeckResults<'tcx> {
    pub(crate) constraints: MirTypeckRegionConstraints<'tcx>,
    pub(crate) universal_region_relations: Frozen<UniversalRegionRelations<'tcx>>,
    // region_bound_pairs、known_type_outlives_obligations、
    // deferred_closure_requirements、polonius_context 省略
}

pub(crate) struct MirTypeckRegionConstraints<'tcx> {
    // placeholder 相关字段省略
    pub(crate) liveness_constraints: LivenessValues,
    pub(crate) outlives_constraints: OutlivesConstraintSet<'tcx>,
    pub(crate) universe_causes: FxIndexMap<ty::UniverseIndex, UniverseInfo<'tcx>>,
    pub(crate) type_tests: Vec<TypeTest<'tcx>>,
}
```

`outlives_constraints` 是 region 间包含要求，`liveness_constraints` 是位置下界，`type_tests` 是稍后要核验的 type-outlives 事项。`universal_region_relations` 则是签名/前提中已知成立的关系，不应和这次检查**产生的要求**混成一张表。`type_check` 中会运行 `check_user_type_annotations`、`visit_body`、`equate_inputs_and_outputs`，然后 `liveness::generate`。

### 3. 借用目录保存哪些事实

来自 `compiler/rustc_borrowck/src/borrow_set.rs` 的 `BorrowData`（省略字段注释）：

```rust
pub struct BorrowData<'tcx> {
    pub(crate) reserve_location: Location,
    pub(crate) activation_location: TwoPhaseActivation,
    pub(crate) kind: mir::BorrowKind,
    pub(crate) region: RegionVid,
    pub(crate) borrowed_place: mir::Place<'tcx>,
    pub(crate) assigned_place: mir::Place<'tcx>,
}
```

`GatherBorrows::visit_assign` 遇到适合记录的 `Rvalue::Ref(region, kind, borrowed_place)` 时创建此记录。`borrowed_place` 是被借的对象（如 `n`），`assigned_place` 是保存引用的 MIR 位置（如局部变量 `r`）；二者不能颠倒。`region` 指向第 18 章求解的 NLL 变量，`reserve_location` 是借用开始处。`activation_location` 用于两阶段借用；普通共享借用通常是 `NotTwoPhase`。`BorrowSet::build` 收集借用，但此时还不知道它在每个位置是否活跃。

### 4. 用 region 解终止借用，再检查访问

来自 `compiler/rustc_borrowck/src/dataflow.rs` 的 `Borrows::kill_loans_out_of_scope_at_location`（省略注释）：

```rust
if let Some(indices) = self.borrows_out_of_scope_at_location.get(&location) {
    state.kill_all(indices.iter().copied());
}
```

`Borrows::new` 预先计算借用在何处离开作用域：常规路径使用 `regioncx.first_non_contained_inclusive` 寻找 region 不再覆盖的 MIR 位置；启用 `-Zpolonius=next` 时有另一套 `PoloniusOutOfScopePrecomputer`。可见“region 解”会进入借用数据流，而不是直接由 HIR 的词法块决定作用域。

来自 `compiler/rustc_borrowck/src/lib.rs` 的 `MirBorrowckCtxt::check_access_for_conflict`（只展示入口；兼容规则与诊断分支省略）：

```rust
let borrows_in_scope = self.borrows_in_scope(location, state);
each_borrow_involving_path(
    self,
    self.infcx.tcx,
    self.body,
    (sd, place_span.0),
    self.borrow_set,
    |borrow_index| borrows_in_scope.contains(borrow_index),
    // 判断 Read/Write/Reservation/Activation 与 borrow.kind 的组合，省略
);
```

调用点要同时给出**位置**、**访问的 Place**、**访问种类**、**在该位置活跃的 BorrowIndex 集合**。只知道 `Values(region)` 还不够；必须判定是否访问同一/重叠 Place，以及共享/可变借用、读/写、两阶段激活等是否相容。`access_place` 还会检查访问权限和初始化状态；`get_flow_results` 同时计算 `Borrows`、`MaybeUninitializedPlaces`、`EverInitializedPlaces`。

## 正文

### 一、已定型的 MIR 为什么还要 typeck

HIR typeck 已决定 `n: i32`、`r: &i32` 等类型，MIR 不是重新猜一次这些类型。关键是：为 NLL，MIR 中的 region 被换成新变量；编译器需要在这个新表示上重新建立赋值、调用、返回、用户显式注解等处的 region 关系。官方指南把这种“擦去旧 region、用新变量重新约束”称为 region uniquification。当前 `type_check::type_check` 源码既遍历 MIR，也检查用户类型注解，并记录 `MirTypeckRegionConstraints`。正常情况下，HIR 已通过的程序不应在 MIR typeck 因普通类型不匹配而失败；这一趟的主要产物是 borrowck 所需的 region 信息。

例如 `fn f<'a, 'b>(x: &'a u32) -> &'b u32 { x }`：MIR typeck 对返回类型建立 `'a: 'b` 要求；第 18 章的 region inference 传播、再查 `UniversalRegionRelations` 是否允许该关系。这是**类型/region 约束错误**，还没有进入“写入是否撞到一笔活跃借用”的判断。

### 二、type test 与 member constraint 分别回答什么

`TypeTest`（`compiler/rustc_borrowck/src/region_infer/mod.rs`）保存 `generic_kind`、`lower_bound: RegionVid`、`span`、`verify_bound`。它对应例如 `T: 'r` 的类型跨 region 有效性；不是两个 region 值互相包含的 `'a: 'b`。`solve` 在传播之后调用 `check_type_tests` 来核验它。

Member constraint 是另一件事，概念上写作“隐藏类型中的某个 region `'m` 必须等于可捕获实参中的一个选择：`'m ∈ {'a, 'b}`”。它不是 `'a: 'b`，也不是 `T: 'r`。当前检出源码的具体路径在 `region_infer/opaque_types/member_constraints.rs`：`apply_member_constraints` 收集 opaque 定义性使用中的候选 region，根据已有下界/上界与 universal 关系筛选，再选择可行的最小候选。`compute_definition_site_hidden_types` 用一个临时 `RegionCtxt` 做这一步；它的注释明确说明该临时 region 图不会直接修改常规 `compute_regions` 所用图，之后会对 opaque 使用重新检查。阅读旧版“member constraint 是 NLL 主求解器的第三类常规边”描述时，以这一当前源码路径为准。

### 三、BorrowSet 不是借用活跃集合

对 `let r = &n`，`Rvalue::Ref` 可在 `BorrowSet` 中对应一个 `BorrowIndex`。即使后来 `r` 已经没有用途，这条目录记录仍存在；删除记录不是结束借用的做法。结束借用通过 `Borrows` 数据流在合适的 MIR 位置移除活跃位。若 `r` 还要在 `n = 2` 后使用，region 含该写入位置，活跃借用仍会被 `check_access_for_conflict` 看见；若 `r` 的最后需求在写入前结束，就能在写入点之前退出活跃集合。

真正的判断还要考虑 Place：借用 `x.field1` 与写入 `x.field2` 是否冲突，不能只比较“都属于局部变量 `x`”。`each_borrow_involving_path` 会结合 Place 路径的关系、访问深度与借用种类过滤。两阶段可变借用的 reservation/activation 又有不同兼容规则；本章先认清入口和数据角色，暂不手算全部边界情况。

### 四、Polonius 是可选接口，不等同于常规流程的每一步

当前 `type_check::type_check` 只在 `polonius.is_next_enabled()` 时建立 `PoloniusContext`。`nll::compute_regions` 可在该路径调用 `compute_loan_liveness`，以 region/CFG 位置构成的局部约束图计算 loan liveness；`Borrows::new` 则选择对应的 out-of-scope 预计算器。另有 legacy facts/output 路径，`borrowck_check_region_constraints` 中的 `MirBorrowckCtxt` 可持有可选 `polonius_output`。这些分支说明 Polonius 与 NLL、BorrowSet、访问检查有接口，但不能把“所有 borrowck 都运行 Polonius”当作默认不变量。

### 五、诊断约束如何指回用户代码

第 18 章关注 `sup`、`sub` 两端；当前 `OutlivesConstraint` 还保存 `locations`、`span`、`category`、`variance_info`、`from_closure`。这些字段帮助解释要求从哪条赋值、返回、调用参数或注解产生，并挑选适合报错的源位置。`RegionInferenceContextInner::best_blame_constraint` 会在约束路径中找可归责的一条。诊断信息不是另一套求解语义：求解仍由 region 值和边界检查决定，字段帮助把“为什么要 `'a: 'b`”翻译为人能理解的错误。

## 常见误区

1. “MIR typeck 重新从零推断变量类型”：主要是为 NLL 在重新编号后的 MIR 上建立/检查 region 约束。
2. “`BorrowSet` 里存在一笔借用就永远冲突”：它是目录；活跃性还要看 region 解与数据流位置。
3. “region 包含写入点就必定报错”：还要看是否有借用进入当前活跃集合、Place 是否重叠、访问与借用种类是否冲突。
4. “`T: 'r`、`'a: 'b`、member constraint 是同一类约束”：三者分别涉及类型跨区有效性、region 包含、opaque lifetime 的候选归属。
5. “Polonius 总是默认执行”：当前实现有可选 next/legacy 分支，常规路径仍需按实际选项判断。
6. “报错 span/category 决定程序是否通过”：它们帮助归因；不替代约束传播与借用冲突检查。

## 本章小结

读一次 MIR borrowck，可按“`type_check` 产出约束 → `compute_regions` 得出合法 region 解 → `BorrowSet` 和 `Borrows` 找到当前位置仍有效的借用 → `check_access_for_conflict` 判断 Place/访问冲突”四步走。`TypeTest` 和 opaque member constraints 是不同旁支；Polonius 是可选实现接口；`OutlivesConstraint` 的来源字段支撑诊断。第 20 章将离开 borrowck，追踪已通过检查的泛型代码如何选择具体 `Instance` 并进入单态化。
