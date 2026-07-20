---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "05"
document: content
status: completed
updated_at: 2026-07-27
---

# 05. 推理上下文与统一

## 学习目标

完成本章后，应当能够：

1. 区分 Type IR 中的 inference variable handle 与 `InferCtxt` 中的可变求解状态。
2. 识别 `TyVar`、`IntVar`、`FloatVar`、region var 与 const var 的表示和存储位置。
3. 追踪 fresh variable 的创建、variable-variable 合并和 variable-value 实例化。
4. 解释 equality relation 为什么还要执行结构递归、generalization、occurs check 并产生 obligations。
5. 解释 snapshot、`probe` 与 `commit_if_ok` 如何支持回溯。
6. 区分 `shallow_resolve`、`resolve_vars_if_possible` 与 `fully_resolve`。
7. 把第 04 章的 universe nameability 接入推理变量的实际状态。

## 前置知识

- 第 01 章的 `Ty`、`Region`、`Const` 与 interned Type IR。
- 第 02 章的 `TypeFoldable` 与递归 fold。
- 第 04 章的 `ReVar`、`UniverseIndex`、placeholder 与 `can_name`。

## 核心心智模型

最重要的分层是：

```text
不可变、interned 的 Type IR
    Ty::Infer(TyVar(?T0))
    ConstKind::Infer(Var(?C0))
    ReVar(?r0)
             │ 这些节点只保存 ID
             ▼
InferCtxt 中的可变状态
    ?T0 -> union-find 等价类 root -> Unknown(U0) / Known(Ty)
    ?C0 -> unification table      -> Unknown(U0) / Known(Const)
    ?r0 -> region constraints     -> equality / outlives constraints
             │
             ▼
关系 API 修改状态并返回 obligations
    at(...).eq / sub
             │
             ▼
resolver 读取表，把已知解重新物化进 Type IR
```

因此，常说“把 `?T0` 变成 `u32`”是一种简写。更精确的实现描述是：

```text
Ty::Infer(TyVar(?T0)) 这个 handle 没有被原地改写；
InferCtxt 记录 ?T0 所在等价类的值为 Known(u32)；
后续 resolve 才返回 u32。
```

Snapshot 再给这套可变状态加上事务边界：

```text
start snapshot
  ├─ 尝试关系检查并写表/加约束
  ├─ commit：保留修改
  └─ rollback：按 undo log 恢复表、变量长度和 universe
```

## 源码地图

| 主题 | 当前仓库路径与关键符号 |
|---|---|
| 推理上下文及内部状态 | `compiler/rustc_infer/src/infer/mod.rs`：`InferCtxt`、`InferCtxtInner` |
| 创建推理变量 | `compiler/rustc_infer/src/infer/mod.rs`：`next_ty_var_with_origin`、`next_const_var_with_origin`、`next_region_var_in_universe` |
| 类型变量存储与统一 | `compiler/rustc_infer/src/infer/type_variable.rs`：`TypeVariableStorage`、`TypeVariableValue`、`equate`、`instantiate` |
| Type IR inference variants | `compiler/rustc_type_ir/src/ty_kind.rs`：`InferTy`；`compiler/rustc_type_ir/src/const_kind.rs`：`InferConst` |
| freshening 与缓存键 | `compiler/rustc_infer/src/infer/freshen.rs`：`TypeFreshener`；`compiler/rustc_trait_selection/src/traits/select/mod.rs`：selection cache / obligation stack |
| equality/subtyping 分派 | `compiler/rustc_infer/src/infer/relate/type_relating.rs`：`TypeRelation::tys` |
| generalization 与 occurs check | `compiler/rustc_infer/src/infer/relate/generalize.rs`：`instantiate_ty_var`、`instantiate_var`、`union_var_term` |
| 泛型函数与方法调用实例化 | `compiler/rustc_hir_typeck/src/fn_ctxt/_impl.rs`：`instantiate_value_path`；`method/confirm.rs`：`fresh_receiver_args`、`instantiate_method_sig`、`unify_receivers`；`fn_ctxt/checks.rs`：`check_argument_types` |
| region subtype constraint | `compiler/rustc_infer/src/infer/relate/type_relating.rs`：`TypeRelation::regions`；`infer/region_constraints/mod.rs`：`make_subregion` |
| lexical region resolution | `compiler/rustc_infer/src/infer/outlives/mod.rs`：`resolve_regions_with_normalize`；`infer/lexical_region_resolve/mod.rs`：`LexicalRegionResolutions` |
| NLL region 求解 | `compiler/rustc_borrowck/src/constraints/mod.rs`：`OutlivesConstraint`；`region_infer/mod.rs`：`propagate_constraints`、`check_universal_regions` |
| HIR region 擦除与 MIR 重编号 | `compiler/rustc_hir_typeck/src/writeback.rs`：`Resolver::handle_term`；`compiler/rustc_borrowck/src/nll.rs`：`replace_regions_in_mir` |
| snapshot 与回滚 | `compiler/rustc_infer/src/infer/snapshot/mod.rs`：`CombinedSnapshot`、`commit_if_ok`、`probe` |
| 解析变量 | `compiler/rustc_infer/src/infer/mod.rs`：`shallow_resolve`、`resolve_vars_if_possible`、`fully_resolve`；`infer/resolve.rs` |
| 总体开发者文档 | `src/doc/rustc-dev-guide/src/type-inference.md` |

## 源码精读

以下片段来自当前检出的 rustc 源码。代码只保留本章所需部分，路径与符号名是后续导航依据。

### 1. `InferCtxtInner`：可回滚状态集中在一个 `RefCell` 中

路径：[`compiler/rustc_infer/src/infer/mod.rs`](../../../compiler/rustc_infer/src/infer/mod.rs)，符号：`InferCtxtInner`、`InferCtxt`。

```rust,ignore
pub struct InferCtxtInner<'tcx> {
    undo_log: InferCtxtUndoLogs<'tcx>,
    projection_cache: traits::ProjectionCacheStorage<'tcx>,
    type_variable_storage: type_variable::TypeVariableStorage<'tcx>,
    const_unification_storage: UnificationTableStorage<ConstVidKey<'tcx>>,
    int_unification_storage: UnificationTableStorage<IntVid>,
    float_unification_storage: UnificationTableStorage<FloatVid>,
    region_constraint_storage: Option<RegionConstraintStorage<'tcx>>,
    // ...
}

pub struct InferCtxt<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    // typing mode、universe 等外层配置……
    inner: RefCell<InferCtxtInner<'tcx>>,
    // ...
}
```

`InferCtxt` 是一次推理会话的边界。`TyCtxt` 提供全局、interned 的类型数据；`inner` 保存这次推理会话中不断变化、且需要 snapshot 的状态。

把表集中在 `RefCell` 内有两个直接结果：

1. 大多数关系 API 可以只借用 `&InferCtxt`，内部再取得短暂的可变借用。
2. snapshot 可以统一记录类型表、常量表、缓存和 region constraints 的变化。

### 2. 创建变量：同时产生 IR handle 和表中记录

路径：[`compiler/rustc_infer/src/infer/mod.rs`](../../../compiler/rustc_infer/src/infer/mod.rs)，符号：`next_ty_var_with_origin`、`next_const_var_with_origin`、`next_region_var_in_universe`。

