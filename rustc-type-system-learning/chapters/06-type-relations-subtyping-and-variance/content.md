---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "06"
document: content
status: completed
updated_at: 2026-08-01
---

# 06. 类型关系、子类型与 Variance

## 学习目标

完成本章后，应当能够：

1. 准确区分 `At::sub(expected, actual)`、`At::sup(expected, actual)` 与 `At::eq` 的方向。
2. 把一次复合类型关系分解为 covariance、contravariance、invariance 与 bivariance 下的参数关系。
3. 使用 `Variance::xform` 手算嵌套位置的最终 variance。
4. 区分定义点 variance inference 与关系检查时的 use-site ambient variance。
5. 追踪 `TypeRelation` / `Relate`、`TypeRelating`、`super_combine_tys` 与 obligations 的协作。
6. 解释 subtype relation 与 coercion 的边界，以及 coercion 为什么可能产生 adjustments。
7. 解释 LUB/GLB 的方向，以及 contravariant 位置为何交换两种 lattice operation。

## 前置知识

- 第 01 章的 Type IR、`TyKind`、`GenericArg` 与 interning。
- 第 02 章的递归 traversal 与 binder-aware 操作。
- 第 04 章的 region kinds、higher-ranked quantifier 与 universe。
- 第 05 章的 `InferCtxt`、type/region vars、snapshot、obligations 与解析层级。

## 核心心智模型

类型关系不是“比较两个完整类型后返回 true/false”，而是一种携带方向的递归约束生成过程：

```text
入口关系
  eq(A, B)   -> ambient = Invariant
  sub(A, B)  -> ambient = Covariant      -> 要求 A <: B
  sup(A, B)  -> ambient = Contravariant  -> 要求 B <: A

递归进入类型构造器
  -> 查询构造器参数的 definition-site variance
  -> ambient.xform(parameter_variance)
  -> 在新的 ambient variance 下 relate 对应参数

叶子
  TyVar / ConstVar -> 合并、实例化或产生延迟 obligation
  Region           -> 产生 subregion / eqregion constraint
  Alias            -> 可能产生 normalization / projection obligation
  不兼容 rigid Ty  -> TypeError
```

Variance 回答的是：

```text
如果把某个参数 A 换成 B，
外层类型的子类型方向如何变化？
```

| Variance | 参数关系如何传到外层 |
|---|---|
| Covariant `+` | 保持方向：`A <: B` 推出 `F<A> <: F<B>` |
| Contravariant `-` | 反转方向：`B <: A` 推出 `F<A> <: F<B>` |
| Invariant `o` | 必须相等：仅 `A == B` 才允许 |
| Bivariant `*` | 参数差异不影响该外层关系 |

最重要的区分：

```text
definition-site variance
  = struct/enum/function generic parameter在定义中的总体使用方式
  = tcx.variances_of(def_id)

use-site ambient variance
  = 当前这一次 eq/sub/sup 递归走到这里时的关系方向
  = TypeRelating::ambient_variance

最终参数关系
  = ambient_variance.xform(definition_site_variance)
```

## 源码地图

| 主题 | 当前仓库路径与关键符号 |
|---|---|
| `eq/sub/sup/lub` 入口 | `compiler/rustc_infer/src/infer/at.rs`：`At::{eq, sub, sup, relate, lub}` |
| relation 抽象 | `compiler/rustc_type_ir/src/relate.rs`：`TypeRelation`、`Relate`、`relate_args_with_variances` |
| variance 定义与组合 | `compiler/rustc_type_ir/src/lib.rs`：`Variance`、`Variance::xform` |
| 旧 solver 的递归 relation | `compiler/rustc_infer/src/infer/relate/type_relating.rs`：`TypeRelating` |
| 结构类型组合 | `compiler/rustc_type_ir/src/relate/combine.rs`：`super_combine_tys`、`combine_ty_args` |
| definition-site variance inference | `compiler/rustc_hir_analysis/src/variance/{terms,constraints,solve}.rs` |
| variance 查询 | `compiler/rustc_hir_analysis/src/variance/mod.rs`：`variances_of` |
| lattice relation | `compiler/rustc_infer/src/infer/relate/lattice.rs`：`LatticeOp`、`LatticeOpKind` |
| coercion | `compiler/rustc_hir_typeck/src/coercion.rs`：`Coerce`、`FnCtxt::coerce`、`CoerceMany` |
| 回归测试 | `tests/ui/variance/variance-types.rs`、`variance-regions-direct.rs` |

