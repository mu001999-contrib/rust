---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "15"
document: grading
status: completed
exercise_version: 2
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-09-21
---

# 15. 评分与反馈

## 总评

E05 定向复核通过后，E01–E04 共 **8/8（100%）**。每题 2 分，每小问 0.5 分。对象 IR、约束候选、dyn compatibility、对象 lifetime 与运行时分派的核心模型已确认，本章完成。

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 对象 IR 与约束来源已确认 | E05.1 复核后各小问均为 0.5。 |
| E02 | 2 | 2 | 方法与 trait 泛型的分层已确认 | E05.2 复核后各小问均为 0.5。 |
| E03 | 2 | 2 | 四项均成立 | 引用 lifetime、对象 lifetime、默认 `'static` 与早销毁的区别均正确。 |
| E04 | 2 | 2 | 构造证明、vtable 与 upcast 已确认 | E05.3 复核后各小问均为 0.5。 |

### E01：对象类型与对象候选

1. 三项 existential predicates 正是 `Trait(Iterator)`、`Projection(Iterator::Item = u32)`、`AutoTrait(Send)`。`'obj` 是 `TyKind::Dynamic(predicates, region)` 的第二字段，不属于 `ExistentialPredicate` 枚举；E05.1 复核后计 0.5。
2. `ExistentialTraitRef` 的 args 为 `[]`，普通 `TraitRef` 的 args 则含第一位 `Self`。本小问计 0.5。
3. 补入具体 S 后，得到 `S: Iterator` 与 `<S as Iterator>::Item = u32`。原答的 TraitRef 与 projection 等式方向正确，本小问计 0.5。
4. 对 `D = dyn Iterator<Item=u32>`，依据的是 D 的 `Dynamic` 自带的 existential projection bound。`assemble_object_bound_candidates` 把该 bound 补上 D，作为对象候选；不必依靠 `ParamEnv::caller_bounds` 或推测底层具体类型。E05.1 复核后计 0.5。

源码依据：`compiler/rustc_type_ir/src/ty_kind.rs::TyKind::Dynamic`、`compiler/rustc_type_ir/src/predicate.rs::ExistentialPredicate`、`compiler/rustc_next_trait_solver/src/solve/assembly/mod.rs::assemble_object_bound_candidates`。

### E02：可构造性与方法分派

1. `Source` 可构造为 `dyn Source<Item=u32>`；对象上的关联类型等式确定 `next` 返回 `Option<u32>`。
2. `label<'a>` 的方法 lifetime 参数可动态调用；`build`、`inspect<T>` 由各自的 `Self: Sized` 排除于对象接口之外。
3. 去掉 `inspect<T>` 的逐项 Sized 后，带方法自身类型泛型的 trait 会整体不满足 dyn compatibility，因而不能构造 `dyn Source<Item=u32>`。`dyn Transform<u32>` 中，trait 的类型实参已在对象类型上固定，与为每个方法调用选择 `T` 不同。E05.2 复核后计 0.5。
4. trait 层的 `Self: Sized` 使整个 trait 不适合作为 dyn principal。`&dyn Source` 这个指针有已知大小，`dyn Source` 作为指向的 Self 仍是 DST；原答正确。

源码依据：`compiler/rustc_trait_selection/src/traits/dyn_compatibility.rs::dyn_compatibility_violations_for_assoc_item`、`is_vtable_safe_method`、`virtual_call_violations_for_method`。

### E03：对象 lifetime

1. `'r` 约束引用的使用，`'obj` 是对象对底层类型的 outlives 要求；源类型需要 `S: 'obj`。
2. 在本题函数返回签名中，`Box<dyn Iterator<Item=u32>>` 的对象 lifetime 默认是 `'static`；借用输入 slice 的迭代器通常无法满足此要求。
3. `Box<dyn Iterator<Item=u32> + 'a>` 可承载这段输入借用。这里 `+ 'a` 是对象底层类型的 outlives 要求；`impl Iterator + use<'a>` 的 `use` 则允许 opaque hidden type 依赖 `'a`。
4. `Box<dyn Trait + 'static>` 可以在局部作用域结束时销毁。拥有数据的 String 不携带非 `'static` 借用，故 String 满足 `String: 'static`。

### E04：构造与运行时表示

1. 需要 `S: Iterator`、`<S as Iterator>::Item = u32`、`S: Send`、`S: Sized` 和 `S: 'obj`。E05.3 补全 `S: Send` 后计 0.5。
2. 已构造对象的动态方法调用不重新搜索 impl candidate。数据指针定位底层 S 的值；vtable 指针指向 vtable，方法入口位于该表的相应槽位，表中还有其他元数据。E05.3 已区分两个指针的职责，计 0.5。
3. 公共 header 为 drop-in-place、size、align；当前布局细节不是稳定 Rust ABI 的承诺。
4. `&dyn Child` 可 upcast 到 `&dyn Base`，底层数据类型不变；vtable 元数据可能调整。

源码依据：`compiler/rustc_next_trait_solver/src/solve/trait_goals.rs::consider_builtin_unsize_to_dyn_candidate`、`compiler/rustc_middle/src/ty/vtable.rs::VtblEntry`。

## 已掌握概念

- existential trait ref 省略 Self、关联类型等式可固定对象方法的返回类型。
- 引用 lifetime 与对象 lifetime 的不同约束、默认对象 lifetime 和 owned 类型的 `'static` 含义。
- `Self: Sized` 可以逐项排除方法；trait 层的 Sized 排除整个对象接口。
- vtable header 与 trait upcasting 的基本行为。
- `Dynamic` 的两个字段、对象 bound candidate 的来源、unsizing 的 Send/Sized/outlives 要求及数据指针与 vtable 指针的职责。

## 后续复核重点

后续在第 20 章进一步连接 vtable 方法槽中的 `Instance` 与 monomorphization；第 18 章复核对象 lifetime 与 region inference。

## 补充练习或复习动作

本章 E05 已完成，可按学习者要求进入第 16 章 Const 与 Const Generics。第 12 章 E05 保留于原章节。

## 完成判定

当前状态 `completed`，掌握度 `mastered`。E05 复核确认了本章核心模型，第 16 章仍为 `planned`。

## 复核记录

2026-09-21：原样记录 E01–E04，并完成 E05 定向复核。E05.1 补齐 E01.1/E01.4，E05.2 补齐 E02.3，E05.3 补齐 E04.1/E04.2；各增加 0.25 分，当前总成绩 8/8，判定 mastered。
