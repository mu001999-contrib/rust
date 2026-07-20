---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "15"
document: content
status: completed
updated_at: 2026-09-21
---

# 15. Trait Objects 与 Dyn Compatibility

## 学习目标

1. 将 `dyn Iterator<Item = u32> + Send + 'obj` 拆成 existential predicates 与对象 lifetime。
2. 区分 trait 是否 dyn-compatible、某方法是否能经对象动态调用、某具体类型是否满足对象约束。
3. 追踪具体类型到对象的 unsizing、object-bound candidate 与 vtable 的分工。
4. 区分引用 lifetime、对象 lifetime、方法 binder 与上一章的 opaque capture。
5. 从 trait 声明判断对象可用接口，并理解 `Self: Sized` 的逐项排除作用。

## 前置知识

第 03 章的 TraitRef args；第 06 章的关系与 coercion；第 07–13 章的 predicates、candidate 与 projection；第 14 章的 opaque、RPITIT 与 outlives。

## 核心心智模型

`dyn Trait` 隐藏具体实现类型，以固定的 trait 接口使用它。指向对象的指针携带数据指针与 vtable 元数据；编译期先验证接口和实现满足约束，运行时通过已构造的 vtable 找到方法实现。

这不是在运行时运行 trait solver。solver 证明可构造/可使用对象；vtable 完成运行时方法分派。

本章使用 edition 2024、当前仓库 next solver 源码。语言示例不启用实验性 dyn/const 扩展。dyn compatibility 曾称 object safety。

## 源码地图

| 仓库路径 | 关键符号 | 作用 |
|---|---|---|
| `compiler/rustc_type_ir/src/ty_kind.rs` | `TyKind::Dynamic` | 对象的 Type IR |
| `compiler/rustc_type_ir/src/predicate.rs` | `ExistentialPredicate`、`ExistentialTraitRef`、`ExistentialProjection`、`with_self_ty` | 擦除/补回 Self 的约束表示 |
| `compiler/rustc_hir_analysis/src/hir_ty_lowering/dyn_trait.rs` | `lower_trait_object_ty`、`lower_trait_object_lifetime` | 降低 dyn 语法并构造 predicates 与 region |
| `compiler/rustc_trait_selection/src/traits/dyn_compatibility.rs` | `dyn_compatibility_violations`、`is_vtable_safe_method`、`virtual_call_violations_for_method` | 接口检查及动态方法筛选 |
| `compiler/rustc_next_trait_solver/src/solve/trait_goals.rs` | `consider_builtin_unsize_to_dyn_candidate` | 具体类型转换为对象的证明 |
| `compiler/rustc_next_trait_solver/src/solve/assembly/mod.rs` | `assemble_object_bound_candidates` | 从对象自带约束证明 trait/projection goals |
| `compiler/rustc_trait_selection/src/traits/vtable.rs` | `own_existential_vtable_entries_iter`、`vtable_entries` | 筛选方法并组织 supertrait vtable |
| `compiler/rustc_middle/src/ty/vtable.rs` | `VtblEntry`、`vtable_allocation_provider` | 方法、布局、drop 与 upcast 元数据 |

## 源码精读

### 1. 对象类型保存的是约束与 region

`compiler/rustc_type_ir/src/ty_kind.rs::TyKind` 的相关变体，其余变体省略：

```rust,ignore
Dynamic(I::BoundExistentialPredicates, I::Region),
```

`compiler/rustc_type_ir/src/predicate.rs`，省略 derive 与注释：

```rust,ignore
pub enum ExistentialPredicate<I: Interner> {
    Trait(ExistentialTraitRef<I>),
    Projection(ExistentialProjection<I>),
    AutoTrait(I::TraitId),
}
```

第一个字段容纳带 binder 的对象约束；第二个字段单独保存对象 lifetime。这里没有把某个具体实现类型或运行时 vtable 地址直接嵌进 `TyKind::Dynamic`。

### 2. ExistentialTraitRef 省略 Self，而不是放入一个 TyVar

`compiler/rustc_type_ir/src/predicate.rs::ExistentialTraitRef::with_self_ty`，完整方法：