```rust,ignore
pub fn next_ty_var_with_origin(&self, origin: TypeVariableOrigin) -> Ty<'tcx> {
    let vid = self.next_ty_vid_with_origin(origin);
    Ty::new_var(self.tcx, vid)
}

pub fn next_ty_vid_with_origin(&self, origin: TypeVariableOrigin) -> TyVid {
    self.inner
        .borrow_mut()
        .type_variables()
        .new_var(self.universe(), origin)
}
```

这两步分别创建：

```text
表中：TyVid -> Unknown { universe }，并保存 origin
IR 中：Ty::Infer(TyVar(TyVid))
```

Const variable 的流程相同，只是使用 const unification table：

```rust,ignore
let vid = self
    .inner
    .borrow_mut()
    .const_unification_table()
    .new_key(ConstVariableValue::Unknown {
        origin,
        universe: self.universe(),
    })
    .vid;
ty::Const::new_var(self.tcx, vid)
```

Region variable 则进入 region constraint collector：

```rust,ignore
let region_var = self
    .inner
    .borrow_mut()
    .unwrap_region_constraints()
    .new_region_var(universe, origin);
ty::Region::new_var(self.tcx, region_var)
```

三者都把创建时的 universe 写入推理状态，所以第 04 章的 nameability 规则不是额外的理论标签，而是后续合并与实例化必须维护的实现不变量。

### 3. 类型变量的状态：union-find 等价类加 `Unknown/Known`

路径：[`compiler/rustc_infer/src/infer/type_variable.rs`](../../../compiler/rustc_infer/src/infer/type_variable.rs)，符号：`TypeVariableStorage`、`TypeVariableValue`。

```rust,ignore
pub(crate) struct TypeVariableStorage<'tcx> {
    values: IndexVec<TyVid, TypeVariableData>,
    eq_relations: UnificationTableStorage<TyVidEqKey<'tcx>>,
    sub_unification_table: UnificationTableStorage<TyVidSubKey>,
}

pub(crate) enum TypeVariableValue<'tcx> {
    Known { value: Ty<'tcx> },
    Unknown { universe: UniverseIndex },
}
```

`eq_relations` 同时承担两项工作：

- union-find：记录哪些 `TyVid` 属于同一个 equality 等价类；
- class value：在等价类的 root 上记录 `Unknown(U)` 或 `Known(Ty)`。

两个关键更新操作是：

```rust,ignore
pub(crate) fn equate(&mut self, a: TyVid, b: TyVid) {
    self.eq_relations().union(a, b);
    self.sub_unification_table().union(a, b);
}

pub(crate) fn instantiate(&mut self, vid: TyVid, ty: Ty<'tcx>) {
    let vid = self.root_var(vid);
    self.eq_relations()
        .union_value(vid, TypeVariableValue::Known { value: ty });
}
```

对应关系是：

```text
?T0 == ?T1      -> equate：合并两个未知变量的等价类
?T0 == u32      -> instantiate：在 root 上写入 Known(u32)
```

两个 `Unknown` 合并时，当前 `UnifyValue` 实现取 universe 的最小值：

```rust,ignore
let universe = cmp::min(universe1, universe2);
Ok(TypeVariableValue::Unknown { universe })
```

若 `?T0@U1 == ?T1@U0`，最终共同的值必须同时能被 U1 和 U0 命名，因此合并后的 class 采用限制更强的 U0。

### 4. 统一入口：关系检查先解析，再决定合并、实例化或递归

路径：[`compiler/rustc_infer/src/infer/relate/type_relating.rs`](../../../compiler/rustc_infer/src/infer/relate/type_relating.rs)，符号：`TypeRelation::tys`。

```rust,ignore
let a = infcx.shallow_resolve(a);
let b = infcx.shallow_resolve(b);

match (a.kind(), b.kind()) {
    (&ty::Infer(TyVar(a_id)), &ty::Infer(TyVar(b_id))) => {
        match self.ambient_variance {
            ty::Invariant => {
                infcx.inner.borrow_mut().type_variables().equate(a_id, b_id);
            }
            ty::Covariant | ty::Contravariant => {
                // 记录 subtype obligation
            }
            // ...
        }
    }
    (&ty::Infer(TyVar(a_vid)), _) => {
        infcx.instantiate_ty_var(self, true, a_vid, self.ambient_variance, b)?;
    }
    (_, &ty::Infer(TyVar(b_vid))) => {
        infcx.instantiate_ty_var(/* ... */)?;
    }
    _ => {
        // 结构化地 relate 两个非变量类型
    }
}
```

因此“unification table”不是 equality API 的全部。`infcx.at(...).eq(a, b)` 还负责：

- 解析已经有值的 inference vars；
- 递归比较 `Vec<A>` 与 `Vec<B>` 等结构；
- 按 variance 处理子类型方向；
- 返回后续需要满足的 obligations；
- 在变量对结构化类型时执行 generalization 和 occurs check。

### 5. `instantiate_ty_var`：写表前先阻止循环类型

路径：[`compiler/rustc_infer/src/infer/relate/generalize.rs`](../../../compiler/rustc_infer/src/infer/relate/generalize.rs)，符号：`instantiate_ty_var`、`instantiate_var`、`union_var_term`。

```rust,ignore
pub fn instantiate_ty_var(/* ... */) -> RelateResult<'tcx, ()> {
    self.instantiate_var(
        relation,
        target_is_expected,
        target_vid.into(),
        instantiation_variance,
        source_ty.into(),
    )
}

fn instantiate_var(/* ... */) -> RelateResult<'tcx, ()> {
    let Generalization { value_may_be_infer: generalized_term } =
        self.generalize(/* ... */)?;

    self.union_var_term(target_vid, generalized_term);
    // 再 relation generalized_term 与 source_term……
}
```

源码注释明确说明 `generalize` 同时执行 occurs check。对于：

```text
?T0 == Vec<?T0>
```

若直接把右侧写成 `?T0` 的值，解析时会得到无限展开的 `Vec<Vec<...>>`。occurs check 在写表前发现目标变量出现在候选值中并拒绝该关系。

这也是源码要求普通调用者使用 `At::eq` 等关系 API，而不是直接调用内部 `instantiate` 的原因：内部写表函数依赖调用者已经维护好结构、variance、universe 和无环性等前置条件。

### 6. Snapshot：用 undo log 实现事务式尝试

路径：[`compiler/rustc_infer/src/infer/snapshot/mod.rs`](../../../compiler/rustc_infer/src/infer/snapshot/mod.rs)，符号：`CombinedSnapshot`、`commit_if_ok`、`probe`。

```rust,ignore
pub struct CombinedSnapshot<'tcx> {
    undo_snapshot: Snapshot<'tcx>,
    region_constraints_snapshot: RegionSnapshot,
    universe: UniverseIndex,
}

pub fn commit_if_ok<T, E, F>(&self, f: F) -> Result<T, E> {
    let snapshot = self.start_snapshot();
    let r = f(&snapshot);
    match r {
        Ok(_) => self.commit_from(snapshot),
        Err(_) => self.rollback_to(snapshot),
    }
    r
}

pub fn probe<R, F>(&self, f: F) -> R {
    let snapshot = self.start_snapshot();
    let r = f(&snapshot);
    self.rollback_to(snapshot);
    r
}
```

`CombinedSnapshot` 保存 undo-log 位置、region constraint snapshot 和进入时的 universe。回滚时会恢复 universe、逆放 undo log，并截去 snapshot 内新建的变量记录。

语义可以压缩为：

