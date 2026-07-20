---
course_id: rustc-type-system
format_version: 1.2.2
applies_to_roadmap: 1.0.0
updated_at: 2026-09-06
---

# 持久化格式与更新协议

本文件用于防止后续学习过程中目录、字段、评分方式和更新行为漂移。除非用户明确要求修改格式，否则必须继续遵守本规范。

## 1. 固定目录结构

当前只创建总规划层。某章开始进行增量持久化时，采用以下结构：

```text
rustc-type-system-learning/
├── README.md
├── STATE.md
├── FORMAT.md
└── chapters/
    └── NN-stable-slug/
        ├── content.md
        ├── exercises.md
        └── grading.md
```

约束：

- `NN` 必须是两位章节编号，与 `README.md` 一致。
- `stable-slug` 创建后不得重命名；标题调整只改文件内部标题。
- 每章只使用 `content.md`、`exercises.md`、`grading.md` 三个职责明确的子文件。
- 不把答案和评分混入 `content.md`，不把讲解正文混入 `grading.md`。

## 2. 章节状态机

章节状态只能使用以下值：

```text
planned
  -> in_progress
  -> exercises_assigned
  -> submitted
  -> graded
  -> completed
```

补充状态：

- `needs_review`：评分后存在关键误解，需复习或补充练习。
- `blocked`：存在明确外部阻碍，无法继续；不能用来表示“暂时还没学”。

状态含义：

| 状态 | 含义 |
|---|---|
| `planned` | 尚未开始 |
| `in_progress` | 正在讲授或整理内容 |
| `exercises_assigned` | 本章习题已给出，等待学习者作答 |
| `submitted` | 学习者答案已记录，等待评分 |
| `graded` | 已完成评分和反馈，但尚未确认掌握 |
| `completed` | 达到本章完成标准，可进入下一章 |
| `needs_review` | 需针对薄弱点复习后重新检查 |
| `blocked` | 因明确阻碍暂停 |

## 3. `content.md` 固定模板

```markdown
---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "NN"
document: content
status: in_progress
updated_at: YYYY-MM-DD
---

# NN. 章节标题

## 学习目标

## 前置知识

## 核心心智模型

## 源码地图

## 源码精读

## 正文

## 常见误区

## 本章小结
```

源码引用使用仓库相对路径并尽量附带符号名；行号只作为当时快照，不能替代符号名。

从第 05 章起，`content.md` 还必须遵守以下源码辅助讲解规则：

- `源码地图` 列出本章涉及的当前仓库路径与关键符号。
- `源码精读` 至少选取三处与核心心智模型直接相关的 rustc 实现片段。
- 每个片段标明仓库相对路径与符号名，并解释字段、控制流或实现不变量。
- 片段保持聚焦；可省略与当前概念无关的分支，但必须明确标出省略处。
- 正文中的抽象概念要回指实现，例如说明一次状态变化最终写入哪张表、产生哪类约束或经过哪个 API。
- 以当前检出的源码为准；路径和符号名是稳定定位依据，行号仅用于临时导航。

## 4. `exercises.md` 固定模板

```markdown
---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "NN"
document: exercises
status: assigned
exercise_version: 1
updated_at: YYYY-MM-DD
---

# NN. 习题

## 作答说明

## 题目

### E01. 题目标题

题目正文。

## 学习者答案

### E01

> 原样记录学习者答案；仅做必要的 Markdown 转义。
```

规则：

- 题号固定为 `E01`、`E02`……，发布后不重排。
- 教学内容发生实质变化时增加 `exercise_version`；文件正文始终呈现当前完整题目集。
- 学习者答案尽量原样保存，不用讲评后的正确答案覆盖原答案。

## 5. `grading.md` 固定模板