## 源码精读

以下片段以当前 checkout 为准；为聚焦本章概念，省略 new solver 分支、诊断字段和部分错误处理。

### 1. `At::{sub,sup,eq}`：参数名不等于关系方向

路径：`compiler/rustc_infer/src/infer/at.rs`

源码文档明确规定：

```rust,ignore
infcx.at(cause, param_env).sub(a, b)
// requires a <: b

infcx.at(cause, param_env).sup(a, b)
// requires b <: a

infcx.at(cause, param_env).eq(a, b)
// requires a == b
```

API 的正式参数叫 `expected` 与 `actual`，但第一个参数始终叫 expected 主要是为了诊断：

```rust,ignore
pub fn sub(self, ..., expected: T, actual: T) {
    TypeRelating::new(..., Variance::Covariant)
        .relate(expected, actual)
}

pub fn sup(self, ..., expected: T, actual: T) {
    TypeRelating::new(..., Variance::Contravariant)
        .relate(expected, actual)
}

pub fn eq(self, ..., expected: T, actual: T) {
    TypeRelating::new(..., Variance::Invariant)
        .relate(expected, actual)
}
```

因此函数调用检查常使用：

```text
sup(expected_parameter_type, actual_argument_type)
```

它表达：

```text
actual_argument_type <: expected_parameter_type
```

不要根据 `sub` 这个名字猜测“actual 是 expected 的 subtype”；必须直接读 API 的方向。

### 2. `Variance::xform`：嵌套位置的方向乘法

路径：`compiler/rustc_type_ir/src/lib.rs`

```rust,ignore
pub enum Variance {
    Covariant,
    Invariant,
    Contravariant,
    Bivariant,
}

pub fn xform(self, v: Variance) -> Variance {
    match (self, v) {
        (Covariant, Covariant) => Covariant,
        (Covariant, Contravariant) => Contravariant,
        (Covariant, Invariant) => Invariant,
        (Covariant, Bivariant) => Bivariant,

        (Contravariant, Covariant) => Contravariant,
        (Contravariant, Contravariant) => Covariant,
        (Contravariant, Invariant) => Invariant,
        (Contravariant, Bivariant) => Bivariant,

        (Invariant, _) => Invariant,
        (Bivariant, _) => Bivariant,
    }
}
```

把 `+/-/o/*` 当作方向运算：

| ambient `xform` inner | `+` | `-` | `o` | `*` |
|---|---:|---:|---:|---:|
| `+` | `+` | `-` | `o` | `*` |
| `-` | `-` | `+` | `o` | `*` |
| `o` | `o` | `o` | `o` | `o` |
| `*` | `*` | `*` | `*` | `*` |

三个实用规则：

```text
两次反转：- xform - = +
进入 invariant 后：o xform anything = o
进入 bivariant 后：* xform anything = *
```

例如：

```text
fn(*const Vec<T>)

ambient                   = +
函数输入                  = -
*const 的参数             = +
Vec 的参数                = +

+ xform - xform + xform + = -
```

所以 `T` 在整个函数指针类型中是 contravariant。

### 3. `TypeRelating::relate_with_variance`：保存、变换、递归、恢复

路径：`compiler/rustc_infer/src/infer/relate/type_relating.rs`

```rust,ignore
fn relate_with_variance<T: Relate<_>>(
    &mut self,
    variance: Variance,
    ...,
    a: T,
    b: T,
) -> RelateResult<_, T> {
    let old = self.ambient_variance;
    self.ambient_variance = self.ambient_variance.xform(variance);

    let result = if self.ambient_variance == Bivariant {
        Ok(a)
    } else {
        self.relate(a, b)
    };

    self.ambient_variance = old;
    result
}
```

这和 binder-aware folder 的结构相似：

```text
保存父级上下文
  -> 进入子结构时变换上下文
  -> 递归
  -> 恢复父级上下文
```

`ambient_variance` 必须在返回前恢复，否则前一个 generic arg 的 variance 会污染下一个 arg。

### 4. `FnSig::relate`：输入反转，输出保持

路径：`compiler/rustc_type_ir/src/relate.rs`

```rust,ignore
for (a_input, b_input) in inputs {
    relation.relate_with_variance(
        Variance::Contravariant,
        ...,
        a_input,
        b_input,
    );
}

relation.relate(a.output(), b.output());
```

