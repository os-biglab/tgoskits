# `host-io-access-bundle` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo test -p cap_access --test required_capabilities` | 请求多个权限位时必须全部满足 | `read_only` 或 `exec_only` 被错误放行 |

公开测试文件为 `components/cap_access/tests/required_capabilities.rs`，当前包含两类断言：

- `requires_all_requested_capabilities`
- `access_or_err_rejects_missing_bits`

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-empty-request` | `Cap::empty()` 请求必须始终允许访问 | `maintainer/hidden-tests/cap_access_hidden.rs` |
| `hidden-combo-request` | 多权限组合请求不能按“任意位命中”放行 | `maintainer/hidden-tests/cap_access_hidden.rs` |

维护者可在仓库根目录执行：

```bash
bash docs/bug-contest/challenges/pilot/host-io-access-bundle/maintainer/run-hidden-tests.sh
```

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo clippy -p cap_access --tests`

## 4. 题型说明

- 本题判题 profile：`host-standard`
- 预计用时：`60` 分钟
- 当前正式化版本只聚焦 `components/cap_access`
