---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "04"
document: content
status: completed
updated_at: 2026-07-26
---

# 04. Region、Universe 与 Placeholder

## 学习目标

完成本章后，应当能够：

1. 区分 `ReEarlyParam`、`ReBound`、`ReLateParam`、`ReVar`、`RePlaceholder` 与 `ReErased` 的阶段和作用域。
2. 解释函数 lifetime parameter 何时 early-bound、何时 late-bound。
3. 追踪 late-bound region 从签名中的 `ReBound` 到函数体中的 `ReLateParam`。
4. 解释 `UniverseIndex::can_name` 表达的可见性关系。
5. 说明 placeholder 为什么是刚性代表，而不是可赋值的 inference variable。
6. 手算 `enter_forall` 后的 placeholder universe，以及不同 universe 中 inference vars 的可命名范围。
7. 用 universe 可见性解释 higher-ranked 检查中的 placeholder escape 与 leak check。

## 前置知识

- 第 01 章中 `Region`、`RegionKind`、`Param`、`Bound`、`Placeholder`、`Infer` 的基本分类。
- 第 02 章中 `Binder<T>`、`BoundVar`、de Bruijn index 与 binder 实例化。
- 第 03 章中 `EarlyBinder<T>`、item 参数 index 与 `ReEarlyParam` substitution。

## 核心心智模型

一个源码 lifetime 会随着检查阶段改变表示：

```text
item 参数，参与 GenericArgs
  'a -> ReEarlyParam(index)

higher-ranked / late-bound 签名，仍在 Binder 内
  'a -> ReBound(DebruijnIndex, BoundVar)

进入函数体，固定为该 body 的自由参数
  ReBound -> ReLateParam(scope, kind)

临时打开 forall，做 higher-ranked 推理
  ReBound -> RePlaceholder(universe, bound-var identity)

调用点或推理过程中的未知 region
  '_ -> ReVar(RegionVid)
```

这里有两条互相独立的坐标轴：

```text
Binder / de Bruijn index：变量由哪一层量词绑定？
Universe / can_name：打开量词后，哪些推理变量有资格引用新名字？
```

Universe index 不是 region 长短，也不是 de Bruijn index。

## 源码地图

| 主题 | 主要路径与符号 |
|---|---|
| region variants 与语义说明 | `compiler/rustc_type_ir/src/region_kind.rs`：`RegionKind` |
| universe 可见性 | `compiler/rustc_type_ir/src/lib.rs`：`UniverseIndex`、`can_name` |
| placeholder 身份 | `compiler/rustc_type_ir/src/binder.rs`：`Placeholder`、`PlaceholderRegion` |
| region constructors 与数据 | `compiler/rustc_middle/src/ty/region.rs`：`EarlyParamRegion`、`LateParamRegion` |
| early/late 分类 | `compiler/rustc_hir_analysis/src/collect/resolve_bound_vars.rs`：`is_late_bound_map` |
| body 中 liberate | `compiler/rustc_middle/src/ty/fold.rs`：`liberate_late_bound_regions` |
| 打开 `forall` | `compiler/rustc_infer/src/infer/relate/higher_ranked.rs`：`enter_forall` |
| 通用 placeholder replacement | `compiler/rustc_next_trait_solver/src/placeholder.rs`：`BoundVarReplacer` |
| region inference universe | `compiler/rustc_infer/src/infer/mod.rs`：`next_region_var_in_universe` |
| leak check | `compiler/rustc_infer/src/infer/region_constraints/leak_check.rs` |

源码引用以路径和符号名为准；行号随 rustc 演进可能变化。

## 正文

### 1. RegionKind 是阶段敏感的表示

[`RegionKind`](../../../compiler/rustc_type_ir/src/region_kind.rs) 的主要 variants：