若要证明：

```text
fn(A1) -> R1 <: fn(A2) -> R2
```

需要：

```text
A2 <: A1    // 输入反转
R1 <: R2    // 输出保持
```

原因是调用者只知道右侧契约。右侧允许传入的每个 `A2`，左侧实现都必须能接收；左侧返回的
`R1` 必须能作为右侧承诺的 `R2` 使用。

### 5. `combine_ty_args`：ADT 参数按 `variances_of` 递归

路径：`compiler/rustc_type_ir/src/relate/combine.rs`

```rust,ignore
for (i, (a, b)) in zip(a_args, b_args).enumerate() {
    let variance = variances[i];
    relation.relate_with_variance(variance, ..., a, b)
}
```

`TypeRelating::relate_ty_args` 在非 invariant ambient 下先查询：

```rust,ignore
let variances = self.cx().variances_of(def_id);
combine_ty_args(..., variances, a_args, b_args, ...)
```

若 ambient 已经 invariant，则直接 invariantly relate 所有 args，既符合 `o xform _ = o`，
也避免无意义的 variance query 和潜在 query cycle。

### 6. region relation：Type subtype 转成 outlives constraint

路径：`compiler/rustc_infer/src/infer/relate/type_relating.rs`

```rust,ignore
match self.ambient_variance {
    Covariant => make_subregion(origin, b, a, ...),
    Contravariant => make_subregion(origin, a, b, ...),
    Invariant => make_eqregion(origin, a, b, ...),
    Bivariant => unreachable!(),
}
```

源码中的注释给出：

```text
Subtype(&'a u8, &'b u8)
  -> Outlives('a: 'b)
  -> SubRegion('b, 'a)
```

这里同时存在两种顺序：

```text
类型子类型：&'a T <: &'b T
outlives：  'a: 'b
region 集合：points('b) ⊆ points('a)
```

`make_subregion(sub, sup)` 的参数因此是 `('b, 'a)`。

### 7. variance inference：从 `*` 开始按约束收紧

路径：

- `compiler/rustc_hir_analysis/src/variance/constraints.rs`
- `compiler/rustc_hir_analysis/src/variance/solve.rs`

定义点 variance 不是手写在大多数 ADT 上，而是从字段递归收集约束：

```rust,ignore
TyKind::Ref(region, ty, mutbl) => {
    add_constraints_from_region(region, ambient);
    add_constraints_from_mt(ty, mutbl, ambient);
}

Mutability::Mut => {
    add_constraints_from_ty(ty, ambient.xform(Invariant));
}

FnSig => {
    for input in inputs {
        add_constraints_from_ty(input, ambient.xform(Contravariant));
    }
    add_constraints_from_ty(output, ambient);
}
```

solver 将每个待推断参数初始化为 bivariant：

```text
solutions[param] = *
```

随后反复取 variance lattice 上的 GLB：

```text
      *
    -   +
      o
```

因此：

```text
只出现在 covariant 位置：* glb + = +
只出现在 contravariant 位置：* glb - = -
同时出现在 + 和 -：         + glb - = o
任何 invariant 使用：       _ glb o = o
```

当前实现还强制 const parameters invariant；函数中未使用而仍为 bivariant 的 generic
parameters 也会收紧为 invariant。

### 8. coercion：在 relation 之上尝试可插入的转换

路径：`compiler/rustc_hir_typeck/src/coercion.rs`

`FnCtxt::coerce(source, target)` 使用：

```rust,ignore
let ok = self.commit_if_ok(|_| coerce.coerce(source, target))?;
```

内部可能尝试：

- mutability weakening / reborrow；
- autoderef；
- unsizing，如 `&[T; N] -> &[T]`；
- fn item 或无捕获 closure 到 fn pointer；
- `!` 到其他类型；
- 最终的 subtype/unification relation。

成功时除了推理副作用和 obligations，还可能产生 expression adjustments。失败的候选通过
snapshot 回滚。纯 `sub` relation 不会为表达式插入这些 adjustments。

### 9. lattice：LUB/GLB 在 contravariant 位置交换

路径：`compiler/rustc_infer/src/infer/relate/lattice.rs`

```text
LUB(A, B) = A、B 的最小共同 supertype
GLB(A, B) = A、B 的最大共同 subtype
```

