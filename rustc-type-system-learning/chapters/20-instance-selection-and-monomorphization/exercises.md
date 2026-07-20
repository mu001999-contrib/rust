---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "20"
document: exercises
status: completed
exercise_version: 1
updated_at: 2026-09-25
---

# 20. 习题

## 作答说明

E01–E04 每题 2 分，每小问 0.5 分，共 8 分。回答时优先分清“定义 ID、参数坐标、实例、mono item、最终代码”这几层。源码和示例的优化结果可能受编译选项影响；题目问概念上的依赖或源码阶段，不要求猜测最终二进制有几个符号。

## 题目

### E01. 同一定义的两个实例

```rust
fn id<T>(x: T) -> T { x }
fn main() {
    let a = id::<u32>(7);
    let b = id::<bool>(true);
    std::hint::black_box((a, b));
}
```

1. 两次调用的 `id` 是否共享同一个定义 `DefId`？各自 `GenericArgs` 是什么？
2. 这两次自由函数调用分别可概念化为何种 `InstanceKind` 与 `args` 的组合？
3. 是否应断言 rustc 事先为它们持久保存两份完整的“单态化 MIR”？当前 `Instance` 与泛型 MIR 如何配合？
4. 已确定两个概念上的调用实例，是否就能断言最终二进制严格有两个独立的 `id` 函数符号？说明一个后续会影响它的阶段或机制。

### E02. trait item 到 impl item 的坐标转换

```rust
trait Show {
    fn show<U>(&self, value: U);
}
struct Wrap<T>(T);
impl<T> Show for Wrap<T> {
    fn show<U>(&self, _: U) {}
}
fn main() {
    Wrap(3_u32).show(true);
}
```

1. 将这次调用视作 `Show::show` trait item 时，`Self` 与方法自身的 `U` 分别是什么？
2. 选中 `impl<T> Show for Wrap<T>` 后，具体 impl 方法的 `T` 与 `U` 分别是什么？能否把 `Self = Wrap<u32>` 原样放进 impl 的 `T` 槽？
3. 当前源码中哪个选择步骤决定 impl 来源？哪两个参数转换步骤把调用坐标转成实际方法体坐标？
4. `Instance::new_raw(trait_method_def_id, trait_item_args)` 是否一般就能代表此处要执行的 impl 方法体？为什么？

### E03. 函数值也会引入 mono item

```rust
fn keep<T>(x: T) -> T { x }
fn main() {
    let f: fn(u16) -> u16 = keep::<u16>;
    std::hint::black_box(f);
}
```

1. 在一般的根节点收集模型中，非泛型 `main` 与泛型 `keep` 定义哪个先作为根？仅写出泛型定义是否就要求为所有 `T` 生成代码？
2. `keep::<u16>` 没有在源码中被调用，为什么仍可能出现对应的 `MonoItem::Fn` 依赖？
3. 收集器扫描哪个 IR、用当前 `Instance` 对 `callee_ty` 做什么，才能看到具体的 `u16` 实参？
4. `MonoItem::Fn` 中放的是定义 `DefId`、`Instance`，还是一份新生成的 MIR？

### E04. 实例种类、收集与代码生成

1. 写出当前 `MonoItem` 的三种变体；`dyn Trait` 的虚方法调用本身一定会作为某个具体 impl 的直接调用 mono item 吗？
2. 构造一个具体类型到 `dyn Trait` 的 vtable 时，为何可能需要收集该具体类型的方法实例，即使源码没有显式调用该方法？
3. 当前仓库中的 `-Zpolymorphize` 是否仍是单态化必经 pass？若不是，本章应以哪两个现存机制理解“决定代码实体”和“实际代入泛型参数”？
4. `collect_crate_mono_items`、`collect_and_partition_mono_items`、`codegen_mir` 分别承担什么步骤？按顺序回答。

## 学习者答案

### E01

> 1. 是，[u32], [bool]；2. Item(id) + [u32], Item(id) + [bool]；3. 否，使用 Instance 实例化泛型 MIR；4. 不能，比如说链接可能会消除。

### E02

> 1. Self 是 Wrap\<u32>，U = bool；2. T = u32, U = bool，不能；3. codegen\_select\_candidate 决定 impl 来源，rebase\_onto 和 translate\_args；4. 不能，要经过 impl 选择和 args 转换。

### E03

> 1. main，否；2. 因为获取了函数地址；3. MIR，用这个 `Instance` 的实参做 instantiate、normalize 和 erase regions；4. Instance。

### E04

> 1. Fn, Static, GlobalAsm, 不一定；2. 因为可能被其它 crate 使用；3. 不是，收集和实例化；4. 收集 mono items、分割 codegen units，代码生成
