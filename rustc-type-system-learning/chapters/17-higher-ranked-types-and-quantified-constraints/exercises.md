---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "17"
document: exercises
status: assigned
exercise_version: 1
updated_at: 2026-09-25
---

# 17. 习题

## 作答说明

E01–E04 每题 2 分，每小问 0.5 分，共 8 分。先按“谁是要证明的全称目标、谁是可选实例”判断，再写替换结果；变量名可自行命名，但须标明 universe 和同一/不同变量身份。题目中的 `P_a@U1` 表示 U1 中对应 bound `'a` 的 placeholder；`?r@U1` 表示 U1 中新建的 region infer var。除特别注明，按讲义中的类型关系模型分析，不要求枚举 coercion 的其余步骤。

## 题目

### E01. Binder、变量身份与实例化

比较下列两个类型：

```rust
for<'a> fn(&'a u32, &'a u32)
for<'x, 'y> fn(&'x u32, &'y u32)
```

1. 第一项的两个 region 使用分别指向几个 bound variable？在这一层 binder 内各是 `D0` 还是 `D1`？
2. 第二项有几个不同的 bound variable？不能只看 `ReBound` 的 depth，还要看什么？
3. 若分别作为待证明的全称目标打开，第一项两个位置得到几个 placeholder？第二项两个位置得到几个 placeholder？可都位于各自新建的同一 universe 吗？
4. 若把第一项作为可选实例使用，同一层 binder 由 `instantiate_binder_with_fresh_vars` 打开后，两个参数的 region 是同一个还是两个不同的 `ReVar`？说明原因。

### E02. HRTB trait goal、assumption 与 impl

```rust
trait Take<X> {}
struct Any;
impl<'a> Take<&'a u32> for Any {}
struct Static;
impl Take<&'static u32> for Static {}
```

1. 证明 `Any: for<'a> Take<&'a u32>` 时，goal 中 `'a` 先变成什么？泛型 impl 中 `'a` 又如何实例化？
2. 两侧匹配时，上述 impl 的新 region var 能否取 goal placeholder？指出其创建 universe 与关键的创建顺序。
3. `Static: for<'a> Take<&'a u32>` 能否由给出的 impl 证明？写出会不合法地要求的 region 关系。
4. 若 `ParamEnv` 中已有 `Any: for<'a> Take<&'a u32>` assumption，现在只需证明 `Any: Take<&'x u32>`，使用 assumption 时是把它打开为任意 placeholder，还是实例化为可选 infer var？最后该 infer var 可取什么？

### E03. 高阶子类型与函数参数逆变

只判断类型关系，记住共享引用对 region 协变、函数参数逆变。

1. 检查 `fn(&'static u32) <: for<'a> fn(&'a u32)` 时，哪一侧的 binder 打开为 placeholder？打开后需要证明哪一个引用类型的子类型关系？
2. 上一问最终需要哪条 outlives 关系？它对任意 placeholder 成立吗？因此整体结果是什么？
3. 反向检查 `for<'a> fn(&'a u32) <: fn(&'static u32)`，来源 binder 是任意 placeholder，还是可选 infer var？选什么实例可使关系成立？
4. 对 `for<'a> fn(&'a u32) <: for<'b> fn(&'b u32)`，按顺序写出右侧和左侧的实例化（含 universe）；左侧变量为什么能取右侧 placeholder？

### E04. 嵌套 binder、leak 与 solver 路径

对概念形态 `for<'a> fn(for<'b> fn(&'a u32, &'b u32))` 和当前源码作答。

1. 在内层 `for<'b>` 的函数参数中，`'a` 与 `'b` 分别处于哪个 de Bruijn depth？如果先后打开两层量词，各自可位于哪个新 universe？
2. 预先在 U0 创建的 `?r0@U0` 能否以 U1 的 `P_a` 为解？若在 U1 创建 `?r1@U1`，它能否引用 `P_a`？用 `can_name` 解释。
3. 当前 `EvalCtxt::compute_goal` 对带 binder 的目标先调用哪个方法？`TraitGoal::match_assumption` 对带 binder 的 assumption 又调用哪个实例化方法？
4. 当前 next solver 构造 canonical response 前，在 `assumptions_on_binders()` 为 `false` 和 `true` 两条分支中分别如何处理 placeholder？为什么不能说“所有高阶错误都只靠早期 leak check 判定”？

## 学习者答案

### E01

待作答。

### E02

待作答。

### E03

待作答。

### E04

待作答。
