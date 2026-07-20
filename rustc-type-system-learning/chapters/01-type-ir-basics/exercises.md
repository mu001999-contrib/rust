---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "01"
document: exercises
status: submitted
exercise_version: 2
updated_at: 2026-07-21
---

# 01. 习题

## 作答说明

本章包含三轮形成性练习。学习者答案按对话原文保存；E01 同时保留首次提交和经过追问后的修订提交。评分以修订提交及后续澄清所体现的最终理解为依据，不以讲评内容覆盖原答案。

## 题目

### E01. Type IR 表示、相等与 `_` 的 lowering

1. 对于：

   ```rust
   struct Matrix<T, const ROWS: usize, const COLS: usize>(
       [[T; COLS]; ROWS]
   );
   ```

   分别写出定义内部字段类型和 `Matrix<u8, 3, 4>` 的近似 Type IR。

2. 判断以下比较为什么不能只用 `==`：

   ```text
   ?0t                         与 u32
   <T as Iterator>::Item       与 u32
   Vec<?0t>                    与 Vec<u32>
   ```

3. 对 `Matrix<_, 3, 4>` 中的 `_`，说明它在哪个阶段是 ambig，在哪个阶段会成为 `TyKind::Infer`，以及编译器依靠什么信息完成这个转换。

### E02. 四类变量及其阶段转换

判断下面各处应该主要出现哪种表示，并说明原因：

1. 检查泛型函数体时的 `T`：

   ```rust
   fn f<T>(x: T) {}
   ```

2. 调用 `f(Default::default())`，尚未得到其他约束时，实参和泛型参数对应的类型。
3. `for<'a> fn(&'a u32)` 尚未打开 binder 时的 `'a`。
4. 为了证明一个 higher-ranked goal，调用 `enter_forall` 后的 `'a`。
5. trait query canonicalization 前的 `?0t`，以及 canonicalization 后代表它的变量。

判断以下说法是否成立：

1. `Param(T)` 可以直接通过 unification 变成 `u32`。
2. `Placeholder` 可以像 inference variable 一样被赋值。
3. `Infer(?0t)` 的求解状态存储在 interned `Ty` 内部。
4. `Bound` 可以安全地脱离其 binder 出现在任意 `Ty` 中。

### E03. `TypeFlags`

分别判断这些类型至少包含哪些主要 flags：

```text
A. Vec<u32>

B. Vec<T>

C. Vec<?0t>

D. Vec<<T as Iterator>::Item>

E. &'?0r Vec<?1t>

F. Binder<for<'a> fn(&'a T)>
```

主要考虑：

```text
HAS_PARAM
HAS_INFER
HAS_ALIAS
HAS_PLACEHOLDER
HAS_BOUND_VARS
HAS_FREE_REGIONS
HAS_FREE_LOCAL_NAMES
```

### E04. 快速路径

分别判断以下操作是否能立即返回原值，并说明原因：

```text
1. resolve_vars_if_possible(Vec<T>)

2. resolve_vars_if_possible(Vec<?0t>)

3. normalize(Vec<u32>)

4. normalize(Vec<<T as Iterator>::Item>)
```

### E05. Binder 与 `outer_exclusive_binder`

判断 `inner = &'a u32` 在以下两个位置是否有 escaping bound vars：

```text
A. Binder<for<'a> &'a u32>

B. Binder::skip_binder() 后得到的 &'a u32
```

并分别给出近似的 `outer_exclusive_binder`。

## 学习者答案

### E01

首次提交：

> 1. TyKind::Array(Ty(TyKind::Array(Ty(TyKind::Param(ParamTy(index 0, name: T))), Const(ConstKind::Param(ParamConst(index 2, name: COLS))))), Const(ConstKind::Param(ParamConst(index 1, name: ROWS))))，TyKind::ADT(AdtDef(Matrix), [Ty(u8), Const(Value(3)), Const(Value(4))])；2. 第一个是推理可统一，第二个是可归一相等，第三个也是推理可统一，但是直接 == 都不想等；3. 在 lowering HIR 的时候吗，然后在 type infer 的时候完成类型确定

经过关于 `AmbigArg`、`GenericArg::Infer` 和 lowering 边界的追问后，修订提交：

> 1. TyKind::Array(Ty(TyKind::Array(Ty(TyKind::Param(ParamTy(index 0, name: T))), Const(ConstKind::Param(ParamConst(index 2, name: COLS))))), Const(ConstKind::Param(ParamConst(index 1, name: ROWS))))，TyKind::ADT(AdtDef(Matrix), [Ty(u8), Const(Value(3)), Const(Value(4))])；2. 第一个是推理可统一，第二个是可归一相等，第三个也是推理可统一，但是直接 == 都不想等；3. lowering 到 hir ty 的时候是 ambig，然后在 lowering 到 mid ty 的时候，会根据 generic param 的定义成为 TyKind::Infer，然后确定具体类型。

### E02

> Cool，所以是类型推断后有个将类型推断的结果写回的操作，覆盖掉之前的 infer vars！现在我来做练习：1. Param；2. 实参为 ?T，泛型参数为 Param T；3. ReBound；4. ReLateParam；5. Infer, 变量是啥？；判断题：不能，需要经过通过 infer var 通过 type inference；不能，placeholder 表示所有，不能赋值为某个；不对，存在在 typeck_results 中然后回写，不会修改原来的 interned Ty；Bound 不能够脱离 binder

### E03

> 练习一：A. 都不包含；B. HAS_PARAM；C. HAS_INFER；D. HAS_PARAM，HAS_ALIAS；E. HAS_INFER, HAS_FREE_REGIONS，6. HAS_BOUND_VARS。

### E04

> 练习二：1 和 3 会理解返回原值，因为 1 没有 infer vars，3 没有 alias。

### E05

> 练习三：A 没有，outer_exclusive_binder = 0，B 有， outer_exclusive_binder = 1。

## 提交记录

| 日期 | 轮次 | 说明 |
|---|---:|---|
| 2026-07-21 | 1 | E01 首次提交和修订提交。 |
| 2026-07-21 | 2 | E02 四类变量与阶段转换。 |
| 2026-07-21 | 3 | E03–E05 TypeFlags、快速路径与 binder 缓存。 |
