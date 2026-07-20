---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "16"
document: exercises
status: completed
exercise_version: 3
updated_at: 2026-09-25
---

# 16. 习题

## 作答说明

E01–E04 每题 2 分，每小问 0.5 分，共 8 分。前两题按稳定 Rust 和当前 Type IR 作答；后两题以当前 `generic_const_args` 实验路径作答，并明确其特性与 next solver 条件。`ConstKind` 可以用概念化写法，不要求精确展开 binder 和 interned 类型。

E05、E06 是针对原题的定向复核，不独立增加总分；复答用于更新对应小问的当前评分。

## 题目

### E01. 把 const 放回 Type IR

```rust
struct Buffer<T, const N: usize>([T; N]);
fn keep<const N: usize>(xs: [u8; N]) -> [u8; N] { xs }
```

1. `Buffer<T, N>` 的 GenericArgs 各是什么 kind？其字段 `[T; N]` 的 TyKind 及长度 ConstKind 分别可怎样表示？
2. 在 `keep` 的泛型签名中，N 是 `ConstKind::Param`、`Infer`、还是 `Value`？给 `keep::<3>` 提供实参后，哪个事实变具体了？
3. 若调用处对常量实参使用 `_` 请求推断，应引入哪类常量推理变量？它和签名中的 Param N 有何区别？
4. `ConstKind::Value` 除具体值的 `ValTree` 之外还保存什么？为什么类型系统不用 CTFE 的原始 `AllocId` 直接比较值？

### E02. Alias、投影与常量约束

```rust
trait Capacity { const CAP: usize; }
struct Fixed<const N: usize>;
impl<const N: usize> Capacity for Fixed<N> { const CAP: usize = N; }
```

1. `<Fixed<3> as Capacity>::CAP` 在求值前对应哪个 `AliasConstKind`？关联常量的 DefId 与 args 各发挥什么作用？
2. 对 `struct Flag<const B: bool>;`，尝试传入 `3_usize` 需要检查哪类 clause？它检查的是什么？
3. 对匿名常量 `{1 + 2}`，说明待求值时与得到数值 3 后可分别对应的 `ConstKind`；哪一步才能得到类型系统可用的 `Value`？
4. 在 next solver 中，两个 const 的关系遇到一个非 rigid const alias 而需要延后时，当前 `combine_consts` 注册什么目标？是否把旧路径的 `ConstEquate` 当成 next solver 的通用入口？

### E03. 具名 const 与 `generic_const_args`

```rust
#![feature(generic_const_items, min_generic_const_args, generic_const_args)]
#![expect(incomplete_features)]

use std::gca;

const ADD1<const N: usize>: usize = N + 1;

struct PlusOne<const N: usize> {
    data: [u8; gca!(ADD1::<N>)],
}
```

1. 为什么稳定写法 `[u8; N]` 可用，而这个例子把 `N + 1` 移入具名 const item？`PlusOne` 的长度在抽象 N 下可概念化为哪类 `ConstKind`？
2. `ADD1::<N>` 的 DefId 与 args 分别是什么？`gca!` 在当前 `min_generic_const_args` 路径中起什么作用？
3. 为什么这里无需 `where [(); N + 1]:`？该前提主要对应哪条实验性实现路径？
4. 对抽象 N，类型中两次引用 `ADD1::<N>` 可凭什么建立相等？把其中一处换成另一个定义但右侧同样是 `N + 1` 的 const item，是否仍可直接这样判断？

### E04. 求值、rigid alias 与支持边界

1. `ConstArgHasType(C, usize)`、`ConstEvaluatable(C)`、`C == D` 分别回答什么问题？
2. 在 GCA 的 const projection 归一化中，`evaluate_const` 暂时返回 `None` 时：若 alias 仍含未解出的非 region 推理变量，certainty 如何？若没有这类变量，求解器如何表示原始 alias？
3. `ConstKind::Expr` 主要对应哪条特性路径？开启 `generic_const_args` 后，直接写含泛型参数的匿名 `{ N + 1 }` 是否就是本章的主机制？
4. 在有具体泛型实参后，哪个查询入口可解析并求值 MIR `UnevaluatedConst`？若仍太泛化，可能得到什么结果？

