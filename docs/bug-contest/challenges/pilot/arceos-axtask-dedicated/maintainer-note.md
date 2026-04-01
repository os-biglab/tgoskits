# `arceos-axtask-dedicated` 维护者说明

本文件只供出题人、评审和比赛平台维护者使用，不对选手公开。

## 1. 出题意图

- 目标组件/模块：`axtask`, `axsync`
- 主要路径：
- `os/arceos/modules/axtask`
- `os/arceos/modules/axsync`
- `test-suit/arceos/rust/task`
- 试运行目标：验证 QEMU 级题目的入门门槛与日志可读性。

## 2. 标准答案摘要

- 真实 bug 位置：按正式出题时补充。
- 根因类别候选：`调度与抢占时序`, `wait queue 条件竞争`, `优先级继承缺失`
- 期望修复方向：保持最小补丁、补回归测试、避免样例定制。

## 3. visible tests 设计意图

- 当前 visible commands：
- `cargo arceos test qemu --target riscv64gc-unknown-none-elf`
- 它们应帮助选手进入问题现场，但不应直接暴露 hidden tests 的输入空间。

## 4. hidden tests 矩阵

| 类别 | 覆盖点 | 预期拦截的投机策略 |
| --- | --- | --- |
| `hidden-timing` | 补不同调度时序与中断窗口 | 只修公开样例、删除分支、静默吞错 |
| `hidden-regression` | 验证相邻模块没有被修坏 | 只修公开样例、删除分支、静默吞错 |
| `hidden-integration` | 验证 test-suit 与模块接线链路 | 只修公开样例、删除分支、静默吞错 |

## 5. 人工复核要点

- 是否只修改了允许路径
- 是否补了回归测试或等价复现脚本
- 根因分析是否与补丁一致
- 是否具备回流到正式仓库的潜力

## 6. 离线资源

- `test-suit/arceos/*`
