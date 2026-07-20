---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "14"
document: exercises
status: completed
exercise_version: 2
updated_at: 2026-09-06
---

# 14. 习题

## 作答说明

四题，每题 2 分，每小问 0.5 分，共 8 分。以本章 edition 2024 示例与当前 next solver 为准；可用概念化名称表示 opaque DefId。

## 题目

### E01. APIT、RPIT 与身份

1. `fn take(x: impl Clone)` 与 `fn make() -> impl Clone`，分别由调用方还是定义方选择具体类型？
2. 两个不同函数均返回 `impl Iterator<Item=u32>`，函数体也均为 `0..3`，它们是否因此具有同一个 opaque identity？
3. 同一 RPIT 函数的一条分支返回 Range<u32>，另一条返回 Vec<u32>::IntoIter，是否只要都实现 Iterator 就可通过？
4. `OpaqueTypeKey` 的两个字段是什么？hidden type 是 key 本身还是另外保存的值？

### E02. 定义性使用与 bounds

1. Typeck 中，若某本地 opaque 不在 defining 集合，能否根据任意 expected type 重新定义它？当前求解器怎样处理该 alias？
2. 同一 opaque key 已登记 hidden type u32，新的定义性使用要求 bool，源码需要进行什么类型关系检查？
3. 登记 hidden type 后，为什么仍要调用 `add_item_bounds_for_hidden_type`？产生的 goals 使用什么 GoalSource？
4. 为什么第 13 章 NormalizesTo 要隔离 expected，而本章 opaque 在定义域内却允许当前 relation 的另一侧约束 hidden type？两者任务有何区别？

### E03. 捕获与 outlives

```rust
fn chars<'a>(s: &'a str) -> impl Iterator<Item=char> + use<'a> { s.chars() }
fn length<'a>(s: &'a str) -> impl Copy + use<> { s.len() }
```

1. 两个 hidden type 分别是什么？它们是否实际使用输入 lifetime？
2. `use<'a>` 与 `+ 'a` 分别表达什么？
3. 本例 length 的返回值能否在输入 String 销毁后继续存在？为什么？
4. 若回到 edition 2021，free function 的 lifetime 默认捕获是否与 2024 一律相同？说明 relevant 区别。

### E04. TAIT、RPITIT 与展开

1. 当前 nightly 的 `type Numbers = impl Iterator<Item=u32>` 需要哪个 feature？相关定义函数用什么属性声明定义性使用？
2. `fn take(x: Numbers)` 是否等价于接受任意 `impl Iterator<Item=u32>`？
3. trait 方法里的 RPITIT 在编译器中与哪类 synthetic item 关联？不同 Self 的不同 impl 能否提供不同返回类型？
4. Codegen 是否仍完全不知道 hidden type？当前 `normalize_opaque_type` 主要通过哪个 query 并结合什么参数获得实际类型？

### E05. 定向复核

本题复核 E02–E04 的对应判断点，不另加总分；通过后更新对应得分。

1. `add_item_bounds_for_hidden_type` 添加 goals 使用哪个 `GoalSource`？随后选择 impl 并产生它的 where 条件时，这些子 goals 又使用哪个来源？
2. edition 2024 中，把 `length` 的返回声明改成仅 `impl Copy`，虽然实际返回的仍是 usize，调用方还能仅凭“没有写 `+ 'a`”保证结果可以在输入 String 销毁后继续使用吗？解释 capture 与 outlives 的区别。
3. 对没有显式 `use<...>` 的 free function RPIT，若 `'a` 未出现在返回 bounds 中，2021 和 2024 分别会不会默认捕获它？
4. 写出 TAIT 的完整 feature 名和针对 `Numbers` 的定义属性；RPITIT 对应的 synthetic item 的准确类别是什么？

## 学习者答案

### E01

> 1. 调用方，定义方；2. 不具有同一个；3. 否；4. def\_id 和 args，另外保存的值。

### E02

> 1. 不能，rigid yes，不会进一步求解；2. 相等；3. 因为需要证明 hidden type 满足 opaque 的约束，ImplWhereBound；4. 因为 NormalizesTo 选择后续时不能受到 expected 影响， 而 opaque 需要根据定义性使用决定 hidden type。

### E03

> 1. Chars<'a>，usize，第一个实际使用，第二个则不；2. use<'a> 表示捕获该泛型参数，+ 'a 则是要求 hidden type outlives 'a；3. 可以，因为没有要求 outlives 'a；4. 不相同，2021 默认捕获。

### E04

> 1. 需要 TAIT，#[define\_opaque]；2. 不等价；3. lifetime GAT，可以；4. codegen 时需要知道具体的 hidden type，type\_of 和 def\_id，然后通过 opaque args 实例化，再通过 normalize 归一化。

### E05

> 1. AliasWellFormed, ImplWhereBound; 2. 可以保证，因为 hidden type 是 usize；capture 是捕获泛型参数，可以在 hidden type 中引用，outlives 则是通过 + 'a 要求 hidden type 满足 :'a；3. 2021 不会默认捕获，2024 会默认捕获；4. #![feature(type\_alias\_impl\_trait)]，#[define\_opaque(Numbers)]，DefKind::AssocTy

#### E05.2 复答

> 2. 不能继续使用，因为会默认 capture 'a。