| variant | 典型含义 | 主要阶段 |
|---|---|---|
| `ReEarlyParam` | item 参数表中的 lifetime slot | 定义端、item 实例化、函数体 |
| `ReBound` | 仍由某个 `Binder` 管理的 region | poly fn signature、HRTB |
| `ReLateParam` | 已从函数签名 binder 中 liberate、固定属于当前 body 的参数 | body typeck |
| `ReStatic` | `'static` | 各阶段 |
| `ReVar` | region inference variable | inference；NLL 中也用作内部索引 |
| `RePlaceholder` | 打开 `forall` 后的刚性代表 | higher-ranked inference |
| `ReErased` | region 细节已不再需要 | trait selection 的部分路径、MIR、codegen |
| `ReError` | 已有诊断后的恢复表示 | error recovery |

同一个源码名字不保证始终对应同一个 variant；要同时看“由谁绑定”和“当前正在执行什么检查”。

### 2. `ReEarlyParam`：完整 GenericArgs 中的 lifetime slot

非函数 item 上声明的 lifetime 都是 early-bound：

```rust
struct Hold<'a, T>(&'a T);
```

其定义内部近似为：

```text
Hold::Generics = ['a#0, T#1]
field type     = &ReEarlyParam('a#0) Param(T#1)
```

实例化 `Hold<'x, u32>` 时，第 03 章的 `EarlyBinder::instantiate` 使用完整 args：

```text
['x, u32]
```

把 `ReEarlyParam('a#0)` 替换成 `'x`。因此 `ReEarlyParam` 通过 item 参数绝对 index 定位，不使用 de Bruijn index。

### 3. 函数 lifetime 参数可能 early-bound，也可能 late-bound

对函数和方法，rustc 会根据 lifetime 的使用位置分类。当前 `is_late_bound_map` 的核心规则可概括为：

- 出现在输入类型中；
- 不出现在 where clauses / 参数 bounds 中；
- 不是只在输出中出现；

满足这些条件的 lifetime 可以作为 late-bound 参数进入函数签名 binder。

典型 late-bound 例子：

```rust
fn borrow<'a>(x: &'a u32) -> &'a u32;
```

签名近似为：

```text
EarlyBinder<
  Binder<for<'a> fn(&ReBound(D0, 'a) u32) -> &ReBound(D0, 'a) u32>
>
```

`'a` 不在 item `Generics::own_params` 中，而在 `PolyFnSig` 的真正 `Binder` 中。

典型 early-bound 例子：

```rust
fn constrained<'a: 'static>(x: &'a u32);
```

`'a` 出现在参数 bounds 中，因此属于 item generics，用 `ReEarlyParam` 表示。另一个常见 early-bound 形态是 lifetime 不出现在任何输入、但出现在返回类型中。

#### 3.1 本质区别：量词位于 item 外层还是函数签名内层

两者都具有“对 lifetime 泛型”的含义，但量词位置不同。

Early-bound lifetime 近似为：

```text
forall<'a> {
    item::<'a> has type fn(... ReEarlyParam('a) ...)
}
```

也就是先为 item 的 lifetime slot 选择一个实参，再取得该实例下的签名。Type IR 结构近似为：

```text
EarlyBinder<'a, Binder<FnSig containing ReEarlyParam('a)>>
```

Late-bound lifetime 近似为：

```text
item has type for<'a> fn(... ReBound('a) ...)
```

它不是 item `GenericArgs` 中的 slot，而是 `PolyFnSig` 自己的 binder slot。每次使用该签名时，都可以按当前关系检查或调用重新实例化这个 binder。

因此可以把区别压缩为：

| 维度 | Early-bound lifetime | Late-bound lifetime |
|---|---|---|
| 量词位置 | item 泛型层 | 函数签名 `Binder` 内 |
| 定义端表示 | `ReEarlyParam(index)` | `ReBound(debruijn, slot)` |
| 参数表 | 位于 `Generics` / `GenericArgs` | 位于 `Binder::bound_vars` |
| 何时实例化 | 实例化 item 时 | 打开或调用 poly signature 时 |
| 进入函数体 | 保持 `ReEarlyParam` | liberate 为 `ReLateParam` |
| 典型用途 | lifetime 参与 item predicates，或由调用者选择输出 lifetime | lifetime 由输入约束，并在每次调用中独立选择 |

#### 3.2 为什么不能把所有函数 lifetime 都设为 early-bound

考虑：

```rust
fn id<'a>(x: &'a u32) -> &'a u32 { x }
```

