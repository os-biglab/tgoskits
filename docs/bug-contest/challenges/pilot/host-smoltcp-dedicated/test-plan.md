# `host-smoltcp-dedicated` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo test -p smoltcp` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-packet` | 补畸形报文、长度边界和异常输入 | 平台侧网络输入集 |
| `hidden-regression` | 验证协议栈相邻路径未被修坏 | 平台侧隐藏回归集 |
| `hidden-state-machine` | 验证 socket 与协议状态机转移 | 平台侧状态机测试集 |

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo xtask clippy`

## 4. 题型说明

- 本题判题 profile：`host-standard`
- 预计用时：`120` 分钟
- 校内试运行目标：验证协议解析题对网安学生的吸引力与稳定性。