```rust,ignore
pub fn with_self_ty(self, interner: I, self_ty: I::Ty) -> TraitRef<I> {
    TraitRef::new(interner, self.def_id, [self_ty.into()].into_iter().chain(self.args.iter()))
}
```

例如普通 `TraitRef` 表示 `S: Convert<u32>`，args 是 `[S, u32]`；existential trait ref 仅保留 `[u32]`。反向转换 `erase_self_ty` 取原 args 的 `[1..]`。

“存在某个实现类型”是语义解释，不等于 IR 中保存了一个等待推断的 `?Self`。`with_self_ty` 可以补入待转换的具体类型，也可以补入对象类型本身，分别服务于构造检查和对象约束求解。

### 3. 能构造对象，不代表所有方法都可动态调用

`compiler/rustc_trait_selection/src/traits/dyn_compatibility.rs::is_vtable_safe_method`，省略 debug 语句：

```rust,ignore
if tcx.generics_require_sized_self(method.def_id) {
    return false;
}

virtual_call_violations_for_method(tcx, trait_def_id, method).is_empty()
```

同文件 `virtual_call_violations_for_method` 中，省略其他检查：

```rust,ignore
let own_counts = tcx.generics_of(method.def_id).own_counts();
if own_counts.types > 0 || own_counts.consts > 0 {
    errors.push(MethodViolation::Generic);
}
```

方法自身的 type/const 泛型会触发这项检查，lifetime 泛型不会。注意是方法 own generics：`dyn Transform<u32>` 已经固定 trait 的类型实参，与动态调用 `fn transform<T>(...)` 的问题不同。

`dyn_compatibility_violations_for_assoc_item` 对要求 `Self: Sized` 的 item 提前返回空 violations；但这种方法又会被上面的 vtable 检查排除。因此“trait 接受该 item”与“对象可调用该 item”是两个问题。

### 4. 构造对象确实会产生 type-outlives goal

`compiler/rustc_next_trait_solver/src/solve/trait_goals.rs::consider_builtin_unsize_to_dyn_candidate`，省略入口、probe 包装及 Sized 检查等部分：

```rust,ignore
ecx.add_goals(
    GoalSource::ImplWhereBound,
    b_data.iter().map(|pred| goal.with(cx, pred.with_self_ty(cx, a_ty))),
)?;

// ……另行添加 a_ty: Sized……

ecx.add_goal(GoalSource::Misc, goal.with(cx, ty::OutlivesPredicate(a_ty, b_region)))?;
```

`a_ty` 是具体源类型，`b_data` 是对象约束，`b_region` 是对象 lifetime。把 `S` 转为 `dyn Iterator<Item=u32> + Send + 'obj` 时，概念上生成：

```text
S: Iterator
<S as Iterator>::Item == u32
S: Send
S: Sized
S: 'obj
```

这里第一组对象 predicates 的 GoalSource 在当前实现中是 `ImplWhereBound`，outlives 则是 `Misc`。这也是为什么不能仅凭枚举名称判断来源一定是用户写的 impl where clause；需看具体调用点。上一章 opaque hidden-type bounds 的来源仍是 `AliasWellFormed`。

入口还检查目标 principal trait 的 dyn compatibility。上述函数负责具体类型到 dyn 的候选，dyn 到 dyn 的 upcasting 有另外的分支。

### 5. vtable 既含方法，也含布局与析构信息

`compiler/rustc_middle/src/ty/vtable.rs::VtblEntry`，省略 derive 与注释：

```rust,ignore
pub enum VtblEntry<'tcx> {
    MetadataDropInPlace,
    MetadataSize,
    MetadataAlign,
    Vacant,
    Method(Instance<'tcx>),
    TraitVPtr(TraitRef<'tcx>),
}
```

`Method` 保存已选定实现的 Instance；allocation 阶段生成函数指针。`TraitVPtr` 用于某些 supertrait upcast 所需的额外 vtable 指针。当前公共 header 是 drop、size、align；不需 drop 的具体类型可使用空 drop 指针。

