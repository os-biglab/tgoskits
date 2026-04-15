# AI 驱动的内核改进持续迭代框架

TGOSKits 适合做成一个持续运行的 AI 迭代工作台：输入一个内核/OS/应用支持目标，自动拆成计划、实现、评审、测试、调试和文档六个阶段，再把执行动作落到现有的 `cargo xtask`、`scripts/check.sh` 和 `scripts/test.sh` 上。

## 1. 目标

- 支持内核改进、系统集成和 Linux 应用适配
- 把 AI 产出变成可审计、可复跑、可回滚的运行记录
- 让“规划 → 编写代码 → review → 测试 → debug → 文档”成为固定闭环

## 2. 最小可用集

建议先落地 6 个角色：

| 角色 | 职责 |
| --- | --- |
| `orchestrator` | 统一入口、拆分阶段、控制门禁 |
| `planner` | 产出实施计划和验证路线 |
| `implementer` | 生成最小补丁 |
| `reviewer` | 检查正确性和回归风险 |
| `test-debugger` | 读取测试日志并指导修复 |
| `doc-writer` | 生成或更新文档 |

这套最小集已经足够把一个需求跑成完整 pipeline。

## 3. 运行方式

仓库里提供了一个轻量 runner：

```bash
python3 scripts/ai_framework.py validate --manifest .copilot/framework/manifests/linux-app-example.toml
python3 scripts/ai_framework.py plan --manifest .copilot/framework/manifests/linux-app-example.toml
python3 scripts/ai_framework.py run --manifest .copilot/framework/manifests/linux-app-example.toml
```

运行后会在 `.copilot/runs/<task-id>/<timestamp>/` 下生成：

- 原始 manifest
- `framework.config.toml` 的副本（如果存在）
- 标准化 manifest
- 每个 stage 的 prompt bundle
- stage 状态文件
- stage 日志（如果执行了命令）
- 每次重试的 attempt prompt / copilot log / command log

## 4. Manifest 结构

`scripts/ai_framework.py` 读取 TOML manifest。最小结构如下：

```toml
[task]
id = "linux-app-example"
title = "Support a Linux application"
target = "StarryOS"
description = "Make nginx boot on StarryOS."
allowed_paths = ["components/", "os/StarryOS/", "os/arceos/", "docs/"]
reference_docs = [
  "README.md",
  "docs/demo.md",
  "docs/starryos-guide.md",
  "docs/components.md",
]
acceptance_criteria = [
  "nginx can boot under StarryOS",
  "nginx launches successfully under StarryOS",
]
output_dir = ".copilot/runs"

[[stages]]
name = "plan"
agent = "planner"

[[stages]]
name = "test"
agent = "test-debugger"
command = ["bash", "-lc", "cargo xtask starry rootfs --arch riscv64 && ${NGINX_SMOKE_CMD:?set NGINX_SMOKE_CMD}"]
```

`agent` 取值对应 `.copilot/framework/prompts/*.md` 中的模板文件名。

`reference_docs` 会被直接展开进每个 stage 的 prompt，建议至少包含：

- `README.md`：先看快速导航，知道该用哪个系统入口
- `docs/demo.md`：知道新增功能、修改已有功能时应遵守的贡献方式
- `docs/components.md`：知道改动应落在哪一层组件
- `docs/starryos-guide.md`：知道 StarryOS 的 rootfs、qemu 和验证入口

对于 nginx 这类 Linux 应用，`test` 阶段最好不要只做“启动系统”，而是让命令直接执行一个烟测脚本，例如：

```bash
python3 scripts/ai_framework.py run --manifest .copilot/framework/manifests/nginx.toml
```

当前仓库已经提供了 `scripts/run_nginx_smoke.py`，它会自动：

- 检查并自动下载 `riscv64-linux-musl-cross` 工具链（缺失时）
- 自动构建 StarryOS kernel（缺失时）
- 下载 StarryOS rootfs
- 将 `nginx` 和所需的 `pcre2` 直接注入 rootfs
- 启动 StarryOS QEMU（不依赖 guest 网络）
- 启动 `nginx`
- 通过启动结果判断 nginx 是否成功拉起

你可以直接运行：

```bash
python3 scripts/ai_framework.py run --manifest .copilot/framework/manifests/nginx.toml
```

首次运行时间会更长，因为会自动完成工具链、kernel 和 rootfs 的准备。

> 备注：`run_nginx_smoke.py` 会把 nginx 相关包离线写进 rootfs，因此不依赖 QEMU 的 user networking / slirp。