```text
probe(f)          = 执行 f，观察结果，始终回滚副作用
commit_if_ok(f)   = f 返回 Ok 时提交，返回 Err 时回滚
```

这让候选搜索、coercion 尝试和“是否可以相等”的查询能够共享一套推理机制。

### 7. 三种解析深度

路径：[`compiler/rustc_infer/src/infer/mod.rs`](../../../compiler/rustc_infer/src/infer/mod.rs) 与 [`compiler/rustc_infer/src/infer/resolve.rs`](../../../compiler/rustc_infer/src/infer/resolve.rs)，符号：`shallow_resolve`、`OpportunisticVarResolver`、`fully_resolve`。

`shallow_resolve` 只在输入类型的最外层是 `Infer` 时读取表：

```rust,ignore
pub fn shallow_resolve(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
    if let ty::Infer(v) = *ty.kind() {
        // TyVar / IntVar / FloatVar：查询当前已知值
    } else {
        ty
    }
}
```

`resolve_vars_if_possible` 使用 folder 深入整个结构：

```rust,ignore
let mut r = resolve::OpportunisticVarResolver::new(self);
value.fold_with(&mut r)
```

它只替换已有解的 type/const variables，保留尚无解的变量，而且明确不处理 region vars。

`fully_resolve` 则是 writeback 风格的严格收尾：

```rust,ignore
pub fn fully_resolve(/* ... */) -> FixupResult<T> {
    value.try_fold_with(&mut FullTypeResolver { infcx })
}
```

未解析的 type/int/float/const variable 会产生 `FixupError`；`ReVar` 需要 region inference 已经完成，才能通过 region resolutions 换成结果。

## 正文

### 1. 推理变量是间接引用

考虑：

```rust
let mut xs = Vec::new();
xs.push(22_u32);
```

在 `Vec::new()` 处，元素类型未知，可概念化为：

```text
xs: Vec<?T0>
```

此时有两份相互关联的数据：

```text
Type IR:  Vec<Ty::Infer(TyVar(?T0))>
table:    ?T0 -> Unknown(U0)
```

处理 `push(22_u32)` 后，关系检查得到 `?T0 == u32`：

```text
Type IR:  仍可持有 Vec<Ty::Infer(TyVar(?T0))>
table:    root(?T0) -> Known(u32)
```

对 `xs` 的类型执行深层解析后才得到 `Vec<u32>`。这种间接层允许许多共享同一个 `TyVid` 的 IR 节点同时观察到同一求解结果。

#### 1.1 `Vec::new()` 与 `push(value: T)` 如何共享一个推理变量

关联不是通过源码中的参数名 `T` 自动完成的，而是通过：

```text
GenericArgs 实例化
  + receiver 类型匹配
  + equality class 合并
```

完成的。

当前 [`Vec`](../../../library/alloc/src/vec/mod.rs) 中两个定义概念上是：

```rust
impl<T> Vec<T> {
    pub const fn new() -> Self;
    pub fn push(&mut self, value: T);
}
```

这里 `new` 与 `push` 的 `T` 都是所属 `impl<T>` 的 parent generic parameter。

##### 第一步：`Vec::new()` 创建 `?T0`

检查 value path `Vec::new` 时，用户没有提供 `Vec::<...>` 的类型实参。
[`instantiate_value_path`](../../../compiler/rustc_hir_typeck/src/fn_ctxt/_impl.rs)
构造完整 `GenericArgs`；缺失参数走 `inferred_kind`：

```rust,ignore
fn inferred_kind(/* ..., */ param: &GenericParamDef, /* ... */) -> GenericArg<'tcx> {
    self.fcx.var_for_def(self.span, param)
}
```

`var_for_def` 为 impl 的 `T` 创建 `TyVar(?T0)`，于是：

```text
args(new) = [?T0]
```

再用这组 args 实例化 `new` 的签名：

```text
定义签名：fn() -> Self
Self：    Vec<T>

实例化后：fn() -> Vec<?T0>
```

因此调用结果和局部变量成为：

```text
xs: Vec<?T0>
eq_relations: ?T0 -> Unknown(U0)
```

##### 第二步：从 receiver 选择 `push`

检查：

```rust
xs.push(22_u32)
```

[`check_expr_method_call`](../../../compiler/rustc_hir_typeck/src/expr.rs) 首先得到：

```text
receiver type = Vec<?T0>
```

方法 probe 枚举 inherent impl candidate 时，会为候选：

```rust
impl<T> Vec<T>
```

创建一组 fresh impl args。概念上：

```text
candidate args = [?T1]
candidate self type = Vec<?T1>
```

然后把候选 self type 与实际 receiver 关联：

```text
Vec<?T0>  related-to  Vec<?T1>
    ↓ 递归比较 Vec 的第 0 个参数
?T0       related-to  ?T1
```

probe 用 snapshot 判断候选是否可用；候选选定后，confirm 阶段重新执行正式实例化。

##### 第三步：确认方法时合并 receiver args

[`ConfirmContext::confirm`](../../../compiler/rustc_hir_typeck/src/method/confirm.rs)
依次执行：

```rust,ignore
let rcvr_args = self.fresh_receiver_args(self_ty, pick);
let all_args = self.instantiate_method_args(pick, segment, rcvr_args);
let (method_sig, method_predicates) = self.instantiate_method_sig(pick, all_args);
self.unify_receivers(self_ty, method_sig_rcvr, pick);
```

对 inherent `Vec::push`，`fresh_receiver_args` 为 `impl<T>` 创建正式的 fresh variable，记作 `?T1`：

```text
rcvr_args = [?T1]
```

`instantiate_method_sig` 使用它替换 `push` 签名中的 impl parameter：

```text
定义签名：
    fn(&mut Vec<T>, value: T)

实例化后：
    fn(&mut Vec<?T1>, value: ?T1)
```

`unify_receivers` 再关联实际 receiver 与签名 receiver：

```text
实际：&mut Vec<?T0>
形式：&mut Vec<?T1>
```

在这里递归得到 `?T0 == ?T1`，`eq_relations` 合并两个 class：

```text
class C0 = { ?T0, ?T1 } -> Unknown(U0)
```

因此即使实现过程中创建了另一个 `TyVid`，`push` 的 `value: ?T1` 与
`xs` 的 element `?T0` 已经共享同一个 equality root。

##### 第四步：实参检查把 class 实例化为 `u32`

方法签名的 `self` input 已经单独处理，传给
[`check_argument_types`](../../../compiler/rustc_hir_typeck/src/fn_ctxt/checks.rs)
的普通参数部分是：

```text
formal input:  ?T1
provided arg:  22_u32 : u32
```

实参检查先按 expected type 检查/coerce 表达式，再要求 formal type 与实际采用的
coerced type 相等。于是产生：

```text
?T1 == u32
```

由于 `?T0` 与 `?T1` 已在同一个 equality class：

```text
class C0 = { ?T0, ?T1 } -> Known(u32)
```

最终：

```text
resolve(?T0) = u32
resolve(xs)  = Vec<u32>
```

完整的数据流是：

```text
Vec::new()
  -> args = [?T0]
  -> return Vec<?T0>
  -> xs: Vec<?T0>

xs.push(...)
  -> actual receiver Vec<?T0>
  -> instantiate impl<T> with [?T1]
  -> method sig fn(&mut Vec<?T1>, ?T1)
  -> unify receivers: ?T0 == ?T1
  -> check 22_u32 against ?T1: ?T1 == u32
  -> shared root becomes Known(u32)
  -> xs resolves to Vec<u32>
```