```markdown
---
course_id: rustc-type-system
roadmap_version: 1.0.0
chapter: "NN"
document: grading
status: graded
exercise_version: 1
earned_points: 0
max_points: 0
mastery: pending
updated_at: YYYY-MM-DD
---

# NN. 评分与反馈

## 总评

## 分题评分

| 题号 | 得分 | 满分 | 结论 | 反馈 |
|---|---:|---:|---|---|
| E01 | 0 | 0 |  |  |

## 已掌握概念

## 后续复核重点

## 补充练习或复习动作

## 完成判定

## 复核记录
```

评分规则：

- 默认每个独立判断点 1 分；部分正确可记 0.5 分，题目另有说明时以题目为准。
- 不只判断最终答案，也检查变量作用域、替换方向和源码概念是否准确。
- `mastery` 使用 `pending`、`mastered`、`needs_review`。
- 通常总分达到 80% 且核心概念已经形成清晰模型时，可判定 `mastered`；其余情况安排针对性复习或补充题。
- 评分文件采用当前事实视图，保留完整分数、有效结论以及对后续学习有用的解释。
- 章节文件聚焦课程内容、题目、学习者原答、当前评分与学习状态，以此构成完整学习记录。

## 6. 每次学习的更新顺序

开始继续学习前：

1. 读取 `README.md`，确认固定规划与版本。
2. 读取 `STATE.md`，从 `current_chapter`、`current_status` 和 `next_action` 恢复断点。
3. 读取 `FORMAT.md`，保证文件名、字段和状态值不漂移。
4. 若章节目录已经存在，再读取该章的三个子文件。

学习过程中：

1. 章节讲义写入 `content.md`；讲义交付后的交互式答疑默认在聊天中进行，按下方内容交付约定处理。
2. 布置题目时更新 `exercises.md`，并将总状态改为 `exercises_assigned`。
3. 收到答案后先原样写入 `exercises.md`，状态改为 `submitted`。
4. 完成讲评后写入 `grading.md`，状态改为 `graded`、`needs_review` 或 `completed`。
5. 每次状态变化都同步更新 `STATE.md` 的当前断点、全局进度和学习推进记录。

### 内容交付约定

- 新章节的讲义与正式习题默认完整持久化到对应文件，聊天中简短确认并提供链接。
- `content.md` 保存可独立阅读的章节讲义，包括讲解、代码示例、源码依据与关键结论。
- 讲义持久化后的交互式学习（追问、补充解释、讨论与临时练习）直接在聊天中进行，默认不再持久化；学习者明确要求记录时，再写入对应文件。
- 正式习题的原始作答、评分与学习进度仍按原协议持久化：`exercises.md` 保存正式题目和原始回答，`grading.md` 保存评分与解释，`STATE.md` 保存学习断点与进度。

## 7. 防漂移规则

- `README.md` 是课程结构的唯一事实来源；`STATE.md` 不得重新定义章节。
- `STATE.md` 是动态进度的唯一事实来源；其他文件不得声明冲突的当前章节。
- 未经用户明确要求，不新增、删除、合并或重排 21 个章节。
- 必须修改规划时，提升 `roadmap_version`，同步所有相关 front matter，并在 `README.md` 追加变更记录。
- 必须修改模板时，提升 `format_version`，并在本文件末尾追加格式变更记录。
- 日期统一使用 `Asia/Shanghai` 的 `YYYY-MM-DD`。
- 课程记录属于个人学习材料；提交 rust-lang/rust 上游改动前，应检查并避免误带该目录。

## 格式变更记录

| 日期 | 版本 | 变更 |
|---|---|---|
| 2026-07-21 | 1.0.0 | 固定总规划层、章节目录、三个子文件模板、状态机和评分协议。 |
| 2026-07-21 | 1.1.0 | 章节档案采用当前事实视图；状态历史聚焦学习推进。 |
| 2026-07-26 | 1.2.0 | 从第 05 章起，讲义必须包含当前仓库 rustc 实现片段及逐段解释。 |
| 2026-09-05 | 1.2.1 | 学习内容以完整文件交付为默认方式，聊天仅简短确认与定位。 |
| 2026-09-06 | 1.2.2 | 章节讲义交付后，交互式学习默认在聊天中进行；正式习题、评分与进度继续按协议记录。 |