它自然具有：

```text
for<'a> fn(&'a u32) -> &'a u32
```

调用者可以先借用一个局部值，再为这一次调用选择与该 borrow 对应的 `'a`。下一次调用可以选择另一个 lifetime。把 `'a` 保留在签名 binder 中，直接表达了“这个函数对任意一次调用选择的 `'a` 都成立”。

若一律把它放入 item args，函数签名就不再直接携带 `for<'a>` 的 higher-ranked 性质；函数指针 coercion、higher-ranked subtyping 和 trait relation 都还需要重新恢复这层量化信息。因此，对只受输入约束的 lifetime 使用 late binding，能让 poly signature 精确保存其真实语义。

#### 3.3 为什么也不能把所有函数 lifetime 都设为 late-bound

考虑只有输出使用 lifetime 的函数：

```rust
fn make<'a>() -> &'a u32 { todo!() }
```

源码语义是调用者先选择某个 `'a`，函数返回一个至少对该 `'a` 有效的引用。其近似结构是：

```text
item::<'a> -> fn() -> &'a u32
```

若把 `'a` 放入签名 binder，会变成更强的：

```text
for<'a> fn() -> &'a u32
```

这表示同一个函数值必须能够对任意后来选择的 `'a` 返回结果，量词位置已经改变。因此，只在输出出现而没有被输入约束的 lifetime 必须作为 early item 参数。

同理，当 lifetime 出现在：

```rust
where T: 'a
```

这样的 item predicate 中时，predicate 必须与 item 的其他 args 一起实例化并在查询间传递，所以 `'a` 需要成为 `Generics` / `GenericArgs` 中的 early slot。显式写在 predicate 内部的 `for<'a>` 则是另一个真正的 higher-ranked binder。

#### 3.4 为什么函数体中又要把 late lifetime liberate

函数签名对外必须保留 `for<'a>`，但函数体只检查一次。在检查 `id` 的 body 时，rustc 选择一个固定但抽象的 body parameter 代表 `'a`：

```text
对外签名：ReBound(D0, 'a)
进入 body：ReLateParam(scope = id, kind = Named('a))
```

这样 body 内所有 `'a` occurrence 都稳定指向同一个参数，并能参与 body 的 outlives 约束；同时，对外保存的 `PolyFnSig` 仍然是 universally quantified。区分 early/late，再在 body 边界执行 liberate，正好同时满足“对外可重复实例化”和“对内固定检查一次”这两个需求。

#### 3.5 `late` 表示延迟绑定层级，不表示运行时绑定

Early/late 的命名描述的是相对于 item 实例化的绑定位置：

```text
early：实例化 item GenericArgs 时确定
late：保留在函数签名 Binder 中，等签名被使用或打开时再实例化
```

所有这些步骤都发生在编译期。一次具体调用通常会让类型检查器用当前调用产生的 region inference vars 实例化 late-bound regions，但“签名被使用”还包括：

- 函数项到函数指针的 coercion；
- 两个 higher-ranked function types 的 subtyping / equality relation；
- trait solving 中检查 HRTB；
- `enter_forall` 打开 binder 并创建 placeholders。

因此，“late 到调用时”适合作为初步直觉；更精确的说法是：

> late-bound lifetime 不在 item args 阶段确定，而是保持 universally quantified，直到某个编译期操作需要实例化或打开它所在的函数签名 binder。

另外，检查函数自身的 body 时，rustc 会预先把它 liberate 为 `ReLateParam`。这仍然是编译器检查定义的过程，并不表示每次运行时调用都会执行一次 lifetime substitution。

#### 3.6 Function-item type 解释了为什么不能统一使用 late binding

函数名在表达式中不是立即变成普通函数指针，而是产生一个零大小的 function-item value。以：

```rust
fn foo<'a, T>(x: &'a T) -> &'a T { x }
```

为例，它的概念性展开近似为：

```rust,ignore
struct FooFnItem<T>;

impl<'a, T> Fn<(&'a T,)> for FooFnItem<T> {
    type Output = &'a T;
}
```

这揭示了两个实例化阶段：

