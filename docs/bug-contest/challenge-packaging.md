# 标准题目包规范

本规范定义每道比赛题应如何打包、交付和维护。
目标是让 Host、ArceOS、StarryOS、Axvisor 四类题目都能共用一套外壳，只在评测命令和离线资源上做差异化。

## 1. 标准目录结构

建议每道题都按如下目录组织：

```text
challenge-id/
├── challenge.md
├── challenge-manifest.yaml
├── repro.sh
├── submission-template.md
├── test-plan.md
└── maintainer-note.md
```

每个文件的职责如下：

- `challenge.md`：给选手看的公开题面，说明背景、目标、允许修改路径和 visible tests。
- `challenge-manifest.yaml`：给主办方和评测平台看的结构化清单，固定题目 ID、赛道、判题 profile、允许修改目录、命令与提交要求。
- `repro.sh`：官方给出的最小复现脚本，帮助选手快速进入问题现场。
- `submission-template.md`：选手提交说明模板，统一收集 patch、根因分析和验证结果。
- `test-plan.md`：记录 visible/hidden tests 设计思路与命令映射。实际 hidden tests 数据不提交到公开仓库，但设计口径要记录在这里。
- `maintainer-note.md`：仅出题人和评审可见，保存标准答案、隐藏测试意图、人工复审要点和防投机策略。

## 2. 题面最少要素

公开题面 `challenge.md` 至少要包含：

- 题目 ID、赛道、难度、适合人群
- 题目背景与问题现象
- 允许修改和禁止修改的路径
- 最小复现命令或 `repro.sh` 入口
- visible tests 的含义和通过标准
- 提交格式
- 评测提醒，例如是否会运行 `cargo fmt --all -- --check` 和 `cargo xtask clippy`

不建议在题面里直接暴露：

- 标准答案
- hidden tests 的精确输入
- 评审对“偷鸡补丁”的识别策略

## 3. manifest 最少要素

`challenge-manifest.yaml` 应作为每道题的结构化事实来源，至少包含：

- 题目基础信息：ID、标题、赛道、阶段、难度、标签
- 基线分支或提交点
- 允许修改与禁止修改路径
- 使用的 `judge_profile`
- visible tests、hidden tests、质量门命令
- 提交物要求
- 题目依赖的离线资源

如果某题要用到 rootfs、guest 镜像、`vmconfigs` 或自定义输入镜像，必须先在 manifest 中登记，再交给比赛平台挂载。

## 4. visible / hidden tests 设计规则

### 4.1 visible tests

visible tests 应满足：

- 能帮助选手定位 bug 类型
- 不把全部边界条件直接泄漏给选手
- 命令足够稳定，不依赖外网
- 日志足够短，适合在线判题回传

### 4.2 hidden tests

hidden tests 应满足：

- 与 visible tests 共享同一条功能链路，但覆盖更完整的边界
- 不通过“换平台、换镜像、换架构”来人为加难
- 不要求额外联网下载
- 如果使用额外镜像或输入数据，必须在平台侧保存并在 `test-plan.md` 与 manifest 中登记

对不同赛道的建议：

- Host：补边界值、并发顺序、错误输入矩阵
- ArceOS：补不同任务时序、不同设备开关、不同测试包组合
- StarryOS：补不同 syscall 序列、rootfs 用户态程序组合
- Axvisor：补不同 guest 输入、不同 VM 资源布局、不同设备访问顺序

## 5. 提交格式规则

选手统一提交以下内容：

- 一份最小修复补丁
- 一份回归测试或复现脚本
- 一份根因分析说明
- 一份影响分析，说明会影响哪个层次，是否可能波及 ArceOS / StarryOS / Axvisor

如果题目明显涉及安全边界，还应允许选手附加：

- 崩溃输入
- 触发 PoC
- 安全影响说明

## 6. 维护者说明最少要素

`maintainer-note.md` 至少要覆盖：

- 设计意图与真实 bug
- 标准修复的大体形态
- 允许的等价修复范围
- hidden tests 设计矩阵
- 人工复核评分点
- 常见投机补丁模式

## 7. 题目包生产流程

建议对每道题按以下流程生成：

1. 在 `component-ledger.yaml` 里确认题目已经登记。
2. 复制 `templates/` 下的 6 个模板文件。
3. 填写公开题面和最小复现脚本。
4. 填写 manifest 与 test plan，登记 visible/hidden tests。
5. 填写维护者说明，保存标准答案和复核要点。
6. 在官方比赛镜像中运行 visible tests，确认题面和脚本可复现。
7. 由第二位出题人或评审人做一次干跑，确认没有歧义。

## 8. 与赛道清单的关系

- 题目包 ID 必须和 `component-ledger.yaml` 中的 `entries[].id` 对齐。
- `judge_profile` 必须使用 `judge-manifest.yaml` 中已经登记的 profile。
- 如果某题是校内试运行题，也要同步补进 `pilot-batch.yaml`。

## 9. 模板清单

本目录已提供以下模板：

- `templates/challenge.md`
- `templates/challenge-manifest.yaml`
- `templates/repro.sh`
- `templates/submission-template.md`
- `templates/test-plan.md`
- `templates/maintainer-note.md`
