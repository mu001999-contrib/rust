---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "12"
document: exercises
status: assigned
exercise_version: 2
updated_at: 2026-09-05
---

# 12. 习题

## 作答说明

四题，每题 2 分，每小问 0.5 分，共 8 分。以普通 Typeck、正向 trait goal 为背景；涉及泛型匹配时省略隐式 sizedness 条件，不要求填写真实 DefId。

E01–E04 已提交并讲评，题目保持第 1 版原文。第 2 版新增 E05 定向复核，不独立加分；用于更新对应小问的当前评分，总分仍为 8。

## 题目

### E01. 候选来源与索引

分别回答：

1. 在 `fn f<T: Clone>()` 内证明 `T: Clone`，可直接使用哪种候选来源？
2. `impl<T: Clone> Convert<T> for Store<T>` 能否作为 impl candidate？它的 Self 可按 Store 索引吗，为什么有泛型仍可进入 non-blanket bucket？
3. 对没有显式 Send impl 的普通结构体，按字段检查其 Send 的途径属于哪种来源？
4. `DeepRejectCtxt::args_may_unify` 返回 true，是否表示该 impl 已经证明 goal？后面还需要哪些关键步骤？

### E02. header 与 where-clause 的同一份 args

```rust
struct Store<T>(T);
trait Convert<A> {}
impl<K: Clone> Convert<K> for Store<K> {}
```

goal：`Store<?X>: Convert<String>`。为 impl 参数 K 创建 fresh `?K`。

1. 实例化后的 impl trait-ref 是什么？
2. 对 goal 与 impl header 做 eq 后，`?X` 与 `?K` 分别是什么？
3. 实例化的显式 where-clause 解析后是什么 goal？它继承哪个 ParamEnv？
4. 若该 candidate 最终成功，它通过什么数据把 `?X` 的解带出 probe？probe 返回 Ok 是否直接提交其中的 inference 赋值？

### E03. 两条候选的推理隔离

```rust
struct Selector;
trait Pick<A> {}
impl Pick<u32> for Selector {}
impl Pick<bool> for Selector {}
```

goal：`Selector: Pick<?A>`。假定没有环境或特殊偏好候选。

1. 两个 impl candidate 分别给 `?A` 什么约束？各自求值能否返回 Yes？
2. 第一个 probe 中的 `?A = u32` 会不会令第二个 candidate 因 bool 不匹配而失败？为什么？
3. 两个不同 response 合并后，能否直接返回 `Yes, ?A = u32`？应保留什么结果？
4. 调用方随后确定 `?A = u32`，重新求值时哪个 candidate 保留，哪个被排除？

### E04. 通用 response 合并

以下四组是相互独立的情况。假设相关候选搜索与偏好筛选已完成，进入普通 `try_merge_candidates` / `flounder`；除题中所写约束外没有其他差异或外部约束，Self 已知，不涉及特殊 opaque/coherence 分支。

1. candidate 集合为空：结果是什么？
2. 两个 candidate 的 canonical response 完全相同，均为 `Yes, ?A = u32`：能否合并？保留什么？
3. 两个 response 分别为 `Yes, ?A = u32` 和 `Yes, ?A = bool`：结果是什么？会同时应用两份等式吗？
4. 一个 response 为无 inference/external constraints 的 Yes，另一个为有条件的 Maybe：`try_merge_candidates` 可以使用哪一个回答 goal？为什么？

### E05. 匹配步骤、环境与相同响应的定向复核

1. 某 impl 通过 fast reject 后，到生成 candidate response 之前，还需要哪三个关键阶段？可用“实例化 → …… → ……”回答。
2. 当前 `P = [T: Clone]`，goal 为 `Goal(P, Store<T>: Convert<T>)`，使用 E02 的 impl 并匹配出 K = T。产生的 Clone 子目标，其 predicate、ParamEnv 和 GoalSource 分别是什么？
3. 两个来源不同的候选，其完整 canonical response 完全相同，均为 `Yes, ?A = u32`。没有其他偏好或约束：`try_merge_candidates` 返回什么？这是否要求先唯一确定候选来源？

## 学习者答案

### E01

> 1. 直接使用 ParamEnv; 2. 可以，可以，因为外层类型是确定的；3. builtin auto-trait；4. 不是，只是快速排除明显不符合的 candidate，后续还需要进行真正的求解。

### E02

> 1. Store\<?K>: Convert\<?K>; 2. ?X = ?K, ?K = String; 3. String: Clone, 从 impl candidates；4. 通过 CanonicalResponse，probe 不直接提交。

### E03

> 1. ?A = u32, ?A = bool，可以；2. 不会，因为两个只是要求不同的条件，互不干扰；3. 不能，应该是 Maybe；4. 保留 Selector: Pick\<u32>，排除 bool。

### E04

> 1. 结果是 NoSolution；2. 不能合并，应该是 Ambiguous；3. 结果是 Maybe，不会同时应用；4. 可以使用第一个，因为不需要额外的约束。

### E05

待提交。
