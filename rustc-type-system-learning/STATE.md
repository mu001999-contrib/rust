---
course_id: rustc-type-system
roadmap_version: 1.0.0
state_schema_version: 1
updated_at: 2026-09-06
timezone: Asia/Shanghai
current_chapter: "14"
current_status: completed
next_action: 按学习者要求进入第 15 章 Trait Objects 与 Dyn Compatibility
---

# 学习状态

这是课程动态状态的唯一事实来源。课程结构不得在此文件中修改。

## 当前断点

| 字段 | 当前值 |
|---|---|
| 当前章节 | 14 |
| 当前状态 | `completed` |
| 最近完成 | 14：Opaque Types 与 impl Trait（E05 复核后 8/8，`mastered`） |
| 下一动作 | 按学习者要求进入第 15 章 Trait Objects 与 Dyn Compatibility |
| 当前重点 | 第 14 章完成，第 15 章尚未开始；第 12 章 E05 继续保留 |

## 全局进度

允许的章节状态定义在 `FORMAT.md`。此表只记录状态，不重复维护章节标题和范围。

| 章 | 状态 | 最近结果/备注 |
|---:|---|---|
| 01 | `completed` | E01–E05 完整成绩 26 / 31（83.9%），`mastered` |
| 02 | `completed` | visitor/folder 4/4；Binder/de Bruijn 4/4；合计 8/8（100%），`mastered` |
| 03 | `completed` | E01–E04 共 8/8（100%），`mastered` |
| 04 | `completed` | E01–E04 共 7.5/8（93.75%），`mastered` |
| 05 | `completed` | 修正后 E01–E04 共 7.5/8（93.75%）；snapshot 定向复核四项全对，`mastered` |
| 06 | `completed` | 修正后 E01–E04 共 8/8（100%）；顺序合并复核四项全对，`mastered` |
| 07 | `completed` | E01–E04 共 7.5/8（93.75%），`mastered` |
| 08 | `completed` | E01–E04 共 7.5/8（93.75%），`mastered` |
| 09 | `completed` | E01–E04 共 7.5/8（93.75%），`mastered` |
| 10 | `completed` | E01–E04 共 7.5/8（93.75%），`mastered` |
| 11 | `completed` | E01–E04 共 7.75/8（96.875%），`mastered` |
| 12 | `graded` | E01–E04 共 7/8（87.5%）；E05 定向复核已发布 |
| 13 | `completed` | E05 复核后 E01–E04 共 8/8（100%），`mastered` |
| 14 | `completed` | E05 及 E05.2 复答后 8/8（100%），`mastered` |
| 15 | `planned` |  |
| 16 | `planned` |  |
| 17 | `planned` |  |
| 18 | `planned` |  |
| 19 | `planned` |  |
| 20 | `planned` |  |
| 21 | `planned` |  |

## 当前待评内容

| 章 | 题目 | 状态 |
|---:|---|---|
| 07 | E01–E04 | `graded`，7.5/8，`mastered` |
| 08 | E01–E04 | `graded`，7.5/8，`mastered` |
| 09 | E01–E04 | `graded`，7.5/8，`mastered` |
| 10 | E01–E04 | `graded`，7.5/8，`mastered` |
| 11 | E01–E04 | `graded`，7.75/8，`mastered` |
| 12 | E01–E04；E05 | E01–E04 已评分 7/8；E05 待提交 |
| 13 | E01–E04；E05 | E05 已复核，当前成绩 8/8，`mastered` |
| 14 | E01–E04；E05 | E05 及 E05.2 复答已复核，当前成绩 8/8，`mastered` |

## 当前掌握记录