`LatticeOp::relate_with_variance`：

```rust,ignore
match variance {
    Invariant => eq(a, b),
    Covariant => relate(a, b),
    Contravariant => {
        self.kind = self.kind.invert();
        let result = self.relate(a, b);
        self.kind = self.kind.invert();
        result
    }
    Bivariant => Ok(a),
}
```

所以：

```text
LUB(fn(A), fn(B))
  -> 输入是 contravariant
  -> 对输入求 GLB(A, B)
```

region 还有一次顺序反转。源码示例：

```text
GLB(&'static T, &'a T) = &'static T
LUB(&'static T, &'a T) = &'a T
```

前提是 `'static: 'a`。

## 正文

### 1. 先固定子类型符号

本章使用：

```text
A <: B
```

表示：任何需要 `B` 的位置都可以安全使用 `A`。

因此 `A` 是更具体或能力更强的值类型，`B` 是允许它出现的上界。典型 reference 例子：

```text
'long: 'short

&'long T <: &'short T
```

持有更长有效期保证的引用可以用于只要求更短有效期的位置。

### 2. `sub` 与 `sup` 的 expected/actual 规则

| 调用 | 建立的关系 | initial ambient |
|---|---|---|
| `sub(expected, actual)` | `expected <: actual` | `Covariant` |
| `sup(expected, actual)` | `actual <: expected` | `Contravariant` |
| `eq(expected, actual)` | `expected == actual` | `Invariant` |

`expected` 永远在第一个参数，服务于错误信息；它不保证处于 `<:` 的右侧。

函数调用：

```rust,ignore
fn consume(expected: E);
let actual: A = ...;
consume(actual);
```

通常要求：

```text
A <: E
```

因此调用方向是：

```text
sup(E, A)
```

### 3. Equality 是 invariant relation

`eq(A, B)` 不只是 `A == B` 的结构哈希判断。若节点指针不同，它仍会递归：

```text
Vec<?T0> == Vec<u32>
  -> ADT identity 相同
  -> ambient invariant
  -> ?T0 == u32
  -> instantiate ?T0 = u32
```

若两侧是 type vars：

```text
?T0 == ?T1
  -> TypeVariableStorage::equate
```

若两侧是 regions：

```text
?r0 == 'a
  -> make_eqregion
```

alias、opaque 或 projection 可能不能立即结构展开，relation 会注册 predicate/obligation，
由后续 normalization 或 solver 处理。

### 4. Covariance：保持方向

共享引用对 lifetime 和 pointee 都是 covariant：

```text
'long: 'short
T1 <: T2

&'long T1 <: &'short T2
```

常见 covariant 构造：

- `&'a T` 中的 `'a`；
- `&'a T` 中的 `T`；
- `*const T` 中的 `T`；
- tuple、array、slice 的元素；
- `fn() -> T` 的返回值；
- 只把参数存储在 covariant 字段中的 ADT。

### 5. Contravariance：反转方向

函数输入是典型 contravariant 位置：

```text
A <: B

fn(B) <: fn(A)
```

原因：

- `fn(A)` 的调用者只承诺传 `A`；
- 一个能接受所有 `B` 的函数，在 `A <: B` 时也能接受 `A`；
- 反过来，一个只接受更窄 `A` 的函数不能充当要求能接受 `B` 的函数。

嵌套两层 contravariance 会恢复 covariance：

```text
fn(fn(T))

outer input = -
inner function input = -
- xform - = +
```

### 6. Invariance：两边必须一致

`&mut T` 对其 lifetime 仍是 covariant，但对 `T` invariant：

```text
&'long mut T <: &'short mut T
```

可以缩短独占借用的有效期；但不能仅凭 `T1 <: T2` 推出：

```text
&mut T1 <: &mut T2
```

否则可以通过 `&mut T2` 写入一个不是 `T1` 的值，破坏原存储。

常见 invariant 位置：

- `&mut T` 的 `T`；
- `*mut T` 的 `T`；
- `UnsafeCell<T>` / `Cell<T>` 的 `T`；
- projection、trait object 等保守结构关系中的部分 args；
- const generic parameters。

“进入 invariant 后无法逃出”：

```text
&mut fn(T)

ambient + xform invariant xform contravariant
  = invariant
```

### 7. Bivariance：关系中忽略参数，但仍有 WF 边界

