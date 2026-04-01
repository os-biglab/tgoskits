# `arceos-axtask-dedicated` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo arceos test qemu --target riscv64gc-unknown-none-elf` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-timing` | 补不同调度时序与中断窗口 | 平台侧时序回归集 |
| `hidden-regression` | 验证相邻模块没有被修坏 | 平台侧回归集 |
| `hidden-integration` | 验证 test-suit 与模块接线链路 | 平台侧集成测试集 |

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo xtask clippy`

## 4. 题型说明

- 本题判题 profile：`arceos-riscv64`
- 预计用时：`120` 分钟
- 校内试运行目标：验证 QEMU 级题目的入门门槛与日志可读性。