这样 `test-debugger` 会同时看到：

- StarryOS/rootfs 的启动动作
- nginx 的启动和拉起验证动作
- 失败日志和重试上下文

## 4.1 Model 与 Copilot 调用配置

如果你想让每个阶段使用不同的 model，并自动调用 Copilot CLI，只需要修改 `.copilot/framework/config.toml`。

示例：

```toml
model = "gpt-5-mini"
copilot_cmd = "copilot"
copilot_output_format = "json"
resume_sessions = true
copilot_args = ["--allow-all-tools", "--allow-all-paths", "--allow-all-urls", "--no-ask-user"]

[stage_models]
intake = "gpt-5-mini"
plan = "gpt-5-mini"
implement = "gpt-5-mini"
review = "gpt-5-mini"
test = "gpt-5-mini"
docs = "gpt-5-mini"

[default_stage_policy]
invoke_copilot = true
autopilot = false
max_attempts = 1
retry_on_failure = false

[stage_policies.implement]
invoke_copilot = true
autopilot = true
max_attempts = 4
retry_on_failure = true

[stage_policies.test]
invoke_copilot = true
autopilot = true
max_attempts = 4
retry_on_failure = true
```

规则：

- `model` 是默认值
- `stage_models.<stage-name>` 优先级最高
- 如果没有 stage-name 命中，会继续尝试 `stage_models.<agent-name>`
- 如果都没有匹配，就回退到 `model`
- `stage_policies.<stage-name>` 优先级最高；其次是 `stage_policies.<agent-name>`；最后回退到 `default_stage_policy`
- `invoke_copilot=true` 时会自动调用 Copilot CLI
- `autopilot=true` 时会为该 stage 打开 autopilot 模式
- `max_attempts + retry_on_failure` 控制失败后的自动循环重试
- `resume_sessions=true` 时，每次 Copilot 调用都会自动复用该 stage 上一次 session
- `copilot_output_format="json"` 时，runner 可以稳定解析 sessionId 并写入 stage 状态

runner 会把最终解析出的 model、policy、sessionId、attempt 结果写进 `manifest.normalized.json` 和每个 stage 的状态文件，方便后续自动化调用层直接读取。

## 5. Prompt、skills、hooks

### Prompts

prompt 模板统一放在：

- `.copilot/framework/prompts/orchestrator.md`
- `.copilot/framework/prompts/planner.md`
- `.copilot/framework/prompts/implementer.md`
- `.copilot/framework/prompts/reviewer.md`
- `.copilot/framework/prompts/test-debugger.md`
- `.copilot/framework/prompts/doc-writer.md`

### Skill / Agent

Copilot skill 放在：

- `.claude/skills/ai-iteration-framework/SKILL.md`
- `.claude/skills/ai-iteration-framework/agents/openai.yaml`

它把整个框架入口收束成一个可调用技能。

### Hooks

hook 模板放在：

- `.copilot/framework/hooks/pre-task.sh`
- `.copilot/framework/hooks/post-patch.sh`
- `.copilot/framework/hooks/post-test-fail.sh`
- `.copilot/framework/hooks/pre-merge.sh`
- `.copilot/framework/hooks/post-merge.sh`

这些脚本是最小模板，适合接到外部自动化或本地工作流中。

## 6. Linux 应用支持 pipeline

如果目标是支持某个 Linux 应用，推荐流程是：

1. `orchestrator` 读应用目标、架构和约束
2. `planner` 拆出 syscall、VFS、进程、网络、设备和配置缺口
3. `implementer` 先补基础能力，再补应用特化
4. `reviewer` 检查是否引入无必要的特化路径
5. `test-debugger` 先跑最小启动，再跑功能验证，再跑回归
6. `doc-writer` 记录启动方式、已知限制、验证命令

## 7. 典型落点

- ArceOS：`components/`、`os/arceos/modules/`、`os/arceos/api/`、`os/arceos/ulib/`
- StarryOS：`components/starry-*`、`os/StarryOS/kernel/`
- Axvisor：`components/axvm`、`components/axvcpu`、`os/axvisor/`
- 统一验证：`cargo xtask test`、`cargo xtask clippy`、对应系统的 `qemu` / `test qemu`

## 8. 当前建议

先把最小集跑通，再扩展专门 agent：

- kernel-specialist
- linux-app-adapter
- platform-specialist

这样可以先保证框架跑得动，再逐步增强自动化能力。