Bivariant 表示参数变化不影响外层 subtype relation。用户定义类型中真正 bivariant 的参数
并不常见；未使用 generic parameter 通常还会触发语言错误，或受 well-formedness 约束。

`combine_ty_args` 对含未决推理变量的 bivariant args 可能额外注册 `WellFormed` predicates，
避免参数在 relation 中完全不受约束却逃逸。

因此：

```text
bivariant relation edge 上不比较两个 arg
```

不等于：

```text
这个类型在任何参数下自动 well-formed
```

### 8. Definition-site variance 的手算方法

从 item 的最外层字段位置以 covariant 开始：

```text
struct Producer<T> {
    value: T,
}

T: +
```

```text
struct Consumer<T> {
    call: fn(T),
}

T: + xform - = -
```

```text
struct Both<T> {
    value: T,       // +
    call: fn(T),    // -
}

T: glb(+, -) = invariant
```

```text
struct Mutable<'a, 'b, T> {
    value: &'a mut &'b T,
}

'a: +                         // outer reference lifetime
'b: + xform invariant xform + = invariant
T:  + xform invariant xform + = invariant
```

### 9. Use-site relation 的手算方法

假设：

```text
struct Packet<A, B, C> {
    out: A,          // A: +
    input: fn(B),    // B: -
    cell: Cell<C>,   // C: invariant
}
```

要证明：

```text
Packet<A1, B1, C1> <: Packet<A2, B2, C2>
```

逐参数得到：

```text
A1 <: A2
B2 <: B1
C1 == C2
```

若外层是 equality：

```text
Packet<A1, B1, C1> == Packet<A2, B2, C2>
```

ambient 已经 invariant，三个参数都按 equality 处理；definition-site 的 `+/-` 不再改变结果。

### 10. Type vars：subtyping 不总是 union

第 05 章的 equality：

```text
?T0 == ?T1
  -> 合并 equality class
```

但 subtyping：

```text
?T0 <: ?T1
```

不能直接把二者 equate，因为将来完全可能得到两个不同但具有 subtype 关系的类型。旧 solver
的 `TypeRelating` 会产生：

```text
SubtypePredicate(?T0, ?T1)
```

等待后续 fulfillment 在获得更多信息后处理。这就是第 05 章中
`sub_unification_table` / subtype relation 所服务的上层方向信息之一。

若一侧是已知结构，relation 可能通过 generalization 和 variance 安全实例化 type var；
具体行为取决于变量处于关系哪一侧、ambient variance 和 universe。

### 11. Coercion 不是 subtyping 的别名

纯 subtype：

```text
检查 A <: B
产生约束/obligation
不改写表达式
```

coercion：

```text
尝试把 source expression 变成 target type
可能插入 MIR/HIR adjustment
可能尝试多个候选并 rollback
内部仍可能调用 subtype/unification
```

例子：

```rust,ignore
let p: &i32 = &mut value;
```

这里不仅是比较 `&mut i32` 与 `&i32` 的 subtype；coercion 会建立 shared reborrow /
mutability weakening adjustment。

```rust,ignore
let s: &[u8] = &[1, 2, 3];
```

这里需要 array-to-slice unsizing。

```rust,ignore
fn f(x: i32) {}
let p: fn(i32) = f;
```

这里需要 fn item 到 fn pointer 的 coercion。

### 12. LUB/GLB 与控制流汇合

对：

```rust,ignore
let value = if cond { a } else { b };
```

通常需要为两个分支找到共同目标类型。若 `A <: B`：

```text
LUB(A, B) = B
GLB(A, B) = A
```

reference：

```text
'long: 'short

&'long T <: &'short T
LUB(&'long T, &'short T) = &'short T
GLB(&'long T, &'short T) = &'long T
```

function input：

```text
LUB(fn(A), fn(B))
  = fn(GLB(A, B))
```

因为 input contravariant，lattice operation 翻转。

实际 `if` / `match` 还会优先尝试定向 coercion，`CoerceMany` 可能更新此前分支的 adjustments；
只有简单心智模型下才可直接把所有行为缩成纯类型 lattice。

### 13. 一次 relation 的源码追踪模板