### 2. 五类推理变量及其专门约束

| 类别 | Type IR 表示 | 主要状态 | 约束特点 |
|---|---|---|---|
| 通用类型变量 | `InferTy::TyVar(TyVid)` | `TypeVariableStorage` | 可与一般类型建立关系 |
| 整数变量 | `InferTy::IntVar(IntVid)` | int unification table | 只能取整数类型，后续可 fallback |
| 浮点变量 | `InferTy::FloatVar(FloatVid)` | float unification table | 只能取浮点类型，后续可 fallback |
| 常量变量 | `InferConst::Var(ConstVid)` | const unification table | 保存 const 值与 universe |
| region 变量 | `RegionKind::ReVar(RegionVid)` | region constraint collector | 主要收集 equality/outlives constraints |

[`InferTy`](../../../compiler/rustc_type_ir/src/ty_kind.rs) 中还有 `FreshTy`、`FreshIntTy`、`FreshFloatTy`，[`InferConst`](../../../compiler/rustc_type_ir/src/const_kind.rs) 中还有 `Fresh`。这些是 `TypeFreshener` 为缓存等用途生成的替代标记；它们不代表当前 `InferCtxt` 中等待赋值的 live variable。

#### 2.1 `FreshTy`：为缓存生成的局部、不可求解占位符

[`TypeFreshener`](../../../compiler/rustc_infer/src/infer/freshen.rs) 的目标不是创建更多推理变量，而是把一个仍含 live inference vars 的值转换成：

```text
“当前 InferCtxt 已知信息”的稳定摘要
```

主要流程是：

```text
已知的 inference var
    -> 换成已知解，并递归 freshen 解内部

仍未知的 TyVar root
    -> FreshTy(按首次出现顺序编号)

仍未知的 IntVar
    -> FreshIntTy(编号)

仍未知的 FloatVar
    -> FreshFloatTy(编号)

仍未知的 ConstVar
    -> InferConst::Fresh(编号)
```

源码中的通用编号逻辑为：

```rust,ignore
match self.ty_freshen_map.entry(input) {
    Entry::Occupied(entry) => *entry.get(),
    Entry::Vacant(entry) => {
        let index = self.ty_freshen_count;
        self.ty_freshen_count += 1;
        let t = mk_fresh(index);
        entry.insert(t);
        t
    }
}
```

因此编号由一次 `TypeFreshener` traversal 中的首次出现顺序决定，而不是来自 `TyVid` 本身。

例如当前状态为：

```text
?T7  -> Unknown
?T11 -> Unknown
```

则：

```text
(?T7, ?T7)  -> (FreshTy(0), FreshTy(0))
(?T7, ?T11) -> (FreshTy(0), FreshTy(1))
```

若 `?T7` 和 `?T11` 已经 equality-unified，它们具有相同 root，freshener 会把二者都映射到同一个 `FreshTy(0)`。

若：

```text
?T7 -> Known(u32)
```

则 freshening 的结果直接是：

```text
?T7 -> u32
```

不会再产生 `FreshTy`。

##### 为什么缓存不能直接使用 `TyVar(TyVid)`

两个推理会话可能遇到结构完全相同的 goal，却使用不同的内部编号：

```text
会话 A：Vec<?T7>: Trait
会话 B：Vec<?T42>: Trait
```

直接把 `TyVid` 放进 cache key，会让两个同形问题看起来不同；缓存也不应为了判断能否复用结果而给 live variable 赋值。

分别 freshen 后：

```text
Vec<FreshTy(0)>: Trait
Vec<FreshTy(0)>: Trait
```

便可以用 equality/hash 比较结构。若变量后来解析为 `u32`，下一次 key 会变为：

```text
Vec<u32>: Trait
```

这反映了 inferencer 已经获得更多信息。

当前 old trait selection 使用 freshened predicate：

- 作为 candidate selection cache 的 key；
- 比较 obligation stack 上是否再次出现同形 goal，以检测递归；
- 用 `MatchAgainstFreshVars` 做不修改 inference state 的近似匹配。

##### `FreshTy` 为什么不能继续求解

两种表示虽然都位于 `InferTy`，生命周期和能力不同：

| 表示 | ID 来源 | 是否有 table entry | 能否实例化/resolve | 作用域 |
|---|---|---:|---:|---|
| `TyVar(TyVid)` | `InferCtxt::new_var` | 有：`Unknown/Known`、universe、origin | 能 | 当前 inference context |
| `FreshTy(u32)` | `TypeFreshener` 局部计数器 | 无 | 不能 | 一次 freshened value / cache 比较 |

`FreshTy(0)` 没有对应的 `TyVid`，也没有：

```text
eq_relations entry
universe
origin
```

`shallow_resolve` 遇到它会原样返回；再次对已经 freshened 的值执行 freshening，源码会视为内部使用错误。模块注释也要求 freshened type 只作为内部缓存/匹配表示，不进入用户诊断或一般类型运算。

##### 为什么区分 `FreshTy`、`FreshIntTy` 和 `FreshFloatTy`

原始 inference vars 的可选值范围不同：

```text
TyVar      可以成为一般类型
IntVar     只能成为整数类型
FloatVar   只能成为浮点类型
```

freshening 保留这种类别信息，避免把“任意未知类型”“未知整数”和“未知浮点数”压成同一种 wildcard。Const inference 使用独立的 `InferConst::Fresh`。

##### 与 canonicalization 的区别

两者都会替换 inference vars，但用途不同：

```text
freshening
    轻量、允许信息丢失
    服务本地 cache key、递归检测和近似匹配
    不支持把结果实例化回 live variables

canonicalization
    保存 variable kind、universe、sub-root 等查询信息
    服务跨 inference context 的 solver query
    支持实例化 canonical response
```

##### 为什么有了 canonicalization 仍保留 `FreshTy`

两者处理的输入相似，但交付物不同：

```text
Freshening 的交付物：
    一个可以直接 equality/hash/match 的 Ty / Predicate

Canonicalization 的交付物：
    Canonical<Value>
    + CanonicalVarKinds
    + OriginalQueryValues
    + universe/sub-root 映射
    + 可实例化的 query response
```

对于 legacy trait selection 的本地缓存，问题通常只是：

```text
“这个结构形状的 obligation 是否已经算过？”
“当前 obligation stack 中是否出现了同形递归？”
```

例如：

```text
原 goal：     Vec<?T7>: Trait
freshened key: Vec<FreshTy(0)>: Trait
```

这里不需要把缓存结果中的变量映射回 `?T7`，也不需要传播 universe、region constraints
或 canonical response；直接对 freshened predicate 做 hash/equality 即可。

Canonical query 面对的问题更完整：

```text
“把这个 goal 交给另一个可缓存的 solver 查询；
查询返回后，怎样把变量解、constraints 和 obligations
安全地映射回调用者的 InferCtxt？”
```

它会保留：

```rust,ignore
CanonicalVarKind::Ty {
    ui: universe,
    sub_root,
}
```

并记录 canonical variable 与调用者原始 `GenericArg` 的对应关系。实例化 response
时才能重新创建或复用 live inference vars、恢复 subtype-related 信息并应用约束。

