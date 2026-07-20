---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "20"
document: grading
status: completed
exercise_version: 1
earned_points: 8
max_points: 8
mastery: mastered
updated_at: 2026-09-25
---

# 20. 评分与反馈

## 总评

E01–E04 共 8/8（100%），判定 `mastered`。已能区分定义、实例、mono item 与最终代码；能追踪 trait 方法选择后的参数坐标转换，以及函数值和 vtable 对收集图的影响。

## 待复核小问（未满分）

无。

## 分题评分

每题 2 分，每小问 0.5 分。

| 题号 | 第 1 问 | 第 2 问 | 第 3 问 | 第 4 问 | 合计 |
|---|---:|---:|---:|---:|---:|
| E01 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E02 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E03 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |
| E04 | 0.5 | 0.5 | 0.5 | 0.5 | 2 |

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 2 | 2 | 已掌握 | 同一 `DefId` 可有不同 `args` 与 `Instance`；最终符号数还受后续优化和链接影响。 |
| E02 | 2 | 2 | 已掌握 | 正确区分 trait item 的 `Self` 槽与 impl 方法的 `T` 槽，指出选择与两步参数转换。 |
| E03 | 2 | 2 | 已掌握 | 函数地址可形成依赖；`MonoItem::Fn` 持有 `Instance`，不是新保存的 MIR。 |
| E04 | 2 | 2 | 已掌握 | 三种 mono item、虚调用边界、现存收集/实例化机制和 codegen 顺序均正确。 |

### E01 讲评

四项正确。`id::<u32>` 与 `id::<bool>` 共享定义 `DefId`，分别对应 `Item(id)` 携带 `[u32]`、`[bool]`。rustc 通常读取泛型 MIR，在使用处按 `Instance` 实例化；即使概念上有两个实例，也不能据此断言最终保留两个独立符号。链接时的消除是有效例子，前面的 MIR 优化也可能改变实际收集结果。

### E02 讲评

四项正确。调用侧 trait item 的参数为 `[Self = Wrap<u32>, U = bool]`；impl 方法侧为 `[T = u32, U = bool]`。`codegen_select_candidate` 确定 impl 来源，随后 `rebase_onto` 与 `translate_args` 转换参数坐标。直接用 trait item 的 `DefId` 和 args 构造原始 `Instance`，一般不能代表最终 impl 方法体。

### E03 讲评

四项正确。`main` 是一般模型中的根，泛型定义本身不会为每种 `T` 自动生成代码。取 `keep::<u16>` 的函数地址可形成 `MonoItem::Fn(Instance)` 依赖。收集器扫描 MIR，并用当前 `Instance` 对函数类型做 instantiate、normalize、erase regions。此题的具体函数指针转换走 `MirUsedCollector::visit_rvalue` 的 `ReifyFnPointer` 分支，读取变量名是 `fn_ty`；直接调用的 `visit_terminator` 分支才读取 `callee_ty`。两者都通过 `self.monomorphize` 带入当前实例实参，题目第 3 问的变量名混用了两条路径，不影响本次评分。源码：`compiler/rustc_monomorphize/src/collector.rs::MirUsedCollector`。

### E04 讲评

四项正确。`MonoItem` 有 `Fn`、`Static`、`GlobalAsm`；虚调用本身通过 vtable 间接分发，不等于一个已知具体 impl 的直接调用。构造具体类型的 trait-object vtable 时，要准备所有实际方法槽位对应的实例；之后其他代码（包括其他 crate）可能通过该对象调用那些方法，因此不能只依据构造处显式调用了什么来收集。`-Zpolymorphize` 已移除；当前主线是 mono item 收集决定实体、按 `Instance` 在使用处实例化 MIR。流程是 `collect_crate_mono_items` 建图、`collect_and_partition_mono_items` 收集并分配 CGU、`codegen_mir` 翻译具体实例。

## 已掌握概念

- `DefId + GenericArgs`、解析后的 `Instance`、`MonoItem::Fn` 与最终代码符号不是同一层。
- trait item 到 impl item 需要候选选择、`rebase_onto` 和 `translate_args`。
- 函数值、drop glue、vtable 方法都能成为单态化收集的依赖。
- 虚方法调用与 vtable 构造的依赖来源不同；收集、CGU 分配和 MIR 代码生成分阶段进行。

## 后续复核重点

第 21 章做源码追踪时，可实际定位 `ReifyFnPointer`、直接 `Call` 与 `Unsize` 三条 MIR 收集分支，观察它们分别引入哪些 mono item。

## 补充练习或复习动作

本章无必做复核题。

## 完成判定

8/8（100%），核心模型清晰，第 20 章为 `completed`；第 21 章尚未开始。

## 复核记录

2026-09-25：发布第 20 章讲义和 E01–E04；等待学习者作答。

2026-09-25：原样记录 E01–E04 答案；按当前源码评分 8/8，判定 `mastered`。
