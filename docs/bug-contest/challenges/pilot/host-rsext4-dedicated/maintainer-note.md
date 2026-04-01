# `host-rsext4-dedicated` 维护者说明

本文件只供出题人、评审和比赛平台维护者使用，不对选手公开。

## 1. 出题意图

- 目标组件/模块：`rsext4`, `axfs-ng-vfs`
- 主要路径：
- `components/rsext4`
- `components/axfs-ng-vfs`
- 试运行目标：验证文件系统题的说明成本与 hidden tests 命中率。

## 2. 标准答案摘要

- 真实 bug 位置：按正式出题时补充。
- 根因类别候选：`元数据解析错误`, `目录项边界处理错误`, `VFS 语义不一致`, `镜像输入健壮性问题`
- 期望修复方向：保持最小补丁、补回归测试、避免样例定制。

## 3. visible tests 设计意图

- 当前 visible commands：
- `cargo test -p rsext4`
- `cargo test -p axfs-ng-vfs`
- 它们应帮助选手进入问题现场，但不应直接暴露 hidden tests 的输入空间。

## 4. hidden tests 矩阵

| 类别 | 覆盖点 | 预期拦截的投机策略 |
| --- | --- | --- |
| `hidden-metadata` | 补目录项、extent 与元数据边界条件 | 只修公开样例、删除分支、静默吞错 |
| `hidden-regression` | 验证相邻文件系统路径未被修坏 | 只修公开样例、删除分支、静默吞错 |
| `hidden-vfs-integration` | 验证 ext4 与 VFS 的协同行为 | 只修公开样例、删除分支、静默吞错 |

## 5. 人工复核要点

- 是否只修改了允许路径
- 是否补了回归测试或等价复现脚本
- 根因分析是否与补丁一致
- 是否具备回流到正式仓库的潜力

## 6. 离线资源

- 无