因此，用 canonicalization 代替一次本地 freshening 虽然在架构上可以设计，但会为只需
hash key 的热路径引入额外的 var metadata、映射和 response 协议；同时 canonicalization
保留的信息也比 legacy cache 所需更多。Freshening 还会有意擦除大部分 free regions，
以符合 trait selection 通常不根据具体 region 关系选择类型级候选的策略。

当前源码也反映了两套路径的历史边界：

- [`SelectionContext::candidate_from_obligation`](../../../compiler/rustc_trait_selection/src/traits/select/mod.rs)
  先断言当前不是 next solver，再使用 `TypeFreshener` 构造 candidate cache key；
- next solver 的查询路径使用 canonicalization，将 variable kinds 和 response 映射作为查询协议的一部分。

所以两者并存不是因为类型理论要求两套“未知变量”，而是 rustc 当前同时需要：

```text
legacy/local algorithm：
    低成本、只读、可丢信息的形状摘要
        -> FreshTy

solver query boundary：
    信息充分、可以来回实例化的变量协议
        -> CanonicalVar
```

若未来把相关 legacy cache 和递归检测全部重构到统一的 solver/query 架构，可以重新评估
`FreshTy` 的使用范围；但在当前实现中，它仍是一个更轻量且语义有意更粗的内部表示。

因此可以把 `FreshTy` 理解为“缓存视图中的未知类型编号”，而不是“尚待 rustc 求解的新 `TyVid`”。

### 3. 创建、合并、实例化、解析

一个通用类型变量的生命周期可以写成：

```text
new_var(U0, origin)
  -> ?T0 class = Unknown(U0)

equate(?T0, ?T1)
  -> union-find 合并 class

instantiate(root(?T0), u32)
  -> class = Known(u32)

resolve(?T0), resolve(?T1)
  -> u32
```

`origin` 主要用于诊断，`universe` 则限制最终值中允许出现哪些 placeholder names。

### 4. 为什么两个 unknown 合并取较小 universe

设：

```text
?T0 创建于 U1
?T1 创建于 U0
?T0 == ?T1
```

它们合并后必须获得同一个值。U1 中的变量能命名 U0 与 U1 的名字；U0 中的变量只能命名 U0 的名字。共同可接受的范围是 U0，因此实现把等价类的 universe 设为：

```text
min(U1, U0) = U0
```

这与上一章的 region nameability 是同一个原则，只是此处直接体现在 `TypeVariableValue::unify_values` 中。

### 5. Equality 不等于一次表写入

关系检查面对的输入有三种典型形态：

```text
变量 == 变量       合并未知变量的等价类
变量 == 结构类型   generalize + occurs check + 实例化
结构类型 == 结构类型  按类型构造递归比较参数
```

例如：

```text
Vec<?T0> == Vec<u32>
```

先确认外层构造都是 `Vec`，再递归得到 `?T0 == u32`。如果类型中出现 alias、subtyping 或 trait 条件，关系结果还会携带 obligations；第 08 章会继续追踪它们的生命周期。

### 6. Occurs check 维护有限 Type IR

下式没有有限解：

```text
?T0 == Vec<?T0>
```

若允许实例化，重复解析会无限展开。`instantiate_ty_var` 通过 generalization traversal 检查候选结构中是否再次出现目标 inference class，在写入表之前报告错误或按 alias 情况延迟处理。

要把它与普通递归类型区分开：

```rust
struct Node(Box<Node>);
```

这里递归经过命名 ADT `Node` 和指针间接层，Type IR 本身仍是有限图；`?T0 = Vec<?T0>` 则试图让一个推理变量直接等于包含自身的展开式。

### 7. Region inference 以约束收集为主

类型变量经常可以较早得到 `Known(Ty)`；region inference 通常保留：

```text
'a: 'b
```

或等价的 subregion 形式，等类型检查收集完信息后再集中求解。region equality 可以做一定的 eager unification，以便把相等 region vars opportunistically 归到同一个代表，但 outlives constraints 仍是主要接口。

因此：

```text
resolve_vars_if_possible
```

只深层解析 type/const vars，不顺带宣称 region constraints 已经求解。第 18 章会进入 NLL 的 universal regions、liveness、SCC 和 constraint propagation。

#### 7.1 `'a: 'b` 如何从 subtype relation 变成 region constraint

`'a: 'b` 表示 `'a` outlives `'b`，等价地：

```text
'b <= 'a
```

它不进入 `sub_unification_table`；该表的 key 是 `TyVid`，只记录类型推理变量之间的关联。Lifetime 关系进入独立的 region constraint 系统。

考虑引用的协变关系：

```text
&'a u32 <: &'b u32
```

要把一个在 `'a` 内有效的引用当作在 `'b` 内有效的引用使用，必须满足：

```text
'a: 'b
```

当前 [`TypeRelation::regions`](../../../compiler/rustc_infer/src/infer/relate/type_relating.rs) 的 covariant 分支直接执行：

```rust,ignore
// Subtype(&'a u8, &'b u8)
// => Outlives('a: 'b)
// => SubRegion('b, 'a)
self.infcx
    .inner
    .borrow_mut()
    .unwrap_region_constraints()
    .make_subregion(origin, b, a, VisibleForLeakCheck::Yes);
```

`make_subregion(sub, sup)` 记录的是 `sub <= sup`，所以参数顺序为：

```text
make_subregion('b, 'a)
```

若关系来自一个显式 `RegionOutlivesPredicate('a, 'b)`，入口
[`register_region_outlives_constraint`](../../../compiler/rustc_infer/src/infer/outlives/obligations.rs)
也会做同样的翻转：

```rust,ignore
// `'a: 'b` ==> `'b <= 'a`
self.sub_regions(origin, r_b, r_a, vis);
```

#### 7.2 Trait solver 的 `Yes` 表示成功注册约束

next trait solver 处理 `RegionOutlives('a, 'b)` 时调用
[`compute_region_outlives_goal`](../../../compiler/rustc_next_trait_solver/src/solve/mod.rs)：

```rust,ignore
let OutlivesPredicate(a, b) = goal.predicate;
self.register_region_outlives(a, b, VisibleForLeakCheck::Yes);
self.evaluate_added_goals_and_make_canonical_response(Certainty::Yes)
```

这里的 `Certainty::Yes` 表示 solver 已经把 goal 成功降低为 region constraint，并不是 region inference 已经完成。真正的包含关系会在后续 region solving 阶段验证。

#### 7.3 NLL 用集合包含和约束传播求解

MIR borrowck 中的 [`OutlivesConstraint`](../../../compiler/rustc_borrowck/src/constraints/mod.rs) 明确保存：

```rust,ignore
pub struct OutlivesConstraint<'tcx> {
    /// SUP must outlive SUB.
    pub sup: RegionVid,
    pub sub: RegionVid,
    // locations、span、category 等诊断信息……
}
```

所以 `'a: 'b` 对应：

```text
OutlivesConstraint {
    sup: 'a,
    sub: 'b,
}
```

NLL 把 region value 看作一组 CFG points、universal-region end elements 和 placeholders。约束要求：

```text
Value('b) subset-of Value('a)
```

[`RegionInferenceContext::propagate_constraints`](../../../compiler/rustc_borrowck/src/region_infer/mod.rs)
先对约束图计算 SCC，再按依赖顺序把 `sub` 的元素传播到 `sup`，得到满足所有 inclusion constraints 的最小闭包。

“传播完成”仍不自动等于程序合法，因为 universal region 是调用者选择的，函数体不能凭空增加它们之间的关系。随后 `check_universal_regions` 检查传播结果要求的 free-region 关系是否已由函数签名和 `where` clauses 保证。