```text
命名函数：foo::<T>     -> 确定 function-item type 的 early args
调用函数：f(&value)    -> 为 Fn impl 的 late lifetime 选择调用期实参
```

若 lifetime 是 early-bound，它会进入 function-item type 本身：

```rust,ignore
struct FooFnItem<'a, T>;

impl<'a, T> Fn<(&'a T,)> for FooFnItem<'a, T> {
    type Output = &'a T;
}
```

此时一个已经命名出来的 `FooFnItem<'x, T>` 只能用与 `'x` 兼容的 borrow 调用；late 版本的 `FooFnItem<T>` 则能在每次调用时选择不同 lifetime，并可满足 `for<'a> Fn(&'a T) -> &'a T`。

统一使用 late binding 会遇到两个结构性问题：

1. **依赖 lifetime 的 item predicate 无处保存。** 对：

   ```rust
   fn check<'a, T: Trait<'a>>(x: &'a T) {}
   ```

   若 `'a` 到调用时才实例化，那么命名 `check::<T>` 时还不能证明 `T: Trait<'a>`。函数项 coercion 成函数指针后，普通 function-pointer type 也没有位置携带这个尚待证明的 item where-clause。让 `'a` early-bound，便可在命名/实例化 function item 时连同 predicate 一起检查。

2. **只出现在输出中的 lifetime 会让 builtin `Fn` impl 不受约束。** 若把：

   ```rust
   fn make<'a>() -> &'a String { todo!() }
   ```

   的 `'a` 设为 late，概念展开会成为：

   ```rust,ignore
   struct MakeFnItem;

   impl<'a> Fn<()> for MakeFnItem {
       type Output = &'a String;
   }
   ```

   `'a` 没有出现在 self type `MakeFnItem` 或输入 `()` 中，是一个不受约束的 impl 参数。将其设为 early 后，self type 变成 `MakeFnItem<'a>`，参数便有了稳定身份。

所以，区别并非由“编译期还是运行期”造成，而是由 function-item type 与 `Fn` 调用 impl 之间的量词位置造成。理论上可以设计另一套携带所有 predicates 和 binders 的 IR，但仍必须表示这两层不同量化；不能在保持当前 Rust 语义的同时把所有 lifetime 简单改成同一种 late-bound slot。

### 4. `ReBound`：仍未打开的量化变量

`ReBound` 的完整身份为：

```text
ReBound(BoundVarIndexKind::Bound(DebruijnIndex), BoundRegion)
```

它只在 binder 语境下有意义。对于：

```rust
for<'a> fn(&'a u32)
```

`'a` 在签名内是 `ReBound(D0, slot0)`。若它出现在更深一层 binder 内，则 de Bruijn index 相应增加。

`ReBound` 不是 region lattice 中可以直接参与普通 inference 的自由 region；使用前要按目标操作进行实例化。

### 5. `ReLateParam`：进入函数体后的固定参数

检查 `borrow` 的函数体时，需要让签名中的 `'a` 成为 body 内可以反复引用的固定 region。rustc 调用：

```text
tcx.liberate_late_bound_regions(fn_def_id, poly_sig)
```

把目标 binder 的 `ReBound` 替换成：

```text
ReLateParam(LateParamRegion { scope: fn_def_id, kind })
```

于是生命周期为：

```text
签名 query：Binder<ReBound(D0, 'a)>
进入 body：ReLateParam(scope = borrow, 'a)
```

`ReLateParam` 在 body 内按参数 region 对待，能够结合声明的 outlives 关系和 free-region 信息使用。它不再依赖 de Bruijn index，也不是 `RePlaceholder`。

### 6. `ReVar`：可以被求解的 existential unknown

调用点省略 lifetime、创建 borrow 或执行 region relation 时，会出现：

```text
ReVar(RegionVid)
```

`RegionVid` 只是 Type IR 中的变量 ID；origin、约束、创建 universe 与求解状态存放在 inference / region constraint 数据结构中。

这与 placeholder 的量词方向相反：

```text
ReVar          存在某个 region，使约束成立；求解器要找它
RePlaceholder  任取一个新 region，约束都必须成立；它是固定测试代表
```

