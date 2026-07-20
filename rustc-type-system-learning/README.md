---
course_id: rustc-type-system
course_title: rustc 类型系统源码学习
roadmap_version: 1.0.0
total_chapters: 21
state_file: STATE.md
format_file: FORMAT.md
---

# rustc 类型系统源码学习

本目录是这套学习过程的持久化事实来源。整体课程结构以本文件为准，动态进度以 [STATE.md](STATE.md) 为准，每章的文件格式与更新规则以 [FORMAT.md](FORMAT.md) 为准。

## 学习目标

完成课程后，应当能够：

1. 准确阅读 rustc Type IR 中类型、区域、常量与泛型参数的表示。
2. 追踪 binder、de Bruijn index、substitution、folding 与 canonicalization 的变量作用域。
3. 理解 inference、normalization、obligation、trait solving 与 region solving 的协作关系。
4. 从 HIR 类型降低一路追踪到 MIR type checking、borrow checking 和 monomorphization。
5. 使用 rustc 的日志、dump、测试和源码导航手段独立定位类型系统问题。

## 固定课程规划

章节编号一旦投入学习便不重新编号；课程范围发生变化时，必须更新 `roadmap_version` 并在本文末尾追加变更记录。

| 章 | 主题 | 核心范围 | 完成标准 |
|---:|---|---|---|
| 01 | Type IR 基础 | `Interner`、`Ty`、`Region`、`Const`、`GenericArg`，interning 与结构共享 | 能从源码识别核心 IR 节点及其不变量 |
| 02 | 遍历、折叠与 Binder | `TypeVisitable`、`TypeFoldable`、visitor/folder、`Binder`、de Bruijn index、capture avoidance | 能手算嵌套 binder 下的访问、替换与 shift |
| 03 | Item 泛型与 EarlyBinder | `EarlyBinder` vs `Binder`、`Generics`、`GenericParamDef`、`GenericArgs`、identity/具体实例化 | 能从 parent chain 构造 args 并分层完成实例化 |
| 04 | Region、Universe 与 Placeholder | region kinds、early/late-bound region、universe、placeholder、leak check | 能区分 region 表示并解释 universe 可见性 |
| 05 | 推理上下文与统一 | `InferCtxt`、类型/区域/常量推理变量、unification table、probe/snapshot | 能追踪推理变量的创建、合并、回滚和解析 |
| 06 | 类型关系、子类型与 Variance | equality、subtyping、coercion、variance、lattice/relate 操作 | 能判断关系检查的方向及 variance 对参数的影响 |
| 07 | Predicate、Clause 与 ParamEnv | predicates、clauses、where-clauses、implied bounds、`ParamEnv` | 能把源码约束翻译成求解器输入环境 |
| 08 | Obligation 与 Fulfillment | obligation cause、obligation forest、注册、选择、歧义与错误传播 | 能追踪一个 obligation 的完整生命周期 |
| 09 | Alias、Projection 与 Normalization | type aliases、projection、weak/inherent aliases、eager/lazy normalization | 能判断何时产生 alias、何时及为何归一化 |
| 10 | Canonicalization 与查询响应 | canonical vars、canonical input/response、query constraints、实例化响应 | 能手算一次 canonical query 的输入输出映射 |
| 11 | Trait Solver：Goal 建模 | `Goal`、`Predicate`、`EvalCtxt`、goal decomposition、coinduction 基础 | 能把 trait/type relation 问题拆成 solver goals |
| 12 | Trait Solver：候选搜索 | impl/param-env/builtin candidates、candidate assembly、evaluation、ambiguity | 能解释候选从收集到合并的过程 |
| 13 | Associated Types 与 GAT | projections、associated type bounds、GAT 参数与 outlives、lazy normalization 交互 | 能追踪关联类型投影及其约束来源 |
| 14 | Opaque Types 与 `impl Trait` | RPIT、TAIT、RPITIT、hidden type 推断与约束、capture rules | 能解释 opaque identity、hidden type 与定义域 |
| 15 | Trait Objects 与 Dyn Compatibility | existential predicates、dyn compatibility、object lifetime、vtable 相关类型信息 | 能从 trait 定义推导可否构造 `dyn Trait` |
| 16 | Const 与 Const Generics | `ty::Const`、abstract const、unevaluated const、const equality、generic const expressions | 能区分常量表示、求值时机与相等性约束 |
| 17 | 高阶类型与量化约束 | HRTB、higher-ranked goals、placeholder instantiation、leak checking、higher-ranked subtyping | 能分析跨多个 binder/universe 的高阶约束 |
| 18 | Region Inference 与 NLL | outlives constraints、universal regions、liveness、SCC/constraint propagation | 能从 MIR 位置约束解释 region 解 |
| 19 | MIR Type Checking 与 Borrowck | MIR typeck、member/type tests、borrow set、Polonius 接口、诊断约束 | 能串起 MIR 类型检查与借用检查输入输出 |
| 20 | 实例选择与 Monomorphization | `Instance`、resolve、substs/args、polymorphization、mono item collection | 能从泛型调用追踪到具体 codegen instance |
| 21 | 调试方法与综合源码实践 | `-Z` 选项、日志、dump、UI tests、最小复现、跨 crate 查询追踪 | 能独立完成一个类型系统问题的定位与讲解 |

## 阶段划分

| 阶段 | 章节 | 目标 |
|---|---|---|
| I. IR 与变量作用域 | 01–04 | 建立不会混淆 Param、Bound、Placeholder、Infer 的表示模型 |
| II. 推理与约束基础 | 05–10 | 理解关系检查、环境、obligation、normalization 与 canonical query |
| III. Trait 系统 | 11–17 | 掌握 trait solver 及关联类型、opaque、dyn、const、高阶约束 |
| IV. Region、MIR 与落地 | 18–21 | 串起 NLL、borrowck、monomorphization 和实际调试工作流 |

## 持久化边界

- 当前已按固定格式持久化第 01–14 章；具体动态断点以 `STATE.md` 为准。
- 章节开始增量记录时，才创建 `chapters/NN-slug/` 及其三个固定子文件。
- 动态进度不得写回本文件的课程表，避免规划与状态相互覆盖。
- 不将这些个人学习记录放入 `src/doc/rustc-dev-guide/`，以免混入 rust-lang/rust 的官方文档工程。

## 规划变更记录

| 日期 | 版本 | 变更 |
|---|---|---|
| 2026-07-21 | 1.0.0 | 首次持久化 21 章整体规划，并冻结章节编号。 |
