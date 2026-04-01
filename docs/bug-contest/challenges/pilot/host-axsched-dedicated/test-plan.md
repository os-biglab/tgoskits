# `host-axsched-dedicated` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo test -p axsched` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-scheduler` | 补不同调度顺序、时间片和优先级组合 | 平台侧调度回归集 |
| `hidden-regression` | 验证 runqueue 相关路径没有被修坏 | 平台侧隐藏回归集 |
| `hidden-stability` | 验证修复不会引入新的饥饿或不公平行为 | 平台侧稳定性测试集 |

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo xtask clippy`

## 4. 题型说明

- 本题判题 profile：`host-standard`
- 预计用时：`90` 分钟
- 校内试运行目标：验证单组件专属题在预赛中的区分度。