### 7. Universe 是名字可见性层级

[`UniverseIndex`](../../../compiler/rustc_type_ir/src/lib.rs) 从根 universe 开始：

```text
U0 = UniverseIndex::ROOT
U1 = U0.next_universe()
U2 = U1.next_universe()
```

每进入一层需要新名字的 `forall`，就创建扩展 universe。当前实现的 universe 按创建顺序形成层级：

```text
U0 names ⊆ U1 names ⊆ U2 names
```

`can_name` 的方向是：

```text
U2.can_name(U0) == true
U2.can_name(U1) == true
U1.can_name(U2) == false
U0.can_name(U1) == false
```

含义是“在当前 universe 中，能否合法引用另一个 universe 引入的名字”。数值更大不表示 region 更长，也不表示它 outlives 更小的 universe。

### 8. Placeholder 的身份是 `(universe, bound identity)`

通用结构近似为：

```rust
Placeholder {
    universe: UniverseIndex,
    bound: BoundRegion | BoundTy | BoundConst,
}
```

因此一个 region placeholder 的身份包含：

```text
(U1, 原 binder 的 slot0/'a)
```

同一 universe 中由不同 bound slots 产生的 placeholders 仍是不同的刚性名字；它们之间没有自动的相等或 outlives 关系。

Placeholder 看起来是“未知”，但它不能像 inference variable 一样被赋值。它代表 arbitrary-but-fixed：检查必须对这个任意选择的固定名字成立。

### 9. `enter_forall`：从 `ReBound` 到 `RePlaceholder`

考虑：

```text
Binder<for<'a> fn(&'a u32)>
```

在当前最大 universe 为 U0 时调用：

```text
infcx.enter_forall(value, |opened| { ... })
```

概念步骤为：

```text
1. create_next_universe() -> U1
2. 为 binder slot0 创建 PlaceholderRegion(U1, 'a)
3. 把该 binder 管理的 ReBound(D0, slot0) 替换为 RePlaceholder(U1, slot0)
4. 在 closure 中检查 opened value
```

结果近似为：

```text
fn(&RePlaceholder(U1, 'a) u32)
```

真正实现同时支持 bound types、regions 和 consts；新版通用 replacer 还维护 placeholder 到原 bound variable 的映射，以便需要时恢复 binder 形式。

### 10. Inference var 的创建 universe 限制其可选答案

本章使用的简写与真实 Type IR 对应如下：

```text
?r0@U0
  = ReVar(RegionVid(0))
  + InferCtxt 中记录 RegionVid(0) 创建于 U0

P1@U1
  = RePlaceholder(PlaceholderRegion {
        universe: U1,
        bound: 原 Binder 中的 BoundRegion identity,
    })
```

`?r0` 中的 `0` 是 inference variable ID；`P1` 只是讲义中“U1 中某个 placeholder”的便捷名称，不等同于 `RegionVid(1)`。更精确的 placeholder 记法可以写成：

```text
P('a, slot0)@U1
```

两者存储 universe 的位置也不同：

- `ReVar` variant 自身只携带 `RegionVid`，创建 universe、origin、约束和求解状态保存在 inference context / region constraint tables 中；
- `RePlaceholder` 内部的 `PlaceholderRegion` 直接携带 `universe` 和原 bound-variable identity。

假设：

```text
?r0 创建于 U0
进入 forall，创建 placeholder P1 于 U1
?r1 创建于 U1
再进入 forall，创建 placeholder P2 于 U2
?r2 创建于 U2
```

则 nameability 为：

| inference var | 可命名 P1(U1) | 可命名 P2(U2) |
|---|---:|---:|
| `?r0@U0` | 否 | 否 |
| `?r1@U1` | 是 | 否 |
| `?r2@U2` | 是 | 是 |

所以 outer inference var 不能通过“记住”后来打开的 arbitrary placeholder 来伪造 higher-ranked 证明。反过来，在新 universe 中创建的变量可以引用当前及外层已经可见的名字。

### 11. 为什么需要防止 placeholder escape

考虑抽象目标：

