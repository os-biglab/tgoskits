# `arceos-axtask-dedicated` 提交说明

本题是 `ArceOS 任务与同步专属题` 的试运行 challenge 包，提交时请围绕以下上下文组织说明。

## 1. 补丁摘要

- 修改路径建议限定在：
- `os/arceos/modules/axtask`
- `os/arceos/modules/axsync`
- `test-suit/arceos/rust/task`
- 涉及组件：`axtask`, `axsync`
- 你修复了什么问题：

## 2. 根因分析

请说明：

- 问题如何触发。
- 根因属于哪一类：`调度与抢占时序`, `wait queue 条件竞争`, `优先级继承缺失`。
- 为什么这不是单纯的样例失败，而是一个真实缺陷。

## 3. 修复方案

请说明：

- 你改了哪些逻辑。
- 为什么这是最小修复。
- 是否考虑过其他修法，以及为什么没有采用。

## 4. 回归验证

请至少给出你实际运行过的命令与结果摘要：

```bash
cargo arceos test qemu --target riscv64gc-unknown-none-elf
```

## 5. 影响分析

请说明这个问题会影响：

- 哪个组件或模块
- 哪类用户或运行场景
- 是否可能波及 `ArceOS` 或其他系统链路