例如：

```rust
fn ok<'a, 'b>(x: &'a u32) -> &'b u32
where
    'a: 'b,
{
    x
}
```

返回 `x` 产生 `'a: 'b`；`where 'a: 'b` 已把该关系加入 universal-region facts，因此检查通过。

省略这个 bound：

```rust,compile_fail
fn bad<'a, 'b>(x: &'a u32) -> &'b u32 {
    x
}
```

函数体仍产生相同 constraint，但调用者没有承诺 `'a: 'b`。传播会暴露一个未被签名允许的 universal-region relation，最终产生 lifetime error。

因此，“证明 `'a: 'b`”可以拆成：

```text
引用/谓词关系
  -> 注册 'b <= 'a
  -> 转换成 OutlivesConstraint { sup: 'a, sub: 'b }
  -> SCC + 集合传播求最小解
  -> 对 universal regions 检查该关系是否来自已知 bounds
  -> 满足则成立；缺少依据则报告错误或向外层 closure 传播 requirement
```

### 8. Snapshot 让推理可以安全试探

设 `?T0` 当前未知。执行：

```text
probe {
    ?T0 == u32
    创建 ?T1
    返回“这条路径可行”
}
```

闭包内部能观察到 `?T0 -> u32` 和 `?T1`；离开 `probe` 后，binding 被撤销，`?T1` 也随表长度回滚。

`commit_if_ok` 适合原子操作：

```text
Ok(result)  -> 保留本次所有推理副作用
Err(error)  -> 恢复到进入闭包前
```

它们不会复制整个 `InferCtxt`，而是记录并逆放变化，因此适用于高频候选尝试。

### 9. 选择正确的解析级别

假设：

```text
?T0 -> Known(u32)
value = Vec<?T0>
```

则：

```text
shallow_resolve(value)          = Vec<?T0>
resolve_vars_if_possible(value) = Vec<u32>
```

前者看到最外层是 `Vec`，便不进入参数；后者通过 `TypeFolder` 遍历参数。

`fully_resolve` 用于要求结果已经完整的阶段。它把“尚无解”视为错误，并要求 region resolution 已完成。读取调试信息时经常适合 opportunistic resolution；进入 writeback 等边界时才需要 full resolution。

#### 9.1 `fully_resolve` 所要求的 region 阶段何时发生

`InferCtxt::fully_resolve` 对 region 的具体依赖是：

```text
InferCtxt.lexical_region_resolutions
    必须已经是 Some(LexicalRegionResolutions)
```

其 [`FullTypeResolver::try_fold_region`](../../../compiler/rustc_infer/src/infer/resolve.rs)
只对 `ReVar` 查这张表：

```rust,ignore
match r.kind() {
    ReVar(_) => self
        .infcx
        .lexical_region_resolutions
        .borrow()
        .as_ref()
        .expect("region resolution not performed")
        .resolve_region(self.infcx.tcx, r),
    _ => r,
}
```

`ReEarlyParam`、`ReLateParam`、`RePlaceholder`、`ReStatic`、`ReErased`
不是待求解的 `ReVar`，会保持原表示。

##### Lexical/non-body analysis 路线的时间线

这条路线常见于 impl item 比较、specialization/WF 检查和某些 trait analysis：

```text
阶段 A：类型/trait 推理进行中
    创建 ReVar
    关系检查调用 make_subregion/make_eqregion
    收集 region constraints
    暂存 T: 'a 形式的 type-outlives obligations

阶段 B：完成主要类型与 trait obligations
    evaluate/select obligations
    尽量确定 type/const inference vars
    为处理 T: 'a 准备可归一化的 T

阶段 C：显式调用 resolve_regions...
    ObligationCtxt::resolve_regions_and_report_errors
      -> InferCtxt::resolve_regions
      -> resolve_regions_with_normalize

阶段 D：处理 type-outlives obligations
    深层 normalize T
    根据 T 的组成和 ParamEnv/implied bounds
    转成 region constraints 或 verify requirements

阶段 E：关闭 constraint collection
    从 InferCtxtInner 中 take region_constraint_storage
    此后不再进行会新增 region constraints 的统一操作

阶段 F：lexical_region_resolve::resolve
    每个 RegionVid 初始化为 Empty(universe)
    expansion 按 lower bounds / VarSubVar 传播并取 LUB
    检查 upper bounds、concrete relations、universe/nameability
    生成 errors 和 LexicalRegionResolutions

阶段 G：保存结果
    infcx.lexical_region_resolutions =
        Some(LexicalRegionResolutions { values })

阶段 H：调用 fully_resolve
    type/const var -> 读取各自 unification table
    ReVar          -> 读取 LexicalRegionResolutions
```

阶段 C 通常出现在“所有普通 obligations 已经处理完、即将把临时推理结果交给后续逻辑”的边界。
例如 impl method 与 trait method 比较中，当前源码先：

```text
evaluate_obligations_error_on_ambiguity
    -> resolve_regions_and_report_errors
    -> fully_resolve(collected type)
```

`resolve_regions_and_report_errors` 消费 `ObligationCtxt`，因为 region constraint storage
已被关闭，之后继续 trait solving 可能产生无法再接收的 region constraints。

##### `fully_resolve` 调用时的三种 region 结果

1. 输入没有 `ReVar`：

   ```text
   &'a T，其中 'a = ReEarlyParam / ReLateParam
   ```

   无需查询 lexical resolution，region 原样保留。

2. 输入含 `ReVar`，且 lexical resolution 已经产生具体 `VarValue::Value(r)`：

   ```text
   ReVar(?r0) -> ReLateParam('a) / ReStatic / RePlaceholder(...)
   ```

   fold 后使用该具体 region。

3. 输入含 `ReVar`，但前置阶段不完整：

   - `lexical_region_resolutions == None`：`expect("region resolution not performed")`，
     表示调用顺序违反 API 前置条件；
   - resolution 中该变量仍为 `Empty`：`resolve_region` 会暂时保留原 `ReVar`，
     外层 `InferCtxt::fully_resolve` 随后检测到 `has_infer_regions()`，记录 delayed bug
     并把残留 `ReVar` 换成 error region；
   - 已标为 `ErrorValue`：lexical resolver 用恢复值继续，已有 region error
     由前面的 reporting 阶段负责。

Type/const 的未决变量通常通过 `FixupError` 返回；region 的“尚未运行 solver”属于调用顺序错误，
所以当前实现使用 `expect`，两者失败通道并不相同。

##### 标准函数体的 NLL 路线

普通函数 body 的当前管线不是：

```text
HIR typeck -> lexical resolve -> InferCtxt::fully_resolve regions
```

HIR writeback 在 [`rustc_hir_typeck/src/writeback.rs`](../../../compiler/rustc_hir_typeck/src/writeback.rs)
中先 opportunistically resolve type/const vars，然后有意把非 bound regions 折叠成 `ReErased`：

```rust,ignore
value = fold_regions(tcx, value, |_, _| tcx.lifetimes.re_erased);
```

原因是 HIR 与 MIR region 没有简单的一一对应。随后：

```text
构造 MIR
  -> MIR typeck 创建新的 RegionVid 并收集 location-sensitive constraints
  -> canonical type operations 返回 QueryRegionConstraints
  -> ConstraintConversion 转成 MIR OutlivesConstraint
  -> borrowck::compute_regions
  -> RegionInferenceContext::new
  -> RegionInferenceContext::solve
       liveness seeds
       SCC / constraint propagation
       type tests
       universal-region checks
```

