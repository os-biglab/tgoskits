# IO 与权限审计包（`cap_access` 正式题）

- 题目 ID：`host-io-access-bundle`
- 赛道：`host`
- 阶段：`pilot-formal`
- 难度：`L1`
- 预计用时：`60` 分钟
- 标签：`security-first` / `hybrid`

## 背景

`cap_access` 是一个极小但高复用的权限包装组件，用于把对象和其能力位绑定在一起。
这类代码的危险之处不在于实现复杂，而在于一个很小的判定错误就可能把“需要全部权限”误判成“只要有任意一个权限位即可”。

本题保留了 `host-io-access-bundle` 的题包 ID，但当前正式版只聚焦 bundle 中的 `components/cap_access` 路径。
这样做是为了先把一条短路径、强语义、可稳定自动判题的权限题做成完整样板。

## 题目目标

- 修复 `WithCap::can_access()` 中的权限判定 bug。
- 让可见测试和维护者隐藏测试都恢复通过。
- 保持补丁最小，不要把能力检查改成题目定制逻辑。

## 影响范围

- 主要组件：`cap_access`
- 主要路径：`components/cap_access`
- 相关系统：`Host`

## 允许修改

- `components/cap_access`

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

请在当前题包目录 `docs/bug-contest/challenges/pilot/host-io-access-bundle/` 下执行该脚本。

本题当前公开 visible command 为：

```bash
cargo test -p cap_access --test required_capabilities
```

如果你是出题人或维护者，需要把仓库切到“带 bug 的 baseline”，可在仓库根目录执行：

```bash
git apply docs/bug-contest/challenges/pilot/host-io-access-bundle/maintainer/bug-introducing.patch
```

## 可见现象

该 bug 会把“请求多个权限位时必须全部满足”的语义，错误地放宽为“只要命中任意一个权限位就算通过”。
因此，只有部分权限的对象会被错误地当成可访问对象。

## visible tests 通过标准

- `required_capabilities` 中的所有断言都应通过。
- `cargo test -p cap_access --test required_capabilities` 退出码必须为 `0`。
- 正式评测还会运行维护者隐藏测试，覆盖空权限请求和更多权限组合。

## 提交格式

请按当前目录下的 `submission-template.md` 提交以下内容：

- 最小修复补丁
- 回归测试或复现脚本
- 根因分析
- 影响范围分析

## 提示

- 本题使用的判题 profile：`host-standard`
- 公开测试文件：`components/cap_access/tests/required_capabilities.rs`
- 维护者隐藏测试入口：`maintainer/run-hidden-tests.sh`
- 质量门：`cargo fmt --all -- --check`、`cargo clippy -p cap_access --tests`
