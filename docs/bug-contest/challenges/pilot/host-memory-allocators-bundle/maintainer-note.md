# `host-memory-allocators-bundle` 维护者说明

本文件只供出题人、评审和比赛平台维护者使用，不对选手公开。

## 1. 出题意图

- 目标组件/模块：`axallocator`, `bitmap-allocator`, `range-alloc-arceos`
- 主要路径：
- `components/axallocator`
- `components/bitmap-allocator`
- `components/range-alloc-arceos`
- 试运行目标：验证新手是否能在宿主机路径中完成基础定位与修复。

## 2. 标准答案摘要

- 真实 bug 位置：按正式出题时补充。
- 根因类别候选：`边界条件`, `双重回收`, `对齐与碎片控制`, `空闲区间合并错误`
- 期望修复方向：保持最小补丁、补回归测试、避免样例定制。

## 3. visible tests 设计意图

- 当前 visible commands：
- `cargo test -p axallocator`
- `cargo test -p bitmap-allocator`
- `cargo test -p range-alloc-arceos`
- 它们应帮助选手进入问题现场，但不应直接暴露 hidden tests 的输入空间。

## 4. hidden tests 矩阵

| 类别 | 覆盖点 | 预期拦截的投机策略 |
| --- | --- | --- |
| `hidden-boundary` | 补额外边界值、空输入或极端输入 | 只修公开样例、删除分支、静默吞错 |
| `hidden-regression` | 防止只对 visible tests 打补丁 | 只修公开样例、删除分支、静默吞错 |
| `hidden-bundle` | 对审计包内多个分配组件交叉回归 | 只修公开样例、删除分支、静默吞错 |

## 5. 人工复核要点

- 是否只修改了允许路径
- 是否补了回归测试或等价复现脚本
- 根因分析是否与补丁一致
- 是否具备回流到正式仓库的潜力

## 6. 离线资源

- 无