```text
1. 调用的是 eq、sub、sup、coerce、lub 还是 glb？
2. 两个 API 参数中哪个是 expected，真正的 <: 方向是什么？
3. initial ambient variance 是 +、- 还是 o？
4. 当前 TyKind 是 ref、fn、ADT、alias 还是 Infer？
5. 进入每个参数时使用什么 definition-site variance？
6. ambient.xform(parameter_variance) 的结果是什么？
7. 叶子最终执行 equate/instantiate、region constraint，还是注册 obligation？
8. 当前是否处于 coercion snapshot；成功后是否产生 adjustment？
9. lattice 路径是否因 contravariant 位置交换 LUB/GLB？
```

### 14. 为什么 contravariant `xform` contravariant 得到 covariance

把 covariance 记成“保持 `<:` 方向”，contravariance 记成“翻转 `<:` 方向”。连续进入两个
contravariant 构造器会翻转两次，因此回到原方向。

假设：

```text
Cat <: Animal
Consumer<T> = fn(T)
```

函数输入 contravariant，所以第一次翻转：

```text
Consumer<Animal> <: Consumer<Cat>
```

再令：

```text
Outer<X> = fn(X)
```

`Outer` 对 `X` 仍 contravariant，第二次翻转：

```text
Outer<Consumer<Cat>> <: Outer<Consumer<Animal>>
```

展开别名：

```text
fn(fn(Cat)) <: fn(fn(Animal))
```

最终方向与最初的 `Cat <: Animal` 相同，所以 `T` 在 `fn(fn(T))` 中 covariant：

```text
+ xform - xform - = +
```

### 15. LUB 与 GLB 的全称及偏序含义

当前 rustc 的 `LatticeOpKind` 只有：

```text
LUB = Least Upper Bound     = 最小上界 = 最近的共同 supertype
GLB = Greatest Lower Bound  = 最大下界 = 最近的共同 subtype
```

这里的“上”和“下”由 subtype 偏序 `<:` 决定。对类型 `A`、`B`：

```text
U 是 upper bound：A <: U 且 B <: U
LUB 是所有共同 upper bounds 中最靠近 A、B 的那个

L 是 lower bound：L <: A 且 L <: B
GLB 是所有共同 lower bounds 中最靠近 A、B 的那个
```

若已知：

```text
A <: B
```

则：

```text
LUB(A, B) = B
GLB(A, B) = A
```

可以画成：

```text
B          <- 两者最近的共同 supertype，即 LUB
^
|
A          <- 两者最近的共同 subtype，即 GLB
```

对两个没有直接 subtype 关系的类型，共同上界或下界可能来自更远的类型，也可能不存在可由
当前 rustc relation 构造的结果。Rust 类型并不构成一个对任意二元组都保证有解的 complete
lattice，因此 LUB/GLB 操作可以失败。

`GUB` 不是本模块使用的名称。若把它展开为 `Greatest Upper Bound`，它寻找的是“最大的共同
上界”，通常会远离输入并趋向全局 top；类型合流真正需要的是最精确、最近的共同 supertype，
所以使用 **least** upper bound。

### 16. Variance inference 是自顶向下还是自底向上

答案分三层：

1. **单个类型表达式的路径传播是自顶向下。**

   从 item/字段的 covariant 根上下文开始，每进入一个构造器就执行：

   ```text
   child_ambient = parent_ambient.xform(parameter_variance)
   ```

2. **同一参数的多个出现位置在参数处汇合。**

   每条自顶向下路径产生一个 variance constraint，solver 用 variance lattice 的 GLB
   汇总这些贡献。

   ```rust,ignore
   struct Both<T> {
       output: T,       // 顶向下得到 +
       consume: fn(T),  // 顶向下得到 + xform - = -
   }
   ```

   汇合结果：

   ```text
   variance(T) = GLB(+, -) = invariant
   ```

3. **跨 item 或递归定义不是一次 bottom-up pass，而是 fixed-point solving。**

   本 crate 中尚未求出的 ADT variance 以 symbolic `InferredTerm` 参与其他 item 的约束；所有
   参数先初始化为 bivariant `*`，然后反复执行：

   ```text
   new = GLB(old, evaluated_constraint)
   ```

   直到没有值变化。完整描述是：

   ```text
   自顶向下收集路径约束
     + 在每个参数处汇合多个使用位置
     + 跨 item 依赖迭代到 fixed point
   ```

### 17. `xform` 表如何从关系语义推导

`xform` 不是任意规定的数字乘法；第一个操作数是 outer/ambient relation，第二个操作数是
当前参数在构造器声明中的 inner variance。操作数顺序不能交换。