| 章 | 掌握度 | 证据 | 后续复核点 |
|---:|---|---|---|
| 01 | `mastered` | E01 8/8；E02 6.5/9；E03 3.5/6；E04 4/4；E05 4/4；总计 26/31 | 第 02 章已复核 Binder；第 04 章已复核 placeholder/universe；第 10 章复核 canonicalization |
| 02 | `mastered` | visitor/folder E05–E08 与 Binder/de Bruijn E01–E04 全部正确；总计 8/8 | 第 03 章已复核 `EarlyBinder` 与真正 binder 的边界；第 17 章复核高阶关系 |
| 03 | `mastered` | parent chain、具体实例化、binder-aware shift、`extend_to` / `rebase_onto` 四题全部正确；总计 8/8 | 第 04 章已复核 early/late-bound region；第 20 章复核实例参数到 monomorphization 的传递 |
| 04 | `mastered` | region variants、universe nameability、liberate/enter_forall 与 placeholder escape；总计 7.5/8 | 第 05 章复核 region var universe；第 17 章复核 higher-ranked quantifier 顺序；第 18 章复核 NLL region representation |
| 05 | `mastered` | E01 1.5/2；E02 2/2；E03 修正后 2/2；E04 2/2；总计 7.5/8 | 第 06 章在 speculative type relations 中复用 snapshot；第 18 章连接 region inference |
| 06 | `mastered` | E01 修正后 2/2；E02 2/2；E03 2/2；E04 修正后 2/2；总计 8/8 | 第 07 章复核 relation 产生的 obligations 如何进入 `ParamEnv`；第 17 章复核 higher-ranked relations |
| 07 | `mastered` | E01 2/2；E02 2/2；E03 2/2；E04 1.5/2；总计 7.5/8 | 第 08 章复核 obligation/goal 的上下文携带；第 17 章复核 higher-ranked assumptions；继续区分 generic impl 与 blanket impl bucket |
| 08 | `mastered` | E01 2/2；E02 2/2；E03 2/2；E04 1.5/2；总计 7.5/8 | 第 09 章复核 selection confirmation 与 nested obligations 的来源；继续区分 `ObligationCause` / backtrace 与 `recursion_depth` |
| 09 | `mastered` | E01 2/2；E02 2/2；E03 2/2；E04 1.5/2；总计 7.5/8 | 第 10 章复核 canonical query 中 unconstrained output variable、输入 canonical vars 与 response constraints |
| 10 | `mastered` | E01 2/2；E02 1.5/2；E03 2/2；E04 2/2；总计 7.5/8 | 第 11 章复核 canonical input 到 query-local goal；第 17 章复核 placeholder 与 universe nameability |
| 11 | `mastered` | E01 2/2；E02 1.75/2；E03 2/2；E04 2/2；总计 7.75/8 | 第 12 章复核 relation 生成的 nested goals 与候选合并；第 17 章复核 cycle provisional result |
| 12 | `pending` | E01 1.75/2；E02 1.75/2；E03 2/2；E04 1.5/2；总计 7/8 | E05 复核匹配步骤、ParamEnv 继承与相同 response 合并 |
| 13 | `mastered` | E05 复核后 E01–E04 各 2/2，总计 8/8 | 第 14 章复用 bounds/normalization；第 17 章连接 GAT 与高阶 binder |
| 14 | `mastered` | E05 及 E05.2 复答后 E01–E04 各 2/2，总计 8/8 | 第 15 章对比 opaque 与 dyn；第 18 章连接 region inference |

## 状态变更记录

此表记录课程学习推进的有效状态。