`own_existential_vtable_entries_iter` 使用 `is_vtable_safe_method` 筛选方法；后续具体实例化还可能产生 Vacant 项。这里描述当前 rustc 实现，不承诺稳定 Rust ABI、固定方法偏移或 vtable 指针唯一性。

## 正文

### 1. 与泛型及 impl Trait 的区别

| 写法 | 编译期保留的信息 | 使用方式 |
|---|---|---|
| `fn f<T: Trait>(x: &T)` | 泛型参数及 bounds，实例化时确定 T | 通常经 monomorphization 静态分派 |
| `fn f() -> impl Trait` | 某个 opaque 身份；定义方确定 hidden type | 保持具体类型的抽象，不自动构造对象 |
| `fn f(x: &dyn Trait)` | dyn 接口，具体实现类型被擦除 | 通过对象元数据动态分派，可被优化器去虚拟化 |

`dyn Trait` 是动态大小类型（DST）；常通过 `&dyn Trait`、`&mut dyn Trait`、`Box<dyn Trait>` 使用。`&dyn Trait` 不要求堆分配，Box 才表达拥有堆分配对象的常见方式。

不同具体类型可以分别转换成相同的对象类型；它们不是因此在类型系统中彼此相等。例如：

```rust
fn choose(flag: bool) -> Box<dyn Iterator<Item = u32>> {
    if flag {
        Box::new(0..3)
    } else {
        Box::new(vec![4, 5].into_iter())
    }
}
```

两条分支经过 coercion 后都是 `Box<dyn Iterator<Item=u32>>`。对比上一章的 RPIT，它没有自动加入这层类型擦除。

### 2. 手算 existential predicates

```rust
type Iter<'obj> = dyn Iterator<Item = u32> + Send + 'obj;
```

可概念化为：

```text
Dynamic(
    [Trait(Iterator, args=[]),
     Projection(Iterator::Item, args=[], term=u32),
     AutoTrait(Send)],
    'obj
)
```

这是省略 binder 包装和内部排序的教学表示。Iterator 没有 Self 之外的 trait 泛型，故 existential args 为 `[]`。对于 `dyn Convert<u32, Out=bool>`，trait 和关联类型 projection 的 existential args 都保留 `[u32]`，projection 的 DefId 指向 `Convert::Out`。

普通关联类型通常要在对象类型中确定，例如 `Item=u32`，这样 `next` 的结果就是 `Option<u32>`。这不是先在运行时查出一个类型，再决定返回值布局。

一个对象至多有一个非 auto 的 principal trait，可以附加多个 auto traits。若需要两个普通 trait 的能力，可定义一个同时继承它们的 dyn-compatible trait 作为 principal。也存在只有 auto traits 的对象，此时 principal 可以为空。