逐行推导：

```text
ambient = Covariant：外层要求保持方向
  -> 直接采用 inner variance
  +×+=+，+×-=-，+×o=o，+×*=*

ambient = Contravariant：外层先翻转一次
  inner +：只翻转一次       -> -
  inner -：再翻转一次       -> +
  inner o：相等反转后仍相等 -> o
  inner *：本来不约束       -> *

ambient = Invariant：外层类型必须相等
  -> 子参数必须按 equality 关系检查
  -> o×anything=o

ambient = Bivariant：整个当前子树不影响外层关系
  -> 更深层参数也不需要关系约束
  -> *×anything=*
```

这也说明它不是普通的交换律乘法：

```text
Invariant.xform(Bivariant) = Invariant
Bivariant.xform(Invariant) = Bivariant
```

前者表示“外层要求类型 identity 相等”，不能因为参数 definition-site bivariant 就把两个不同
实例视为相同类型；后者表示“更外层已经完全忽略这个子树”，内部的 invariant 位置也不会重新
产生约束。

### 18. `ambient` 与 `inner` 分别属于哪里

对调用：

```text
ambient.xform(inner)
```

两个操作数的来源不同：

```text
ambient
  = 从类型树根部沿已经走过的路径累计下来的关系方向
  = 当前节点所处的上下文

inner
  = 即将跨过的这一条“父构造器 -> 参数”边自身的 definition-site variance
  = 当前类型构造器对这个参数的声明方向
```

例如推断：

```rust,ignore
struct S<T> {
    field: fn(Vec<T>),
}
```

逐层计算：

| 当前节点 | 进入该参数前的 ambient | 下一条边的 inner | 新 ambient |
|---|---:|---:|---:|
| 字段类型根 `fn(Vec<T>)` | `+` | 函数输入 `-` | `-` |
| `Vec<T>` | `-` | `Vec` 参数 `+` | `-` |
| `T` | `-` | — | 最终贡献 `-` |

也就是：

```text
root ambient = +
进入 fn input： +.xform(-) = -
进入 Vec arg：  -.xform(+) = -
```

再看两层函数：

```text
fn(fn(T))

root ambient = +
进入外层 input：+.xform(-) = -
进入内层 input：-.xform(-) = +
```

`inner` 不是“剩余整棵子树最终算出的 variance”，而只是当前跨过的一条边。例如在
`fn(Vec<T>)` 中，跨入 `Vec<T>` 时 inner 是函数输入的 `-`；到达 `Vec<T>` 后，再跨入 `T`
时 inner 才是 `Vec` 参数的 `+`。

同一机制也用于 use-site relation，只是根 ambient 不一定为 `+`：

```text
sub(A, B) -> 根 ambient = +
sup(A, B) -> 根 ambient = -
eq(A, B)  -> 根 ambient = o
```

因此 definition-site variance inference 与实际 relation 共用 `xform` 规则，但根 ambient 的
来源不同：前者从字段的 `+` 开始收集使用位置，后者由本次 `sub/sup/eq` 决定。

### 19. 用“值集合”理解函数参数逆变

对普通类型，集合直觉是：

```text
Cat <: Animal
Values(Cat) ⊆ Values(Animal)
```

到了函数参数，必须观察的不是参数集合本身，而是“哪些函数值满足这个函数类型”：

```text
Values(fn(Animal))
  = 能正确处理每一种 Animal 的函数集合

Values(fn(Cat))
  = 只要求能正确处理每一种 Cat 的函数集合
```

“能处理所有 Animal”比“能处理所有 Cat”是更强的要求，因此满足前者的函数更少：

```text
Values(fn(Animal)) ⊆ Values(fn(Cat))
```

按照 subtype 的集合直觉，得到：

```text
fn(Animal) <: fn(Cat)
```

这正是参数逆变：

```text
Cat <: Animal
fn(Animal) <: fn(Cat)
```

替换原则也给出相同结论。若某处只会传入 `Cat`，它要求的是 `fn(Cat)`；传入一个能处理任意
`Animal` 的函数当然安全。反过来，要求 `fn(Animal)` 的位置可能传入 `Dog`，只会处理 `Cat`
的函数不能放在那里。

可以把输入与输出对照：

