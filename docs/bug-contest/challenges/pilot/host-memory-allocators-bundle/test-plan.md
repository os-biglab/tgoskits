# `host-memory-allocators-bundle` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo test -p axallocator` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |
| `visible-02` | `cargo test -p bitmap-allocator` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |
| `visible-03` | `cargo test -p range-alloc-arceos` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-boundary` | 补额外边界值、空输入或极端输入 | 平台侧隐藏输入集 |
| `hidden-regression` | 防止只对 visible tests 打补丁 | 平台侧隐藏回归集 |
| `hidden-bundle` | 对审计包内多个分配组件交叉回归 | 平台侧交叉测试集 |

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo xtask clippy`

## 4. 题型说明

- 本题判题 profile：`host-standard`
- 预计用时：`60` 分钟
- 校内试运行目标：验证新手是否能在宿主机路径中完成基础定位与修复。
