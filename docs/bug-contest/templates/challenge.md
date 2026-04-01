# <题目标题>

- 题目 ID：`<challenge-id>`
- 赛道：`<track>`
- 阶段：`<phase>`
- 难度：`<difficulty>`
- 标签：`systems-first` / `security-first` / `hybrid`

## 背景

用 2-4 段说明这个组件或系统链路的角色，以及本题想让选手理解什么。

## 题目目标

- 说明选手需要定位什么类型的问题。
- 说明预期提交的是“修复 + 回归测试/复现脚本 + 根因分析”，而不是只交一个能过样例的补丁。

## 影响范围

- 主要组件/模块：`<component-list>`
- 主要路径：`<path-list>`
- 相关系统：`ArceOS` / `StarryOS` / `Axvisor` / `Host`

## 允许修改

- `<allowed-path-1>`
- `<allowed-path-2>`

## 禁止修改

- 不允许修改 hidden tests 或平台侧资源。
- 不允许删除公开测试来绕过问题。
- 不允许把题目变成“直接 return 固定值”的投机补丁。

## 最小复现

优先使用仓库根目录下的统一入口，或直接运行：

```bash
./repro.sh
```

如需手动运行，请给出一条最短命令：

```bash
<visible-command>
```

## visible tests 通过标准

- 列出 visible tests 对应的行为，而不是隐藏答案。
- 说明日志中什么现象代表“问题仍然存在”。
- 说明最终官方评测还会执行额外 hidden tests 和质量门。

## 提交格式

请按 `submission-template.md` 提交以下内容：

- 最小修复补丁
- 回归测试或复现脚本
- 根因分析
- 影响范围分析

## 提示

- 如题目涉及 QEMU 或 rootfs，说明官方环境已经预置所需资源。
- 如题目涉及安全边界，提醒选手分析输入可信度与影响面。