NLL 的结果保存在 borrowck 的 `RegionInferenceContext` 中，不写入
`InferCtxt.lexical_region_resolutions`。因此，`InferCtxt::fully_resolve` 的 region 分支
不是读取 NLL 解的通用 API。

可以把两条路线压缩为：

```text
non-body / lexical analysis：
    collect -> resolve_regions -> LexicalRegionResolutions -> fully_resolve

function body / NLL borrowck：
    HIR writeback erase regions
      -> MIR typeck recollect
      -> RegionInferenceContext::solve
      -> borrowck consumers read regioncx
```

#### 9.2 决定走哪条路线的不是 `ReVar`，而是消费者

`ReVar(RegionVid)` 只表示“这个 `RegionVid` 是当前推理上下文中的 region
推理变量”。它没有携带 `lexical` 或 `NLL` 标签，也不会自行选择 solver。分界来自创建它的
`InferCtxt` 以及消费者要把结果交到哪里。

**需要保留一个可继续使用的、非推理 region 结果时，走 lexical resolution：**

```text
非函数体或 item-level analysis
  -> 在临时 InferCtxt 中实例化/关系检查
  -> 收集 region constraints
  -> resolve_regions
  -> LexicalRegionResolutions
  -> fully_resolve(value)
  -> 把不含 ReVar 的 value 交给后续分析
```

典型调用点包括 impl item 与 trait item 的签名比较、coherence、
specialization/WF 检查，以及某些 predicate normalization。这里没有 MIR
控制流位置可供求解；消费者关心的是签名、泛型参数或 predicate 之间的 outlives
关系。以 impl method 比较为例，当前实现先调用
`resolve_regions_and_report_errors`，随后才对收集的类型调用 `fully_resolve`。

**函数体中的借用有效范围需要依赖 MIR 位置时，走擦除后重建的 NLL 路线：**

```text
HIR typeck 的 InferCtxt
  ReVar(HIR RegionVid)
  -> writeback 把 region 折叠为 ReErased
  -> 构造 MIR
  -> borrowck 创建新的 BorrowckInferCtxt
  -> replace_regions_in_mir / renumber_mir
  -> ReVar(MIR RegionVid)
  -> MIR typeck 生成 outlives + liveness + location constraints
  -> RegionInferenceContext::solve
```

因此，“原来的 `ReVar` 被交给 NLL”并不准确。HIR typeck 的 `RegionVid` 已在
writeback 边界消失；NLL 使用的是 borrowck 上下文中重新编号的一套 `RegionVid`。
二者恰好都用 `RegionKind::ReVar` 表示，但 ID 的所有者、约束域和求解结果不同：

| 路线 | 变量属于 | 约束是否包含 MIR 位置 | 结果保存位置 |
|---|---|---:|---|
| lexical + `fully_resolve` | 临时 type/trait-analysis `InferCtxt` | 否 | `LexicalRegionResolutions`，随后 fold 回 Type IR |
| MIR NLL | `BorrowckInferCtxt` | 是 | borrowck 的 `RegionInferenceContext` |

一个实用判断是：

```text
需要回答“这个临时推理结果中的 ?r 应当替换成哪个可输出 region”？
  -> resolve_regions + fully_resolve

需要回答“这个 borrow 在 MIR 的哪些控制流点有效，是否覆盖某次使用”？
  -> 擦除 HIR region，MIR 中重新创建 ReVar，再由 NLL 求解
```

对普通函数体局部变量，例如 `let x = &local`，真正的借用范围属于第二类。
对 impl 签名是否满足 trait 签名这类 item-level 关系检查，则属于第一类。

#### 9.3 `ReVar` 不是“body lifetime”的专用表示

源码里写出的 lifetime 与编译器创建的推理变量不是一回事：

```text
源码声明的 'a
  -> 通常表示为 ReEarlyParam / ReBound
  -> 进入具体检查作用域后也可能被 liberate 为 ReLateParam

编译器暂时不知道的某个 region
  -> 在当前 InferCtxt 中创建 ReVar(RegionVid)
```

因此，`ReVar` 更像局部编号 `tmp0`：仅凭 `tmp0` 这个表示，无法判断它属于函数体借用检查、
impl/trait 签名比较，还是另一次临时 trait analysis；必须先看是谁创建了它。

以函数体为例：

```rust,ignore
fn use_ref<'a>(x: &'a i32) {
    let y = &*x;
    consume(y);
}
```

可以建立以下简化模型：

```text
HIR typeck：
  参数中的 'a        -> 已声明的参数 region，不是“等待猜测的 body ReVar”
  对 &*x 的 reborrow -> 创建临时 ?h0 = ReVar(HIR RegionVid)
  得到类似 ?h0 <= 'a 的关系

HIR writeback：
  &'?h0 i32 -> &'erased i32

MIR borrowck：
  签名 'a      -> 新的 universal RegionVid ?m0
  reborrow     -> 新的 existential RegionVid ?m1
  根据 MIR point、liveness 和 outlives constraints 求解 ?m1
```

这里擦除 `?h0` 不是放弃检查，而是把“借用在哪些控制流点有效”交给拥有完整 MIR
信息的 NLL 重新建模。`?h0` 与 `?m1` 不要求具有相同编号或一一对应关系。

再看没有函数体控制流参与的 item-level 检查：

```rust,ignore
trait Tr {
    fn get<'a>(&'a self) -> &'a u32;
}

impl Tr for S {
    fn get<'b>(&'b self) -> &'b u32 { /* ... */ }
}
```

比较 trait signature 与 impl signature 时，rustc 会打开 binder、实例化参数，并可能在这个
临时 `InferCtxt` 中创建 `ReVar`。这些变量表达的是“两个签名之间的 region 关系”，不是
“borrow 在 body 的哪些位置活跃”。该检查必须在返回前给出确定结论，无法等某个 MIR body
替它求解，所以执行：

```text
收集签名关系约束
  -> resolve_regions
  -> fully_resolve 临时结果
```

由此得到更直接的分类：

```text
ReVar 来自普通 body 的 HIR FnCtxt，问题依赖控制流位置
  -> writeback 擦除
  -> MIR borrowck 创建另一批 ReVar
  -> NLL 求解

ReVar 来自独立的签名/trait/item 分析 InferCtxt，
并且该分析必须返回已确定的 Type IR
  -> 在同一个 InferCtxt 中 resolve_regions
  -> fully_resolve
```

#### 9.4 两个例子中的具体求解结果

对函数体：

```rust,ignore
fn use_ref<'a>(x: &'a i32) {
    let y = &*x;
    consume(y);
}
```

当前 rustc 在 MIR renumber 后会得到多于一个 `RegionVid`。编号不属于稳定接口，但一次实际
dump 可以概括为：

```text
?1 = universal region 'a
?5 = 参数 x 的引用类型中的 region
?3 = 创建 y 时 reborrow expression 的 region
?6 = local y 的类型中的 region
?4 = 把 y 传给 consume 时再次 reborrow 的 region
?7 = 调用参数临时值类型中的 region
?8 = consume 的 late-bound 参数实例化得到的 region
```

关键约束形成一条 outlives 链：

```text
'a = ?1 = ?5
?5 : ?3
?3 : ?6
?6 : ?4
?4 : ?7
?7 : ?8
```

