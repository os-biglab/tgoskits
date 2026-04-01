# `host-sync-timer-bundle` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo test -p timer_list --test exact_deadline` | 事件在 `now == deadline` 时也应过期 | `expire_one()` 返回 `None` 或记录列表为空 |

公开测试文件为 `components/timer_list/tests/exact_deadline.rs`，当前包含两类断言：

- `expires_event_at_exact_deadline`
- `zero_deadline_expires_immediately`

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-shared-deadline` | 多个事件共用同一 deadline 时都能在精确边界被取出 | `maintainer/hidden-tests/timer_list_hidden.rs` |
| `hidden-next-deadline` | 精确过期后 `next_deadline()` 会正确推进到下一项 | `maintainer/hidden-tests/timer_list_hidden.rs` |

维护者可在仓库根目录执行：

```bash
bash docs/bug-contest/challenges/pilot/host-sync-timer-bundle/maintainer/run-hidden-tests.sh
```

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo clippy -p timer_list --tests`

## 4. 题型说明

- 本题判题 profile：`host-standard`
- 预计用时：`75` 分钟
- 当前正式化版本只聚焦 `components/timer_list`