### E05. const alias 与求值边界定向复核

1. 在未启用 `macroless_generic_const_args` 的例子中，`gca!(ADD1::<N>)` 对应哪个 HIR `ConstArgKind`、哪个 Type IR `ConstKind`/`AliasConstKind`？具体匿名 `{ 1 + 2 }` 在求值前、后又可能分别是什么？
2. next solver 的 `combine_consts` 遇到非 rigid const alias、关系需要延后时注册什么？`ConstEquate` 对应哪条实验性路径？
3. GCE 的 `where [(); N + 1]:` 提供的是什么前提？它是否表示 lowering 时已经求出 `N + 1` 的数值？GCA 为什么可以不写这一前提？
4. GCA const projection 的 `evaluate_const` 返回 `None` 时，仍有未解非 region 推理变量与没有这类变量，两种结果各是什么？`ConstKind::Expr` 属于 GCE 还是 GCA？
5. `const_eval_resolve` 在泛型实参仍太泛化时，返回哪个具体错误变体？

### E06. 剩余三个边界的简短复核

1. 对具体匿名 `{ 1 + 2 }`，依次写出 HIR const 实参、求值前 Type IR、求值后 Type IR 的节点名称。
2. `combine_consts` 中，next solver 遇到非 rigid const alias 时注册什么？旧 GCE 分支注册什么？前者是否是 GCA 专有？
3. GCA const projection 的 `evaluate_const` 返回 `None` 且没有未解非 region 推理变量时，原始 alias 以哪种 `IsRigid` 状态与 expected 建立关系？后续 goals 成立时 certainty 是什么？

## 学习者答案

### E01

> 1. Type，Const，Array(Ty::Type; Ty::Const), ConstKind::Param；2. N 是 ConstKind::Param，N 变具体了；3. Const::Infer, Param N 指向具体的泛型参数，Infer 则是推断常量；4. 还保存类型，因为值相等 AllocId 不一定相等。

### E02

> 1. Projection, 用于查找 impl 和实例化；2. 检查 3\_usize: bool，检查的是类型是否匹配；3. 之前是 AnnoConst，之后是 Value，第二步；4. 讲义里没有吧。

### E03

> 1. 因为涉及到 N + 1，这个例子用 gca! 的写法，延迟常量求值，抽象为 Alias Free；2. DefId 是 ADD1，args 是 N，gca! 的作用是标记这里是直接表示 const 实参，不 lower 到 AnnoConst，这样在后续 lower 到 typed ir 的时候，则是 ty::Const ConstKind Alias 而不是 Value；3. 因为不需要调用者证明 N + 1 可计算，而等到 N 确定的时候再确定可求值就可以了，该前提对应的是 generic\_const\_exprs，因为 gce 需要在 lower 的时候就求值，但是此时无法确定，需要用户先声明前提后跳过；4. 凭定义相等，无法直接判断。

### E04

> 1. C: uszie, C 可求值，C 和 D 相等；2. Certainty 为 AMBIGUOUS，不能判定无解，没有这类变量则直接 Certainty::Yes，NoSolution；3. 对应 generic\_const\_args，是的；4. const\_eval\_resolve，得到 Error。

### E05

> 1. ConstArgKind::Path，Const Alias Free，求值前 AnnoConst，求值后 ty::Const Value；2. 注册 ProjectionClause，对应 GCA；3. 提供可求值前提，不表示已经求出，因为 GCA 可以保持抽象 N 下未求值，而 GCE 的设计依赖外部声明的前提；4. 分别是 Certainty::AMBIGUOUS 和 Certainty::Yes，属于 GCE，GCA 在这个阶段只会产生 Alias；5. 返回 TooGeneric。

### E06

> 1. AnnoConst, Alias(Anno),  ty::Const Value；2. 注册 `ProjectionClause`，注册 `ConstEquate`，不是；3. `IsRigid`::Yes，成立时 Certainty::Yes
