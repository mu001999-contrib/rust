---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "13"
document: exercises
status: submitted
exercise_version: 2
updated_at: 2026-09-06
---

# 13. 习题

## 作答说明

四题，每题 2 分，每小问 0.5 分，共 8 分。可使用概念化 IR；忽略隐式 sizedness 等题目未单列的条件。

E01–E04 原题保持不变，原答已保存。第 2 版新增 E05 定向复核，不独立增加满分；用于更新对应小问的当前评分。

## 题目

### E01. Projection 的 parent 与 own args

```rust
trait Family<P> {
    type Out<'a, Q, const N: usize> where Self: 'a, P: 'a;
}
```

考察 `<Wrap<u16> as Family<u32>>::Out<'x, bool, 4>`：

1. 完整 projection args 按槽位顺序是什么？
2. 提取出的 trait-ref 是什么？
3. own args 是什么？只调用 `trait_ref()` 会丢掉哪部分？
4. projection 关联项标识指向 trait 的 Out 声明，还是已经选好的 impl Out 定义？

### E02. 找到 impl 后实例化 RHS

接上题：

```rust
struct Wrap<T>(T);
impl<T, P> Family<P> for Wrap<T> {
    type Out<'a, Q, const N: usize> = (&'a T, &'a P, [Q; N])
    where Self: 'a, P: 'a;
}
```

1. 匹配后 impl parent args `[T, P]` 是什么？
2. rebase 后 impl 关联项的完整 args 是什么？
3. 实例化的 RHS 是什么？
4. 本次 normalization 需要验证的两个显式 GAT outlives 条件是什么？源码添加这类 own predicates 时用什么 GoalSource？

### E03. GAT 的前提与保证

```rust
trait Lend {
    type Item<'a>: Clone + 'a where Self: 'a;
    fn lend<'a>(&'a self) -> Self::Item<'a>;
}
```

1. `where Self: 'a` 是 GAT 使用前提，还是指定 Item 的具体类型？
2. `: Clone + 'a` 约束 Self，还是这份 Item 投影？
3. 在 `fn duplicate<'a, L: Lend>(value: &'a L)` 中，为什么即使不知道 Item 的 RHS，也可以对 `value.lend()` 的结果调用 clone？
4. `where Self: 'a` 是否意味着 Self 必须为 `'static`？若实现 RHS 为 `&'a T`，这个前提怎样帮助检查其良构性（Self = Cell<T>）？

### E04. 环境、抽象投影与 normalization

1. `T: Iterator<Item = u32>` 概念上提供哪两类 clause？哪一条直接约束 Item 的输出？
2. 如果只知道 `T: Iterator`，能否推出 Item = u32？此时投影能否保持抽象？
3. 为什么 `normalize_associated_term` 要先创建独立的 unconstrained output variable，再将结果与 expected term 做 relation？
4. 假设 `trait Map { type Out<X>; }` 且 `impl Map for () { type Out<X> = u8; }`，知道 `<() as Map>::Out<?X> == u8` 能否唯一确定 X？说明原因。

### E05. 参数、前提与 normalization 定向复核

1. 对 E01–E02 的具体投影，写出本次 GAT own args，以及 RHS `(&'a T, &'a P, [Q; N])` 实例化后的完整类型。
2. `where Self: 'a` 是使用前提还是具体类型定义？当 Self = Cell<T>、RHS = `&'a T` 时，请补全 `Cell<T>: 'a → …… → &'a T 良构`。
3. normalization 登记 GAT 的 own predicates 时，使用哪个 GoalSource？它检查使用前提还是提供输出的 Clone 等保证？
4. 即使已经找到唯一 impl，为什么内部 NormalizesTo 仍使用独立的 unconstrained output，而不直接拿 Expected 作为输出参数？

## 学习者答案

### E01

> 1. [Wrap\<u16>, u32, 'x, bool, 4]; 2. Wrap\<u16>: Family\<u32>; 3. ['a, Q, N];, 丢掉 GAT own args; 4. 指向 trait 的 Out 声明。

### E02

> 1. [u16, u32]; 2. [u16, u32, 'x, bool, 4]; 3. (&'x u16, &'x bool, [bool; 4]); 4. Wrap\<u16>: 'x, u32: 'x, ItemBounds。

### E03

> 1. 指定 Item 的具体类型；2. 约束 Item 投影；3. 因为 lent 返回 Self::Item<'a> 使用的是 &'a self 中的 'a，并且约束了 Self::Item<'a>: Clone；4. 不是，要求检查 Cell\<T>: 'a，满足 wf。

### E04

> 1. trait 和 projection，第二条；2. 不能，可以；3. 因为不知道 assoc term 具体是哪个 impl 里的；4. 不能，因为不具备单射，u8 并不和某一个具体的 type 关联。

### E05

> 1. ['x, bool, 4], (&'x u16, &'x u32, [bool, 4])；2. 是使用前提，Cell\<T>: 'a -> T: 'a -> &'a T: 'a wf；3. AliasWellFormed, 检查使用前提，输出的 Clone 等保证在 impl 中证明；4. 因为不应该让 expected item 干扰候选