其中 `r1 : r2` 表示 `r1` outlives `r2`。NLL 先用变量/类型 liveness 给每个 region
放入必须包含的 MIR points，再沿上述 outlives 边传播集合。该例一次实际求解的核心结果是：

```text
?1 ('a) = 整个函数体的 points，并带有 universal end element
?5      = 与 ?1 相同
?3      = 从创建 y 的 reborrow 到 consume 调用完成
?6      = 从 y 完成赋值后到 consume 调用完成
?4      = 从调用前对 y 的 reborrow 到调用完成
?7/?8   = consume 调用点
```

用简化的 point 编号表示：

```text
?1 = ?5 ⊇ ?3 ⊇ ?6 ⊇ ?4 ⊇ ?7 = ?8

?3 = {create-y, ..., call}
?6 = {after-assign-y, ..., call}
?4 = {reborrow-for-call, call}
?7 = ?8 = {call}
```

因此，函数体例子的“region resolve 结果”不是：

```text
?3 -> 某个新的名字 'b
```

而是：

```text
RegionVid ?3 -> 一组 MIR points
```

并据此判断 borrow 是否在每次使用点有效、是否与写入或移动冲突。

对 trait/impl signature：

```rust,ignore
trait Tr {
    fn get<'a>(&'a self) -> &'a u32;
}

impl Tr for S {
    fn get<'b>(&'b self) -> &'b u32 { /* ... */ }
}
```

可以把当前检查简化为：

```text
trait 的 late-bound 'a
  -> liberate 为一个有稳定身份的 ReLateParam(A)

impl 的 late-bound 'b
  -> instantiate_binder_with_fresh_vars
  -> ReVar(?i)
```

函数输入的逆变位置与输出的协变位置共同产生两个方向的 region 关系：

```text
A <= ?i
?i <= A
```

于是 lexical region solver 得到：

```text
?i = A
```

若消费者随后对签名执行 `fully_resolve`，概念上的结果为：

```text
fn(&'?i S) -> &'?i u32
    ↓
fn(&'A S) -> &'A u32
```

这里的 `A` 是已 liberate 的参数 region，不是 MIR point 集合。当前简单的 method
compatibility 路径主要使用 `resolve_regions` 检查约束是否有错误；只有确实要保留临时
Type IR 的消费者，才需要继续调用 `fully_resolve` 把 `?i` fold 掉。

#### 9.5 为什么 trait 被 liberate，而 impl 被实例化成 fresh vars

method compatibility 检查的方向是：

```text
impl_fty <: trait_fty
```

即 impl 提供的函数必须能用在 trait 契约所要求的位置。对高阶函数子类型：

```text
for<I> Impl(I) <: for<A> Trait(A)
```

正确的证明顺序是：

```text
对每一个调用者可能选择的 trait lifetime A：
    必须能从 impl 的 forall<I> 中选择一个实例 I，
    使 Impl(I) <: Trait(A)
```

逻辑形式为：

```text
forall A. exists I. Impl(I) <: Trait(A)
```

因此两侧的处理有意不对称：

```text
trait/super/expected 一侧：
  A 必须代表任意、固定、不可由当前关系检查挑选的 lifetime
  -> 当前 method comparison 将其 liberate 为 ReLateParam(A)

impl/sub/actual 一侧：
  impl 本身承诺可用于所有 I
  -> 对当前这个任意 A，可以合法地选择一次 impl 实例
  -> instantiate_binder_with_fresh_vars 得到 ReVar(?i)
```

在简单的 `get` 签名中，输入位置的逆变和输出位置的协变分别产生相反方向的 region
约束，最终迫使：

```text
?i = A
```

这不是把 impl 的 universal lifetime 降格成“只要存在一个 lifetime 就行”。外层的
`A` 是任意选择的；对每个 `A`，impl 的 `for<I>` 都允许重新实例化一次，所以整体仍覆盖
所有 lifetime。

如果反过来把 trait lifetime 做成可推断的 `ReVar`，检查可能只挑一个方便的 trait
实例，例如 `'static`：

```text
exists A. actual <: Trait(A)
```

这只能证明 impl 满足 trait 的某一个 lifetime 实例，无法证明它满足调用者可能选择的
所有实例。典型错误形状是：

```text
trait 要求：for<'a> fn(&'a T) -> &'a T
impl 只能：      fn(&'static T) -> &'static T
```

若 expected `'a` 可被推成 `'static`，这个错误实现可能被误接受；把 trait 侧保持为任意的
刚性 region 会使检查失败。

这里使用 `liberate_late_bound_regions` 而不是一般关系算法中的 placeholder，是当前
impl-method 检查的具体组织方式：它把 trait binder 中的 `ReBound` 替换成附着于
`impl_m.def_id` 的 `ReLateParam`，使这个任意 lifetime 能作为当前 method scope 中的稳定
自由 region 参与 implied-bounds、WF 和 region checking。它仍然是刚性的，不是可赋值的
`ReVar`。

### 10. 一次关系检查的完整追踪模板

以后阅读推理源码时，可以按以下顺序定位：

```text
1. 输入 Type IR 中有哪些 Infer / ReVar 节点？
2. 每个变量创建于哪个 InferCtxt、哪个 universe？
3. 调用的是 eq、sub、coerce，还是更高层 API？
4. shallow resolve 后，两侧属于变量/结构/alias 的哪种组合？
5. 状态变化写入哪张 table，或产生哪类 constraint/obligation？
6. 当前是否处于 snapshot；结果会 commit 还是 rollback？
7. 消费者使用 shallow、opportunistic 还是 full resolution？
```

这七步会贯穿后续 variance、obligation、normalization、trait solver 和 canonicalization。

## 常见误区

### 误区一：`Ty` 节点会被统一操作原地修改

`Ty` 是 interned handle。统一修改的是 `InferCtxt` 的会话状态；解析操作读取状态并返回新的/已有的 interned `Ty`。

### 误区二：所有 inference vars 都使用同一张表

通用类型、整数、浮点、常量和 region 有各自的存储与约束规则。共享的是“IR ID 间接指向推理状态”的总体模型。

### 误区三：`?T0 == ?T1` 立刻选定一个具体类型

这一步通常只合并等价类。具体类型可以在之后由任一成员收到的约束确定。

### 误区四：`probe` 成功意味着副作用已经保留

`probe` 返回闭包的观察结果，但始终回滚推理副作用。需要成功时保留状态的事务使用 `commit_if_ok`。

### 误区五：`resolve_vars_if_possible` 会完成 region solving

它深层替换已有解的 type/const vars，并保留 region vars。完整 region 求解有独立阶段。

## 本章小结

- inference variable 的 Type IR 节点是 ID handle，可变解存放在 `InferCtxt`。
- 通用类型变量的 equality state 是 union-find 等价类加 `Unknown(U)` / `Known(Ty)`。
- variable-variable 使用 union；variable-value 经关系检查、generalization 和 occurs check 后实例化。
- 不同 universe 的 unknown classes 合并时取较小 universe，以维护共同 nameability。
- relation API 还负责结构递归、variance、constraints 与 obligations，不能简化为直接写表。
- snapshot 用 undo log 提供 `probe` 的必回滚语义和 `commit_if_ok` 的成功提交语义。
- shallow resolution 只看外层，opportunistic resolution 深入 type/const 结构，full resolution 要求推理已经收尾。
