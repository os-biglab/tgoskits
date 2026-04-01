# `arceos-axfs-dedicated` 维护者说明

本文件只供出题人、评审和比赛平台维护者使用，不对选手公开。

## 1. 出题意图

- 目标组件/模块：`axfs`, `axruntime`
- 主要路径：
- `os/arceos/modules/axfs`
- `os/arceos/modules/axruntime`
- `test-suit/arceos/rust/fs`
- 试运行目标：验证块设备与文件系统链路题在官方镜像中的稳定性。

## 2. 标准答案摘要

- 真实 bug 位置：按正式出题时补充。
- 根因类别候选：`路径解析错误`, `块设备接线错误`, `shell 命令回归`
- 期望修复方向：保持最小补丁、补回归测试、避免样例定制。

## 3. visible tests 设计意图

- 当前 visible commands：
- `cargo arceos test qemu --target riscv64gc-unknown-none-elf`
- 它们应帮助选手进入问题现场，但不应直接暴露 hidden tests 的输入空间。

## 4. hidden tests 矩阵

| 类别 | 覆盖点 | 预期拦截的投机策略 |
| --- | --- | --- |
| `hidden-fs-edge` | 补路径解析、目录操作与文件系统边界条件 | 只修公开样例、删除分支、静默吞错 |
| `hidden-block-path` | 验证块设备链路与 shell 命令协同 | 只修公开样例、删除分支、静默吞错 |
| `hidden-regression` | 验证相邻运行时与文件系统路径未回归 | 只修公开样例、删除分支、静默吞错 |

## 5. 人工复核要点

- 是否只修改了允许路径
- 是否补了回归测试或等价复现脚本
- 根因分析是否与补丁一致
- 是否具备回流到正式仓库的潜力

## 6. 离线资源

- `test-suit/arceos/*`