```text
对所有 'a，都要求某个外层未知 ?r0 与 'a 相同
```

若打开 `forall<'a>` 后得到 `P1@U1`，并允许：

```text
?r0@U0 := P1@U1
```

那么我们只为本次选择的 placeholder 找到了解，而没有证明同一个外层 `?r0` 能等于所有可能的 `'a`。这会把 U1 中的新名字带回 U0。

Universe rule 直接表达阻止条件：

```text
U0.cannot_name(U1) == true
```

因此 `?r0@U0` 不能把 `P1@U1` 作为解。

### 12. Leak check 检查的是量词作用域泄漏

这里的 leak 与内存泄漏无关；它指 fresh placeholder 通过约束逃出其允许的 universe。

当前显式 leak checker 会围绕新 universe 中的 placeholders 检查 region constraint graph。实现会构建 outlives 图、计算 SCC，并重点识别：

```text
P1 与另一个不同 placeholder 被迫相等
P1 被迫流入一个不能命名 P1 的 universe
```

典型失败形态是：

```text
?r0@U0 = P1@U1
```

因为 U0 不能命名 P1。当前 rustc 已把 universe 信息整合进 region solvers；显式 `leak_check` 仍用于兼容行为及若干需要较早判定的路径。

### 13. `ReLateParam` 与 `RePlaceholder` 的边界

两者都可视为把 binder 中的变量变成某种固定代表，但使用场景不同：

| 维度 | `ReLateParam` | `RePlaceholder` |
|---|---|---|
| 场景 | 检查某个函数自己的 body | 临时打开 higher-ranked `forall` |
| 身份 | `(scope, kind)` | `(universe, bound identity)` |
| 显式关系 | 可结合 body 的 free-region / outlives 信息 | 只使用当前 higher-ranked 检查明确提供的 assumptions/constraints |
| 生命周期 | 属于该 body 的参数环境 | 属于 inference 过程中的 fresh universe |
| Type IR 判断 | `is_param()` / `is_free()` | placeholder，不属于 `Region::is_free()` |

将函数签名带入自己的 body 用 liberate；证明一个 higher-ranked 关系用 `enter_forall`。两者不能互换。

### 14. 一套机械判断流程

看到某个 region 时依次问：

```text
1. 它是否仍由 Binder 管理？
   是 -> ReBound，并计算 de Bruijn index

2. 它是否属于 item GenericArgs？
   是 -> ReEarlyParam(index)

3. 它是否是当前函数签名 liberate 到 body 的参数？
   是 -> ReLateParam(scope, kind)

4. 它是否是打开 forall 后的 arbitrary rigid name？
   是 -> RePlaceholder(universe, bound identity)

5. 它是否是等待约束求解的 existential unknown？
   是 -> ReVar(RegionVid)，并检查变量的创建 universe

6. 当前阶段是否已经不需要 region identity？
   是 -> ReErased
```

## 常见误区

1. Universe index 描述 nameability，不描述 lifetime 长短或 outlives 顺序。
2. `ReBound` 使用 de Bruijn index；`RePlaceholder` 使用 universe，二者是打开 binder 前后的不同表示。
3. `ReLateParam` 属于函数 body 参数环境；`RePlaceholder` 属于临时 higher-ranked inference。
4. Placeholder 是任意但固定的刚性名字；`ReVar` 才是待求解变量。
5. 新 universe 可以命名旧 universe 的名字；旧 universe 不能命名后来引入的新名字。
6. 防止 placeholder escape 是量词正确性的要求，不是 Rust 借用值在运行时泄漏的问题。

## 本章小结

Region 表示的主线可以压缩为：

```text
item generic          ReEarlyParam(index)
closed forall         ReBound(debruijn, slot)
function body         ReLateParam(scope, kind)
opened forall         RePlaceholder(universe, slot)
existential unknown   ReVar(vid @ creation_universe)
identity no longer needed
                      ReErased
```

`Binder` 回答“变量由哪层量词绑定”，Universe 回答“打开量词后谁能引用这个新名字”。只要始终分开这两个问题，就能解释 placeholder 刚性、inference var 的可选解范围和 leak check 的必要性。
