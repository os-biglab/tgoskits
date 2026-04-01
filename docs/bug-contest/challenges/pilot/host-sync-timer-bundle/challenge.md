# 同步与计时审计包（`timer_list` 正式题）

- 题目 ID：`host-sync-timer-bundle`
- 赛道：`host`
- 阶段：`pilot-formal`
- 难度：`L1`
- 预计用时：`75` 分钟
- 标签：`systems-first` / `hybrid`

## 背景

`timer_list` 是一个很小的定时事件容器，但它的语义边界非常明确：
只要 `now` 已经到达事件 deadline，该事件就应该被视为过期并返回。

这类时间边界 bug 很适合作为正式题，因为：

- 复现链路短
- 语义清晰
- 既能训练系统方向学生，也适合做自动判题

本题保留了 `host-sync-timer-bundle` 的题包 ID，但当前正式版只聚焦 bundle 中的 `components/timer_list` 路径。

## 题目目标

- 修复 `TimerList::expire_one()` 中的 deadline 边界 bug。
- 保证“恰好等于 deadline”时事件也会过期。
- 让公开测试和维护者隐藏测试都恢复通过。

## 影响范围

- 主要组件：`timer_list`
- 主要路径：`components/timer_list`
- 相关系统：`Host`

## 允许修改

- `components/timer_list`

## 禁止修改

- `docs/bug-contest`
- `.github`
- `container`
- `scripts/repo`

## 最小复现

优先直接运行：

```bash
./repro.sh
```

请在当前题包目录 `docs/bug-contest/challenges/pilot/host-sync-timer-bundle/` 下执行该脚本。

本题当前公开 visible command 为：

```bash
cargo test -p timer_list --test exact_deadline
```

如果你是出题人或维护者，需要把仓库切到“带 bug 的 baseline”，可在仓库根目录执行：

```bash
git apply docs/bug-contest/challenges/pilot/host-sync-timer-bundle/maintainer/bug-introducing.patch
```

## 可见现象

该 bug 会让“当前时间恰好等于 deadline”的事件无法过期，表现为：

- 事件在应当触发的时刻返回 `None`
- `Duration::ZERO` 的立即触发事件也会被错误延后

## visible tests 通过标准

- `exact_deadline` 中的所有断言都应通过。
- `cargo test -p timer_list --test exact_deadline` 退出码必须为 `0`。
- 正式评测还会运行维护者隐藏测试，覆盖多个同 deadline 事件和后续 deadline 推进。

## 提交格式

请按当前目录下的 `submission-template.md` 提交以下内容：

- 最小修复补丁
- 回归测试或复现脚本
- 根因分析
- 影响范围分析

## 提示

- 本题使用的判题 profile：`host-standard`
- 公开测试文件：`components/timer_list/tests/exact_deadline.rs`
- 维护者隐藏测试入口：`maintainer/run-hidden-tests.sh`
- 质量门：`cargo fmt --all -- --check`、`cargo clippy -p timer_list --tests`