更高阶的 `dyn for<'b> Fn(&'b str) -> usize + 'obj` 中，`'b` 属于调用签名的 binder；`'obj` 是对象 lifetime，二者分工不同。参见 [Reference：trait objects](https://doc.rust-lang.org/reference/types/trait-object.html)。

### 3. Dyn compatibility 是接口能否进行动态分派

固定对象类型后，调用者需要知道每个可调用方法的签名，且能通过受支持的 receiver 传入被擦除的实现对象。

检查时可以按以下顺序：

1. trait 及 supertraits 不要求 `Self: Sized`，supertraits 同样 dyn-compatible。
2. 普通关联类型可以由对象绑定；参与对象接口的 GAT 当前不支持；普通关联 const 在本章未启用扩展的条件下不支持。
3. 可动态调用的方法有受支持的 receiver，例如 `&self`、`&mut self`、`self: Box<Self>`，以及支持的 Rc/Arc/Pin 形式。
4. 方法自身没有 type/const 泛型；lifetime 泛型可以保留，因为它不需要为每个 lifetime 生成不同机器代码。
5. 方法不在普通参数/返回值里依赖裸 `Self`，也不直接返回 RPITIT 或使用 async fn 的隐藏返回类型。
6. 用逐项 `where Self: Sized` 把仅适合具体类型的功能排除出对象接口。

`Self::Item` 与裸 `Self` 要区分：对象的 `Item=u32` 可确定 `Self::Item` 的类型，而 `-> Self` 仍要求返回那个已被擦除的具体实现。两个 `&dyn Trait` 也可能指向不同具体类型，所以 `fn compare(&self, other: &Self)` 不能直接当成接受任意同接口对象。

方法类型泛型需要按实参单态化；当前固定 vtable 接口没有提供“对任意类型实参生成方法实例”的机制。RPITIT/async fn 的支持限制也是当前 Rust 对象模型的边界，并不是说其他语言或未来扩展绝无实现可能。

完整语言规则参见 [Reference：Dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility)。当前源码还包含 final 方法和实验性关联 const 的特殊分支，本章不把这些实验规则泛化成稳定用法。

### 4. 一个 trait 可同时提供动态接口和具体类型专用接口

```rust
trait Source {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    fn label<'a>(&self, text: &'a str) -> &'a str { text }
    fn build() -> Self where Self: Sized;
    fn inspect<T>(&self, _: T) where Self: Sized {}
}

struct Counter(u32);

impl Source for Counter {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.0 += 1;
        Some(self.0)
    }
    fn build() -> Self { Self(0) }
}

fn main() {
    let mut c = Counter::build();
    c.inspect("concrete call");
    let obj: &mut dyn Source<Item = u32> = &mut c;
    assert_eq!(obj.next(), Some(1));
    assert_eq!(obj.label("hi"), "hi");
}
```

`Source` 是 dyn-compatible，`next` 和 `label` 可动态调用；`build` 与 `inspect` 留给满足 `Self: Sized` 的类型。`&dyn Source` 本身是 Sized，不代表这里的 `Self = dyn Source` 满足 Sized；要区分 receiver 指针和被指向的 Self。

GAT 也可明确排除：`type Item<'a> where Self: Sized;` 配合其余合规方法时，可以构造该 trait 的对象，但不能通过对象使用这个 GAT。本机已验证该形式。逐项的 `Self: Sized` 与在 trait 层写 `trait Source: Sized` 的效果不同：后者排除整个 trait object。

### 5. 构造时证明与使用时证明

构造 `&mut dyn Source<Item=u32>` 时，rustc 检查源类型 `Counter` 的 trait impl 与 Item 等式，检查 Sized/outlives，并由指针 coercion 加入对象元数据。这一阶段能知道具体类型。

之后假设函数只收到 `obj: &mut dyn Source<Item=u32>`，它不需要重新猜测底层到底是哪种实现。`assemble_object_bound_candidates` 读取 Dynamic 的 bounds：

- 对 projection 和 auto-trait bounds，补入 Self 为对象类型，形成候选。
- 对 principal，补入对象类型并遍历 supertraits，形成对象候选。

因此 `<dyn Source<Item=u32> as Source>::Item` 可以依据对象自带 projection bound 归一化为 u32，而不是搜索 Counter 的 impl 才得到 u32。此信息是对象类型的一部分，不必放在函数的 caller_bounds 中才可使用。

`dyn Trait + Send` 的 Send 保证来自对象类型契约；若只有 `dyn Trait` 且 Trait 没有隐含 Send，则不能因为某次构造恰好使用 Send 类型，就在擦除后无条件恢复 Send。这与 opaque 的 auto-trait 行为要分开理解。

### 6. 对象 lifetime 是 outlives bound，不是 capture 列表

```rust,ignore
&'r (dyn Trait + 'obj)
```

- `'r`：这次借用对象的引用 lifetime。
- `'obj`：对象承诺底层实现类型满足的 outlives bound。

具体源类型 S 转换到对象时检查 `S: 'obj`，源码片段 4 已展示真正的 OutlivesPredicate。对于这类引用的良构性，还要让对象在引用使用期间有效；通常体现为 `'obj: 'r`，两者无需相等。

如果 S 是含 `&'data str` 的结构，`S: 'obj` 就要求相关借用支持 `'obj`。若 S 是拥有数据的 String，可以满足 `'static`；`Box<dyn Trait + 'static>` 的值仍可立刻 drop，`'static` 并不要求该 Box 永远存活。

这和 `impl Trait + use<'a>` 不同：use 是允许 hidden type 依赖哪些参数；`dyn Trait + 'a` 中的 `+ 'a` 是实际的 outlives 要求。

### 7. 默认对象 lifetime 有自己的规则

对于本章没有额外 trait lifetime 约束的例子：

| 出现在签名/类型别名中的写法 | 默认含义 |
|---|---|
| `&'a dyn Trait` | `&'a (dyn Trait + 'a)` |
| `Box<dyn Trait>` | `Box<dyn Trait + 'static>` |
| `Box<dyn Trait + 'a>` | 显式要求底层类型 outlive `'a` |

一般规则先看包含类型提供的唯一 lifetime 要求，再看 trait 的 lifetime bounds；没有这些线索时，表达式上下文可推断，而表达式之外采用 `'static` 默认。`'_` 请求按相应 lifetime elision 规则处理，不能把所有省略情况都解释成 `'static`。这是对象 lifetime 默认规则，不受上一章 RPIT 的 2024 默认 capture 规则支配。见 [Reference：Default trait object lifetimes](https://doc.rust-lang.org/reference/lifetime-elision.html#default-trait-object-lifetimes)。

```rust
fn borrowed<'a>(xs: &'a [u32]) -> Box<dyn Iterator<Item = u32> + 'a> {
    Box::new(xs.iter().copied())
}
```

迭代器内部实际保存输入借用，所以声明 `+ 'a`。即使每次产出的 Item 是拥有值的 u32，迭代器自身仍有借用。若把返回签名改成没有 lifetime 的 `Box<dyn Iterator<Item=u32>>`，便要求底层迭代器支持 `'static`，普通输入借用不满足。

### 8. 动态调用与 supertrait upcasting

一个对象指针可概念化为 `(data_ptr, vtable_ptr)`。动态调用选定方法槽，从中取出具体函数入口并传入数据指针；drop 和动态布局使用对应元数据。类型参数、lifetime、predicate 不是运行时交给 solver 的问题。

若 `trait Child: Base`，可把 `&dyn Child` coercion 为 `&dyn Base`：

```rust
trait Base { fn id(&self) -> u32; }
trait Child: Base { fn extra(&self) -> u32; }
fn upcast(x: &dyn Child) -> &dyn Base { x }
```

数据仍是原对象，但元数据可能需要调整；某些布局通过 `TraitVPtr` 找到 supertrait vtable，不能保证任意继承布局都只需复用原指针。这也不是向任意 trait 的转换或恢复具体类型的 downcast。

## 常见误区

- dyn 类型保存编译期接口约束；对象指针携带运行时分派元数据。
- `dyn Trait` 是 DST，`&dyn Trait` 和 `Box<dyn Trait>` 是 Sized 的指针形式。
- trait 自身泛型固定后可形成对象；方法自身的 type/const 泛型是另一项限制。
- `Self::Item` 可由对象等式绑定确定；裸 Self 仍表示擦除的实现类型。
- trait dyn-compatible 与每个方法可调用分别检查。
- `+ 'static` 是底层类型的 outlives 保证，不是对象值的销毁时刻。
- RPIT capture、object lifetime 与引用 lifetime 分层处理。
- vtable 是分派数据，不是 impl candidate 搜索缓存。

## 本章小结

构造对象时，编译器把具体实现对齐到 existential predicates，验证 dyn compatibility、trait/projection/auto-trait bounds 与 outlives。使用对象时，solver 利用对象自带的契约证明可用能力，运行时则由数据指针与 vtable 执行动态调用。

验证记录：2026-09-13 使用本机 `rustc 1.99.0-nightly (d453bdd8f 2026-08-14)`、`--edition=2024 -Znext-solver`，Source 动态调用、lifetime 泛型方法、Self: Sized 排除方法、固定 trait 类型参数、Self: Sized 排除 GAT、trait upcasting、两分支 Box 对象及借用型迭代器均编译运行通过。独立诊断测试确认泛型方法、返回 Self、普通关联 const、参与接口的 GAT 与 RPITIT 产生 E0038；返回借用迭代器而默认要求 `'static` 的示例产生 lifetime 诊断。源码讲解以当前工作区文件为准，本机 rustc 用于示例验证，并非声称它由当前工作区现场构建。