| 日期 | 章节 | 从 | 到 | 原因 |
|---|---:|---|---|---|
| 2026-07-21 | 01 | `planned` | `in_progress` | 开始学习 Type IR 核心表示、interning 与 lowering。 |
| 2026-07-21 | 01 | `in_progress` | `exercises_assigned` | 完成章节讲授并形成 E01、E02 两轮练习。 |
| 2026-07-21 | 01 | `exercises_assigned` | `submitted` | 原样记录三轮答案及 E01 修订提交。 |
| 2026-07-21 | 01 | `submitted` | `graded` | E01–E05 综合评分为 26 / 31。 |
| 2026-07-21 | 01 | `graded` | `completed` | 达到 80% 阈值，并形成 Type IR 基础概念的清晰模型。 |
| 2026-07-21 | 02 | `planned` | `in_progress` | 开始学习 TypeVisitable/TypeFoldable、visitor/folder 与 binder-aware traversal。 |
| 2026-07-21 | 02 | `in_progress` | `exercises_assigned` | 完成 visitor/folder、Binder、de Bruijn、escaping vars 与 capture avoidance 的讲授和两轮练习。 |
| 2026-07-21 | 02 | `exercises_assigned` | `submitted` | 原样记录两轮八题答案。 |
| 2026-07-21 | 02 | `submitted` | `graded` | 八题全部正确，评分 8 / 8。 |
| 2026-07-21 | 02 | `graded` | `completed` | 能正确手算嵌套 binder、shift 和稳定实例化映射，判定 mastered。 |
| 2026-07-21 | 03 | `planned` | `in_progress` | 开始学习 item generics、parent chain、`GenericArgs` 与 `EarlyBinder`。 |
| 2026-07-21 | 03 | `in_progress` | `exercises_assigned` | 完成第 03 章讲授并布置 E01–E04。 |
| 2026-07-21 | 03 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-07-21 | 03 | `submitted` | `graded` | 四题全部正确，评分 8 / 8。 |
| 2026-07-21 | 03 | `graded` | `completed` | 能正确构造完整 args、分层实例化并处理内部 binder shift，判定 mastered。 |
| 2026-07-24 | 04 | `planned` | `in_progress` | 开始学习 region kinds、early/late-bound region、universe 与 placeholder。 |
| 2026-07-24 | 04 | `in_progress` | `exercises_assigned` | 完成第 04 章讲授并布置 E01–E04。 |
| 2026-07-26 | 04 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-07-26 | 04 | `submitted` | `graded` | 综合评分 7.5 / 8。 |
| 2026-07-26 | 04 | `graded` | `completed` | region 表示与 universe 可见性已掌握，量词顺序经讲评补齐，判定 mastered。 |
| 2026-07-26 | 05 | `planned` | `in_progress` | 开始学习 `InferCtxt`、推理变量、统一、snapshot 与解析。 |
| 2026-07-26 | 05 | `in_progress` | `exercises_assigned` | 完成第 05 章讲授、当前 rustc 源码精读并布置 E01–E04。 |
| 2026-07-27 | 05 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-07-27 | 05 | `submitted` | `needs_review` | 综合评分 6.5/8；snapshot rollback 安排定向复核。 |
| 2026-07-27 | 05 | `needs_review` | `completed` | snapshot 定向复核四项全部正确；作为 E03 修正答案计分，E03 更新为 2/2，总成绩更新为 7.5/8。 |
| 2026-07-28 | 06 | `planned` | `in_progress` | 开始学习 equality、subtyping、coercion、variance 与 lattice relation。 |
| 2026-07-28 | 06 | `in_progress` | `exercises_assigned` | 完成当前 rustc 源码辅助讲授并发布 E01–E04。 |
| 2026-08-01 | 06 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-08-01 | 06 | `submitted` | `needs_review` | 综合评分 7/8；安排 region constraint 与引用 LUB/GLB 顺序合并复核。 |
| 2026-08-01 | 06 | `needs_review` | `completed` | 顺序合并复核四项全部正确；作为 E01/E04 修正答案计分，总成绩更新为 8/8。 |
| 2026-08-02 | 07 | `planned` | `in_progress` | 开始学习 `Predicate`、`Clause`、where clauses、implied bounds 与 `ParamEnv`。 |
| 2026-08-02 | 07 | `in_progress` | `exercises_assigned` | 完成当前 rustc 源码辅助讲授并发布 E01–E04。 |
| 2026-08-02 | 07 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-08-02 | 07 | `submitted` | `completed` | 综合评分 7.5/8；`Predicate`/`Clause`、`ParamEnv`、implied outlives 与 goal 证明链达到掌握标准。 |
| 2026-08-02 | 08 | `planned` | `in_progress` | 开始学习 `Obligation`、fulfillment、obligation forest 与错误传播。 |
| 2026-08-02 | 08 | `in_progress` | `exercises_assigned` | 完成当前 rustc 源码辅助讲授并发布 E01–E04。 |
| 2026-08-02 | 08 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-08-02 | 08 | `submitted` | `completed` | 综合评分 7.5/8；`Obligation`、fulfillment lifecycle、ambiguity 与 nested obligations 达到掌握标准。 |
| 2026-08-05 | 09 | `planned` | `in_progress` | 开始学习 alias IR、projection 与 normalization。 |
| 2026-08-05 | 09 | `in_progress` | `exercises_assigned` | 完成当前 rustc 源码辅助讲授并发布 E01–E04。 |
| 2026-08-12 | 09 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-08-12 | 09 | `submitted` | `completed` | 综合评分 7.5/8；alias IR、projection candidate、normalization 与 deferred goal 达到掌握标准。 |
| 2026-08-12 | 10 | `planned` | `in_progress` | 开始学习 canonical vars、canonical query input 与 query response。 |
| 2026-08-12 | 10 | `in_progress` | `exercises_assigned` | 完成当前 rustc 源码辅助讲授并发布 E01–E04。 |
| 2026-08-17 | 10 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-08-17 | 10 | `submitted` | `completed` | 综合评分 7.5/8；canonical input/output 映射、query-local 实例化与 response constraints 达到掌握标准。 |
| 2026-08-26 | 11 | `planned` | `in_progress` | 开始学习 `Goal`、`EvalCtxt`、goal decomposition 与 coinduction 基础。 |
| 2026-08-26 | 11 | `in_progress` | `exercises_assigned` | 完成当前 rustc 源码辅助讲授并发布 E01–E04。 |
| 2026-09-05 | 11 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 答案。 |
| 2026-09-05 | 11 | `submitted` | `graded` | 对照当前源码完成四题评分，综合成绩 7.75/8。 |
| 2026-09-05 | 11 | `graded` | `completed` | Goal 建模、分派、fixpoint 与 cycle 基础达到 mastered；第 12 章保持 planned。 |
| 2026-09-05 | 12 | `planned` | `in_progress` | 开始学习 candidate assembly、impl/ParamEnv/builtin 来源、probe 和 response 合并。 |
| 2026-09-05 | 12 | `in_progress` | `exercises_assigned` | 持久化当前源码精读与四题候选搜索练习。 |
| 2026-09-05 | 12 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 的四题答案。 |
| 2026-09-05 | 12 | `submitted` | `graded` | 源码对照评分 7/8，发布 E05 定向复核以确认关键规则。 |
| 2026-09-05 | 13 | `planned` | `in_progress` | 按学习者要求继续下一章，第 12 章成绩与 E05 复核记录保留。 |
| 2026-09-05 | 13 | `in_progress` | `exercises_assigned` | 持久化 GAT 源码讲义、参数映射与 bounds 示例，发布 E01–E04。 |
| 2026-09-06 | 13 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 的四题回答。 |
| 2026-09-06 | 13 | `submitted` | `graded` | 对照源码完成四题讲评，综合成绩 6/8。 |
| 2026-09-06 | 13 | `graded` | `needs_review` | 发布 E05，复核参数替换、使用前提与 normalization 边界。 |
| 2026-09-06 | 13 | `needs_review` | `completed` | E05 四项概念复核通过，对应原题更新后总成绩 8/8，判定 mastered。 |
| 2026-09-06 | 14 | `planned` | `in_progress` | 开始学习 opaque identity、hidden type、capture、TAIT 与 RPITIT。 |
| 2026-09-06 | 14 | `in_progress` | `exercises_assigned` | 持久化当前源码讲义与验证示例，发布 E01–E04。 |
| 2026-09-06 | 14 | `exercises_assigned` | `submitted` | 原样记录 E01–E04 的四题回答。 |
| 2026-09-06 | 14 | `submitted` | `graded` | 对照源码完成评分，综合成绩 7/8，发布 E05 定向复核。 |
| 2026-09-06 | 14 | `graded` | `graded` | E05.1、E05.3、E05.4 复核通过，更新为 7.75/8；继续确认 E05.2。 |
| 2026-09-06 | 14 | `graded` | `completed` | E05.2 复答确认默认捕获的调用方约束，更新为 8/8，判定 mastered。 |
