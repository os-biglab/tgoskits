# `host-io-access-bundle` 维护者说明

本文件只供出题人、评审和比赛平台维护者使用，不对选手公开。

## 1. 出题意图

- 目标组件：`cap_access`
- 主要路径：`components/cap_access`
- 题型定位：用一个极小的权限判定 bug，训练选手识别“全部权限”与“任意权限”之间的语义差异。

## 2. 真实 bug 与标准答案

- 真实 bug 位置：`components/cap_access/src/lib.rs`
- 受影响函数：`WithCap::can_access()`
- buggy baseline：`maintainer/bug-introducing.patch`
- 参考修复：`maintainer/solution.patch`
- 注意：`solution.patch` 只应用在已经打入 `bug-introducing.patch` 的 baseline 上，不直接作用于干净工作树。

根因：

- 正确语义是“请求的所有权限位都必须被对象持有”。
- buggy baseline 把 `contains(cap)` 改成了 `intersects(cap)`。
- 这会把“需要 `READ | WRITE`”误判为“只要有 `READ` 或 `WRITE` 任意一个就行”。

## 3. visible tests 设计意图

- 公开测试文件：`components/cap_access/tests/required_capabilities.rs`
- 公开命令：`cargo test -p cap_access --test required_capabilities`
- 目标是让选手直接看到“组合权限请求被错误放行”的现象，而不是去猜平台环境问题。

## 4. hidden tests 矩阵

| 类别 | 覆盖点 | 预期拦截的投机策略 |
| --- | --- | --- |
| `hidden-empty-request` | `Cap::empty()` 也应保持可访问 | 只特判 `READ | WRITE` 组合 |
| `hidden-combo-request` | `READ | EXECUTE` 等组合不能按任意位放行 | 只修公开测试里的特定组合 |

隐藏测试资产：

- `maintainer/hidden-tests/cap_access_hidden.rs`
- `maintainer/run-hidden-tests.sh`

## 5. 推荐验证流程

在仓库根目录执行：

```bash
# 切到带 bug 的 baseline
git apply docs/bug-contest/challenges/pilot/host-io-access-bundle/maintainer/bug-introducing.patch

# 公开测试应失败
cargo test -p cap_access --test required_capabilities

# 维护者隐藏测试也应失败
bash docs/bug-contest/challenges/pilot/host-io-access-bundle/maintainer/run-hidden-tests.sh

# 恢复正确实现
git apply -R docs/bug-contest/challenges/pilot/host-io-access-bundle/maintainer/bug-introducing.patch

# 公开测试通过
cargo test -p cap_access --test required_capabilities

# 隐藏测试通过
bash docs/bug-contest/challenges/pilot/host-io-access-bundle/maintainer/run-hidden-tests.sh
```

## 6. 人工复核要点

- 是否修改了 `components/cap_access` 之外的路径
- 是否把权限判定继续写成“任意位匹配”
- 是否补充了真正覆盖组合权限语义的测试说明
- 是否正确解释了 `contains` 与 `intersects` 的语义差异