```text
输入位置是责任：
  接受的输入集合越大 -> 承担的责任越强
  -> 满足契约的函数越少
  -> 函数类型越小

输出位置是保证：
  保证返回 Cat -> 当然也保证返回 Animal
  -> fn() -> Cat <: fn() -> Animal
```

所以“subtype 的集合更小”并没有失效；发生变化的是所比较的集合层级：

```text
参数层：Values(Cat) ⊆ Values(Animal)
函数层：Values(fn(Animal)) ⊆ Values(fn(Cat))
```

### 20. 为什么嵌套函数的两次逆变恢复协变

先给内层函数类型起名字：

```text
Cat <: Animal

CatHandler    = fn(Cat)
AnimalHandler = fn(Animal)
```

由函数参数逆变：

```text
AnimalHandler <: CatHandler
```

现在暂时忘掉 `Handler` 内部也是函数，把它们当成普通的两个类型：

```text
AnimalHandler <: CatHandler
```

再放进外层函数参数。外层输入同样逆变，所以：

```text
fn(CatHandler) <: fn(AnimalHandler)
```

展开别名：

```text
fn(fn(Cat)) <: fn(fn(Animal))
```

这与原始方向：

```text
Cat <: Animal
```

相同，因此 `T` 在 `fn(fn(T))` 中 covariant。

从替换原则看，假设某处要求：

```text
outer: fn(AnimalHandler)
```

这个调用者只会向 `outer` 传入 `AnimalHandler`。如果实际提供的是：

```text
fn(CatHandler)
```

仍然安全，因为每个 `AnimalHandler` 都是一个合法的 `CatHandler`：能处理所有 Animal 的函数
当然能处理 Cat。所以：

```text
fn(CatHandler) 可以替代 fn(AnimalHandler)
```

反方向不安全。要求 `fn(CatHandler)` 的调用者可能传入一个只会处理 Cat 的 handler；一个
只接受 `AnimalHandler` 的 outer function 不能接收它，因为这个 handler 未必能处理 Dog。

集合关系也可以分两次写：

```text
第一层：
Values(AnimalHandler) ⊆ Values(CatHandler)

第二层：
Values(fn(CatHandler)) ⊆ Values(fn(AnimalHandler))
```

每经过一个函数输入，包含方向翻转一次：

```text
一次函数输入：逆变
两次函数输入：协变
三次函数输入：再次逆变
```

## 常见误区

### 误区一：`sub(expected, actual)` 表示 `actual <: expected`

错误。`sub(a, b)` 表示 `a <: b`；需要 `actual <: expected` 时通常调用 `sup(expected, actual)`。

### 误区二：`&mut T` 对所有参数都 invariant

`&'a mut T` 对 `'a` covariant，对 `T` invariant。

### 误区三：contravariance 意味着函数返回值反转

函数输入 contravariant，输出 covariant。

### 误区四：两个 subtype-related TyVar 可以直接 union

union 表示 equality。纯 subtype relation 必须保留方向，通常形成 subtype obligation 或上下界。

### 误区五：variance 是每次关系检查临时推出来的

ADT definition-site variance 通常由 crate-level variance inference 计算并保存在
`tcx.variances_of`；每次 relation 只是把它和 ambient variance 组合。

### 误区六：能 coercion 就一定存在纯 subtype relation

coercion 可以插入 reborrow、unsize、autoderef 或 fn-pointer adjustment，因此能力严格高于一次
纯 `sub` 检查。

### 误区七：LUB 总是逐参数求 LUB

contravariant 参数需要求相反的 lattice operation；invariant 参数要求 equality。

## 本章小结

- `sub(a, b)` 建立 `a <: b`，`sup(a, b)` 建立 `b <: a`，第一个参数始终用于 expected 诊断。
- relation 从 initial ambient variance 出发，通过 `Variance::xform` 把方向递归传入子结构。
- covariance 保持方向，contravariance 反转，invariance 要求相等，bivariance 忽略 relation edge。
- ADT variance 在定义点由约束求解计算；使用点通过 `tcx.variances_of` 读取。
- 函数输入 contravariant、输出 covariant；`&mut T` 的 lifetime covariant、pointee invariant。
- equality、subtyping 与 coercion 是不同操作；coercion 可能插入 adjustments 并通过 snapshot 试探。
- LUB 找最小共同 supertype，GLB 找最大共同 subtype；contravariant 位置交换两者。
- type relation 的结果不仅是成功/失败，还可能修改 inference state、产生 region constraints 与 obligations。
