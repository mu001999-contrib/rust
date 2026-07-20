---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "14"
document: grading
status: completed
exercise_version: 2
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-09-06
---

# 14. 评分与反馈

## 总评

E05 及 E05.2 复答通过后，E01–E04 共 **8/8（100%）**。每题 2 分，每小问 0.5 分。opaque 身份、定义性约束、capture 契约、edition 差异及 TAIT/RPITIT 与后端展开均已确认，本章完成。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 四项均成立 | 各小问均为 0.5；语法位置、身份、返回一致性及 key/value 区分。 |
| E02 | 2 | 2 | 定义性约束与来源已确认 | E05.1 复核后各小问均为 0.5。 |
| E03 | 2 | 2 | hidden type、capture 契约与 edition 差异已确认 | E05.3 与 E05.2 复答通过后，各小问均为 0.5。 |
| E04 | 2 | 2 | TAIT、RPITIT 与后端展开已确认 | E05.4 补全名称后各小问均为 0.5。 |

### E01：身份与具体类型

1. APIT 输入由调用方选择；RPIT hidden type 由定义方决定。
2. 不同函数的 RPIT 声明具有不同身份，即使实际返回相同的 Range 类型。
3. 同一具体 RPIT 的各返回分支需要一致的类型；共同实现 Iterator 只是 bounds 要求。
4. `OpaqueTypeKey` 包含 `def_id` 和 `args`；hidden type 是 storage 关联的值。

### E02：定义权限、关系与子目标

1. 定义集合外的 opaque 按 `IsRigid::Yes` 处理。“不会进一步求解”在此应精确理解为：不继续展开或重新定义该 alias；它仍可参与相等关系检查和 trait bounds 求解。
2. 新 hidden type 与已登记值通过 `self.eq(...)` 检查一致性。本例需要 `bool == u32`，因此关系检查无法成功。
3. 检查 hidden type 满足 opaque 声明的 bounds；这批 goals 的来源是 `GoalSource::AliasWellFormed`。若证明其中一个 trait goal 时选择了 impl，随后该 impl 的 where 条件才是 `ImplWhereBound`。来源描述的是当前这一步为什么产生 goal，而不是整个证明最终是否使用 impl。
4. 关联类型 projection 的 normalization 读取既有定义，隔离 expected 可避免它干扰候选选择；opaque 的定义性使用则正是在建立 hidden-type 定义，允许 relation 的另一侧参与约束。

源码依据：`compiler/rustc_next_trait_solver/src/solve/project_goals/opaque_types.rs::normalize_opaque_type`；`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs::add_item_bounds_for_hidden_type`；`compiler/rustc_type_ir/src/solve/mod.rs::GoalSource`。

### E03：capture 是依赖契约，outlives 是类型约束

1. hidden types 分别是 `std::str::Chars<'a>` 与 `usize`；前者实际使用 `'a`，后者没有该依赖。
2. `use<'a>` 允许 hidden type 依赖该泛型参数，并不要求一定实际使用它；`+ 'a` 要求 hidden type 满足 outlives `'a`。
3. `length` 的结果可以在输入 String 销毁后继续使用，关键是 `use<>` 明确不捕获输入 lifetime，且定义返回 usize。仅凭“没有写 `+ 'a`”不能作出同样判断：2024 默认捕获可能仍把返回 opaque 与输入 lifetime 关联。反过来，`T: 'a` 是类型的 outlives 保证，也不是要求值必须活到 `'a` 结束。
4. 对没有显式精确捕获列表的 free function RPIT：2021 不自动捕获未出现在返回 bounds 中的 lifetime；2024 默认捕获作用域内的全部泛型参数，包括这类 lifetime。显式 `use<...>` 则直接指定捕获集合。

语言依据：[Rust Reference：Capturing](https://doc.rust-lang.org/reference/types/impl-trait.html#capturing)。

### E04：准确名称与实现衔接

1. 完整语法为 `#![feature(type_alias_impl_trait)]`，定义函数使用 `#[define_opaque(Numbers)]`。E05.4 已补全，当前计 0.5 分。见 [Unstable Book：type_alias_impl_trait](https://doc.rust-lang.org/unstable-book/language-features/type-alias-impl-trait.html)。
2. `Numbers` 是特定 opaque 身份；APIT 则允许调用方选择任意满足 bounds 的输入类型。
3. 按本章借用 self 的示例，“lifetime GAT”作为匿名关联类型的概念类比计满分。准确实现类别是 synthetic anonymous associated type（`DefKind::AssocTy`）；一般 RPITIT 并非一律仅有 lifetime 参数。不同 Self 的 impl 可以提供不同的返回类型。
4. Codegen 可读取 hidden type：`cx.type_of(def_id.into()).instantiate(cx, opaque_ty.args)`，随后 normalize，再与 expected 进行相等关系检查。

源码依据：`compiler/rustc_ty_utils/src/assoc.rs::associated_type_for_impl_trait_in_trait`；`compiler/rustc_next_trait_solver/src/solve/project_goals/opaque_types.rs::normalize_opaque_type`。

## 已掌握概念

- APIT/RPIT 的类型选择方与 opaque identity。
- 同一 key 的 hidden-type 一致性检查。
- opaque 定义性使用与关联类型 normalization 的任务区别。
- Chars 与 usize 的实际 lifetime 依赖。
- TAIT 的特定身份、RPITIT 的关联类型模型及 Codegen 实例化路径。
- AliasWellFormed 与 ImplWhereBound 的分层、2021/2024 lifetime 默认捕获差异，以及 TAIT/RPITIT 的准确名称。

## 后续复核重点

调用方依据 opaque 的公开捕获契约检查借用关系，而不是根据函数体的具体 hidden type 排除参数依赖。

## 补充练习或复习动作

本章复核已完成。后续可进入第 15 章 Trait Objects 与 Dyn Compatibility；第 12 章 E05 保留于原章节。

## 完成判定

当前状态 `completed`，掌握度 `mastered`。E05.2 复答确认了默认捕获对调用方的影响，达到本章完成标准。第 15 章保持 `planned`。

## 复核记录

2026-09-06：E05.1、E05.3、E05.4 及 E05.2 复答通过，对应 E02.3、E03.4、E04.1、E03.3 均更新为 0.5 分，当前成绩 8/8。结合 E05 初答中的 capture/outlives 定义，确认本章掌握。

E05.2 的准确结论：不能保证。2024 中仅声明 `impl Copy` 会默认捕获 `'a`；调用方不能把返回类型当作 usize 来排除该依赖。原例的 `use<>` 才明确排除输入 lifetime。这与当前函数体是否实际保存借用是两个层次。
