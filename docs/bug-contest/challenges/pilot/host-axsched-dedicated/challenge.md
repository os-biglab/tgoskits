# 调度器专属题

- 题目 ID：`host-axsched-dedicated`
- 赛道：`host`
- 阶段：`pilot`
- 难度：`L2`
- 预计用时：`90` 分钟
- 标签：`systems-first` / `hybrid`

## 背景

本题来自 TGOSKits 校内试运行题单，目标是先验证 `调度器专属题` 这类题型是否适合作为 Host 方向的正式比赛题。
它既服务于学生训练，也服务于仓库质量提升，因此最终提交需要同时包含补丁、回归验证和根因分析。

## 题目目标

- 验证单组件专属题在预赛中的区分度。
- 参赛者应定位真实缺陷并提供最小修复，而不是只让公开样例通过。
- 参赛者应补充回归测试或等价复现脚本，说明该问题的影响范围。

## 影响范围

- 主要组件/模块：`axsched`
- 主要路径：
- `components/axsched`
- 相关系统：`Host`

## 允许修改

- `components/axsched`

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

本题当前配置的 visible commands 为：

```bash
cargo test -p axsched
```

## visible tests 通过标准

- 公开评测至少会覆盖一条与上述命令等价的功能链路。
- 评测通过不仅要求 visible tests 通过，还会执行 hidden tests 与质量门。
- 你需要修复真实问题，而不是针对样例输入做硬编码处理。

## 提交格式

请按当前目录下的 `submission-template.md` 提交以下内容：

- 最小修复补丁
- 回归测试或复现脚本
- 根因分析
- 影响范围分析

## 提示

- 本题使用的判题 profile：`host-standard`
- 典型 bug 类：`runqueue 不变量破坏`, `时间片更新错误`, `饥饿与优先级反转`
- 质量门：`cargo fmt --all -- --check`, `cargo xtask clippy`
