---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "15"
document: exercises
status: completed
exercise_version: 2
updated_at: 2026-09-21
---

# 15. 习题

## 作答说明

四题，每题 2 分，每小问 0.5 分，共 8 分。按讲义的普通 Rust 规则和当前 next solver 作答，不启用实验性关联 const 等特性。说明关键理由；IR 可使用省略 binder 包装的概念表示。

## 题目

### E01. 对象的 Type IR

考虑 `dyn Iterator<Item = u32> + Send + 'obj`：

1. 三项 existential predicates 分别是什么？`'obj` 保存在何处？
2. Iterator 的 existential trait ref 的 args 是 `[]` 还是 `[Self]`？与普通 TraitRef 有何区别？
3. 将 trait bound 用 `with_self_ty` 补入具体类型 S 后，得到什么 trait predicate？projection bound 对应什么等式？
4. 函数只知道 `D = dyn Iterator<Item=u32>`，证明 `<D as Iterator>::Item == u32` 时，可从哪里取得依据？需要猜测其底层具体类型吗？

### E02. Dyn compatibility 与可调用方法

```rust
trait Source {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    fn label<'a>(&self, text: &'a str) -> &'a str { text }
    fn build() -> Self where Self: Sized;
    fn inspect<T>(&self, _: T) where Self: Sized {}
}
```

1. 能否构造 `dyn Source<Item=u32>`？next 的返回类型如何确定？
2. label 的 lifetime 泛型是否阻止动态调用？build 和 inspect 能否通过该对象调用？
3. 去掉 inspect 上的 `where Self: Sized` 会怎样？为什么 `dyn Transform<u32>` 中固定 trait 泛型实参不是相同的问题？
4. 把 build 的逐项 Sized 约束换成整个 trait 的 `trait Source: Sized`，还能构造 dyn 对象吗？说明 receiver 指针 Sized 与 Self Sized 的区别。

### E03. 引用 lifetime 与对象 lifetime

假设 Trait 是 dyn-compatible 且没有额外的 lifetime supertrait bounds：

1. `&'r (dyn Trait + 'obj)` 中两个 lifetime 分别约束什么？S 转换到对象时需要满足什么 type-outlives predicate？
2. 在函数返回签名中，`Box<dyn Iterator<Item=u32>>` 默认对象 lifetime 是什么？为什么不能直接返回借用普通输入 slice 的迭代器？
3. 对 `fn borrowed<'a>(xs: &'a [u32]) -> ...`，补出允许返回借用型 boxed iterator 的完整返回类型。它与 `impl Iterator + use<'a>` 中的 use 分别表达什么？
4. `Box<dyn Trait + 'static>` 的值能否在局部作用域结束时销毁？底层是拥有数据的 String 时，为什么可能满足 `'static`？

### E04. 构造证明、vtable 与 upcasting

1. 将 Sized 类型 S 转为 `dyn Iterator<Item=u32> + Send + 'obj`，列出主要 trait、projection、Sized 和 outlives 要求。
2. 动态调用时是否重新执行 impl candidate 搜索？对象指针中分别用什么定位数据和方法入口？
3. `VtblEntry` 中除了 Method，还包含哪三项公共 header 信息？这些布局细节是否是稳定 Rust ABI 承诺？
4. 当 `Child: Base` 时，`&dyn Child` 能否转换成 `&dyn Base`？数据是否需要变成另一个具体类型？vtable 元数据是否保证始终不变？

### E05. 定向复核

本题复核 E01、E02、E04 的对应判断点，不另加总分；通过后更新对应原题得分。

1. 把 `dyn Iterator<Item=u32> + Send + 'obj` 写成 `TyKind::Dynamic(?, ?)`：三个 existential predicates 放在哪里，`'obj` 放在哪里？函数只有 `D = dyn Iterator<Item=u32>` 时，`<D as Iterator>::Item = u32` 的候选来自对象本身还是必须来自 `ParamEnv::caller_bounds`？
2. 若删掉 `inspect<T>` 的逐项 `where Self: Sized`，`Source` 整体还能构造 dyn 对象吗？固定的 `dyn Transform<u32>` 为什么不受同一限制？
3. 将 S 转成 `dyn Iterator<Item=u32> + Send + 'obj`，写全 trait、projection、auto trait、Sized 和 type-outlives 要求。成功构造后的对象指针，哪个部分定位数据，哪个部分定位方法入口？

## 学习者答案

### E01

> 1. Trait(Iterator, []), Projection(Iterator::Item, [], u32), AutoTrait(Send), Outlives('obj); 2. 是 args，没有 Self；3. 得到 TraitRef，对应 \<S as Iterator>::Item == u32；4. 直接从 param env，不需要猜测具体类型。

### E02

> 1. 可以构造，由 dyn 类型中的 Item=u32 确定；2. 不会阻止动态调用，build 和 inspect 不能动态分发；3. 去掉后也不能调用，因为有 generics，因为 trait 上的泛型是可以的，trait 上的泛型在引用 dyn Trait\<T> 的时候就确定了；4. 不能，会破坏整个 trait 的 dyn-compatible。receiver 指针是固定大小的胖指针，而 Self Sized 要求 Self 类型本身的大小是确定的。

### E03

> 1. 'r 约束引用的生命周期，'obj 约束对象的寿命周期。需要满足 S: 'obj；2. 默认 lifetime 是 'static，因为不满足 outlive 要求；3. Box\<dyn Iterator\<Item=u32> + 'a>。前者表达要求对象 outlives 'a，后者要求捕获 lifetime generic；4. 可以销毁，因为 String: 'static 成立。

### E04

> 1. trait(Iterator, args = []), projection(Iterator::Item, Item = u32, args=[]), Sized(S: Sized), outlives(S: 'obj)；2. 不重新执行，通过 vtable；3. drop/size/align，非稳定 ABI 承诺；4. 可以的，数据的具体类型不变。vtable 元数据不能保证始终不变。

### E05

> 1. 放在第一个 ？，放在第二个？，候选来自对象 Dynamic 表示本身；2. 不能够。因为 u32 出现在 trait 本身的泛型；3. trait(Iterator, args=[]), projection(Iterator::Item, Item=u32, args=[]),autotrait(Send), Sized(S: Sized), Outlives(S: 'obj)。data\_ptr 指向数据，vtable\_ptr 指向方法入口
