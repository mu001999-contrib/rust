---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "09"
document: exercises
status: submitted
exercise_version: 1
updated_at: 2026-08-12
---

# 09. 习题

## 作答说明

本轮共四题，每题 2 分，共 8 分。请同时写出 IR、归一化来源以及产生的 obligations；可使用概念化表示，不要求填写真实 `DefId` 数值。

## 题目

### E01. Alias IR 与 GenericArgs

考虑：

```rust,ignore
trait Convert<A> {
    type Out<B>;
}

type Pair<T> = (T, T);
```

回答：

1. `<S as Convert<u32>>::Out<bool>` 对应哪个 `AliasTyKind`？其 `def_id` 指向 trait 还是 `Convert::Out`？
2. 该 `AliasTy` 的 `args` 可概念化为何？
3. 在普通稳定路径中，`Pair<u8>` 通常保留为 `AliasTyKind::Free`，还是在 lowering 时成为 `(u8, u8)`？
4. 当前源码中，课程规划里的 weak alias 对应哪个 kind 名称？

### E02. ParamEnv projection candidate

考虑：

```rust,ignore
fn next_u32<T>(x: T)
where
    T: Iterator<Item = u32>,
{
    // 使用 <T as Iterator>::Item
}
```

回答：

1. 这个 where-clause 可概念化为哪两条 caller bounds？
2. 哪一条证明 `T` 实现了 `Iterator`？
3. 哪一条能把 `<T as Iterator>::Item` 归一化为 `u32`？
4. old solver 的 projection candidate assembly 会从哪个来源取得这个具体值？

### E03. impl projection 与 nested obligations

考虑：

```rust,ignore
trait Lookup<K> {
    type Value;
}

impl<T: Clone> Lookup<usize> for Vec<T> {
    type Value = Option<T>;
}
```

归一化：

```text
<Vec<String> as Lookup<usize>>::Value
```

回答：

1. impl header 用 fresh args 实例化后可概念化为什么？
2. 与 goal trait ref 做 `eq` 后得到什么映射？
3. normalized value 是什么？
4. 会产生哪个 impl where-clause nested obligation？

### E04. Ambiguity 与 `NormalizesTo`

考虑尚未解析的：

```text
<?T as Iterator>::Item
```

回答：

1. old solver 暂时无法选择唯一结果时，可用什么 term 替换这个 projection？同时登记什么 obligation？
2. 后续得到 `?T = Vec<u8>`，且相应 impl 的 `Item = u8`，输出推理变量最终如何解析？
3. new solver 为什么先建立 `NormalizesTo(alias, ?U)`，再令 `?U == Expected`？
4. `NormalizesTo` 的输出 term 在进入候选计算时应处于什么状态？

## 学习者答案

### E01

> 1. Projection，指向 Convert::Out；2. [S, u32, bool]；3. (u8, u8)；4. Free。

### E02

> 1. T: Iterator, <T as Iterator>::Item == u32；2. 第一条；3. 第二条；4. ParamEnv。

### E03

> 1. Vec<?T>: Lookup<usize>；2. ?T = String; 3. Option<String>; 4. String: Clone。

### E04

> 1. 用一个推理变量 ?U，登记 projection obligation <?T as Iterator>::Item == ?U; 2. ?U = u8; 3. 不让 Expected 影响 normalization；4. normalized 或者 ambiguity
