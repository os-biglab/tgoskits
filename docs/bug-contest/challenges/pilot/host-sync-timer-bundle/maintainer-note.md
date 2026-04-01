# `host-sync-timer-bundle` 维护者说明

本文件只供出题人、评审和比赛平台维护者使用，不对选手公开。

## 1. 出题意图

- 目标组件：`timer_list`
- 主要路径：`components/timer_list`
- 题型定位：用一个精确边界 bug 训练选手理解“时间到点即过期”的语义，而不是依赖睡眠时间或模糊等待。

## 2. 真实 bug 与标准答案

- 真实 bug 位置：`components/timer_list/src/lib.rs`
- 受影响函数：`TimerList::expire_one()`
- buggy baseline：`maintainer/bug-introducing.patch`
- 参考修复：`maintainer/solution.patch`
- 注意：`solution.patch` 只应用在已经打入 `bug-introducing.patch` 的 baseline 上，不直接作用于干净工作树。

根因：

- 正确语义是 `deadline <= now` 时事件应视为过期。
- buggy baseline 把比较从 `<=` 改成了 `<`。
- 这样“恰好到点”的事件不会触发，`Duration::ZERO` 的立即事件也会出错。

## 3. visible tests 设计意图

- 公开测试文件：`components/timer_list/tests/exact_deadline.rs`
- 公开命令：`cargo test -p timer_list --test exact_deadline`
- 目标是让选手直接观察到 exact deadline 语义被破坏，而不是把问题误判成调度或等待精度问题。

## 4. hidden tests 矩阵

| 类别 | 覆盖点 | 预期拦截的投机策略 |
| --- | --- | --- |
| `hidden-shared-deadline` | 多个事件共用同一 deadline 时都应被依次取出 | 只特判单个事件或零 deadline |
| `hidden-next-deadline` | 精确过期后下一 deadline 应正确推进 | 只让当前测试过、未恢复整体比较语义 |

隐藏测试资产：

- `maintainer/hidden-tests/timer_list_hidden.rs`
- `maintainer/run-hidden-tests.sh`

## 5. 推荐验证流程

在仓库根目录执行：

```bash
# 切到带 bug 的 baseline
git apply docs/bug-contest/challenges/pilot/host-sync-timer-bundle/maintainer/bug-introducing.patch

# 公开测试应失败
cargo test -p timer_list --test exact_deadline

# 维护者隐藏测试也应失败
bash docs/bug-contest/challenges/pilot/host-sync-timer-bundle/maintainer/run-hidden-tests.sh

# 恢复正确实现
git apply -R docs/bug-contest/challenges/pilot/host-sync-timer-bundle/maintainer/bug-introducing.patch

# 公开测试通过
cargo test -p timer_list --test exact_deadline

# 隐藏测试通过
bash docs/bug-contest/challenges/pilot/host-sync-timer-bundle/maintainer/run-hidden-tests.sh
```

## 6. 人工复核要点

- 是否只修改了 `components/timer_list`
- 是否正确恢复了 `deadline <= now` 的比较语义
- 是否补充了覆盖 exact deadline 语义的回归说明
- 是否避免用 sleep 时长、整数偏移等方式投机绕过
