---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "06"
document: grading
status: completed
exercise_version: 1
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-08-01
---

# 06. 评分与反馈

## 总评

修正后得分 `8 / 8`（100%），掌握度为 `mastered`。首轮得分为 `7 / 8`；顺序合并复核的四项答案全部正确，因此 E01、E04 各补足 0.5 分。`sub/sup/eq` 的 API 方向、嵌套 `xform`、definition-site variance、ADT 参数递归、type-var subtype obligation、coercion、region constraint 与 contravariant lattice 均已掌握。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 修正后正确 | 首轮已正确回答 `sub/sup/eq`、调用检查方向、引用 subtype 与 outlives；复核中正确写出 `make_subregion(sub='inner, sup='outer)`。 |
| E02 | 2 | 2 | 正确 | 四条嵌套路径均正确；`&'a mut fn(T)` 对 pointee invariant，但对外层引用 lifetime `'a` covariant。 |
| E03 | 2 | 2 | 正确 | `Packet` 三个参数依次为 `+/-/o`；使用点关系和 `Vec` subtype obligation、`Cell` equality 均正确。 |
| E04 | 2 | 2 | 修正后正确 | 首轮已掌握 coercion、一般类型的 LUB/GLB 和函数输入处的 lattice 反转；复核中由 `X <: Y` 正确推出 `LUB(X, Y) = Y`、`GLB(X, Y) = X`。 |

## 逐题解释

### E01：type relation 与 region constraint 使用相反的参数书写视角

前三个 API 方向正确：

```text
sub(E, A) -> E <: A
sup(E, A) -> A <: E
eq(E, A)  -> E == A
```

已知：

```text
&'a T <: &'b T
```

需要：

```text
'a: 'b
points('b) ⊆ points('a)
```

`make_subregion` 的签名按 `(sub, sup)` 排列，所以调用为：

```text
make_subregion('b, 'a)
```

### E02：嵌套 variance

答案全部正确。补全根 ambient 后分别为：

```text
fn(*const Vec<T>)   : + xform - xform + xform + = -
fn(fn(T))           : + xform - xform - = +
fn() -> *mut Vec<T> : + xform + xform o xform + = o
&'a mut fn(T)       : + xform o xform - = o
```

`&'a mut _` 的 `'a` 不经过 mutable pointee 的 invariant 边，因此为 `+`。

### E03：ADT variance 与 type vars

答案全部正确：

```text
Packet<A, B, C> 的 variances = [+, -, o]

Packet<A1, B1, C1> <: Packet<A2, B2, C2>
  -> A1 <: A2
  -> B2 <: B1
  -> C1 == C2
```

`Vec` 的 covariant edge 保留 `?T0 <: ?T1` 的方向，旧 `TypeRelating` 为两个未决 type vars 注册 subtype obligation；`Cell` 的 invariant edge 将关系变为 equality，因此 equate。

### E04：type lattice 中先固定 subtype 方向

coercion 的判断正确。更具体地说，三个例子分别需要 reborrow/mutability weakening、unsize 与 fn-item-to-fn-pointer adjustment；纯 subtype relation 不插入表达式 adjustments。

已知：

```text
'long: 'short
```

首先得到类型关系：

```text
&'long T <: &'short T
```

套用已经正确回答的一般规则 `A <: B`：

```text
LUB(A, B) = B
GLB(A, B) = A
```

因此：

```text
LUB(&'long T, &'short T) = &'short T
GLB(&'long T, &'short T) = &'long T
```

函数输入处反转 lattice operation 的结论正确：若 `A <: B`，则 `GLB(A, B) = A`，所以：

```text
LUB(fn(A) -> R, fn(B) -> R) = fn(A) -> R
```

## 已掌握概念

- `sub/sup/eq` 的关系方向与 expected/actual 角色。
- 函数输入、mutable pointee 与嵌套类型的 variance 手算。
- definition-site variance 到 use-site relation 的参数分解。
- subtype-related TyVars 与 equality union 的边界。
- coercion adjustments 与纯 relation 的边界。
- contravariant 位置交换 LUB/GLB。

## 顺序合并复核结果

学习者修正答案：

```text
1. X <: Y
2. LUB(X, Y) = Y
3. GLB(X, Y) = X
4. 'outer: 'inner, sub = 'inner, sup = 'outer
```

四项全部正确。统一推导链为：

```text
'outer: 'inner
  -> &'outer T <: &'inner T
  -> X <: Y
  -> LUB(X, Y) = Y
  -> GLB(X, Y) = X
  -> make_subregion('inner, 'outer)
```

## 已完成的补充练习

完成以下合并复核题：

```text
已知 'outer: 'inner，令：

X = &'outer T
Y = &'inner T
```

回答：

1. `X` 与 `Y` 的 subtype 方向；
2. `LUB(X, Y)`；
3. `GLB(X, Y)`；
4. 若调用 `sub(X, Y)`，产生的 outlives 关系，以及 `make_subregion(sub, sup)` 的两个实参。

## 完成判定

- 修正后总分为 `8 / 8`（100%）。
- 引用 subtype、LUB/GLB 与 region constraint 参数顺序均已掌握。
- 当前章节状态：`completed`；掌握度：`mastered`。

## 复核记录

| 日期 | 原因 | 结论 |
|---|---|---|
| 2026-07-28 | 完成讲授、源码精读并发布 E01–E04 | 等待提交。 |
| 2026-08-01 | 评阅 E01–E04 | `7 / 8`；安排 region constraint 与引用 LUB/GLB 顺序合并复核。 |
| 2026-08-01 | 评阅顺序合并复核 | 四项全部正确；E01、E04 更新为 2/2，总成绩更新为 8/8，章节判定 `mastered`。 |
