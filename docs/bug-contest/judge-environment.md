# 判题环境规范

本规范把比赛环境固定在仓库现有的两份“事实来源”之上：

- [container/Dockerfile](../../container/Dockerfile)
- [.github/workflows/test.yml](../../.github/workflows/test.yml)

目标不是发明一套新的评测体系，而是把比赛判题尽量做成“比赛版 CI”，让选手环境、官方环境和仓库真实验证入口尽量保持一致。
这里的“一致”指共享同类入口和版本基线，而不是机械复制 CI 中所有 self-hosted 任务。

结构化版本见 `judge-manifest.yaml`。

## 1. 官方镜像基线

比赛镜像建议直接由仓库根目录的 `container/Dockerfile` 构建，再以赛季标签发布一份镜像快照。
推荐标签形态：

```bash
docker build -t tgoskits-bug-contest:2026-s1 -f container/Dockerfile .
```

建议不要直接把 GitHub Actions 中的 `ghcr.io/<repo>-container:latest` 暴露给选手作为唯一入口，原因有两点：

- 比赛环境需要稳定冻结版本，不能随仓库 `latest` 漂移。
- 一些比赛平台不适合依赖 GHCR 鉴权拉取镜像。

## 2. 固定版本

以下版本应视为比赛赛季基线，在整个赛季内保持不变：

| 项目 | 版本/来源 | 用途 |
| --- | --- | --- |
| 基础系统 | `ubuntu:24.04` | 比赛容器底座 |
| Rust toolchain | `nightly-2026-02-25` | 与当前容器配置对齐 |
| QEMU | `10.2.1` | ArceOS / StarryOS / Axvisor 统一仿真 |
| 交叉工具链 | `x86_64` / `aarch64` / `riscv64` / `loongarch64` musl cross | 多架构编译 |
| Rust targets | `x86_64-unknown-none`、`riscv64gc-unknown-none-elf`、`aarch64-unknown-none-softfloat`、`loongarch64-unknown-none-softfloat` | 系统构建 |
| 额外工具 | `dosfstools`、`cargo-binutils`、`axconfig-gen`、`cargo-axplat` | 文件系统镜像与构建辅助 |

## 3. 统一判题原则

- `Host 题`：尽量使用 `cargo test -p <crate>` 作为主判题入口。
- `ArceOS 题`：统一固定 `cargo arceos test qemu --target riscv64gc-unknown-none-elf`。
- `StarryOS 题`：统一固定 `cargo starry test qemu --target riscv64`。
- `Axvisor 题`：统一固定 `cargo axvisor test qemu --target aarch64`，且只在决赛使用。
- `质量附加项`：统一复用 `cargo fmt --all -- --check` 与 `cargo xtask clippy`。

这样做有三个好处：

- 和现有 CI 入口尽量一致，但不复制 Axvisor 的 self-hosted `x86_64` 与开发板任务，减少“比赛环境特殊化”。
- 出题人可以直接依据仓库真实测试入口设计 visible/hidden tests。
- 赛后更容易把优秀提交转回项目的正式测试资产。

## 4. 资源配额建议

| 赛道 | CPU | 内存 | 单题超时 | 网络 |
| --- | --- | --- | --- | --- |
| Host | 2 vCPU | 4 GiB | 180 秒 | 关闭 |
| ArceOS | 4 vCPU | 6 GiB | 600 秒 | 关闭 |
| StarryOS | 4 vCPU | 8 GiB | 900 秒 | 关闭 |
| Axvisor | 6 vCPU | 12 GiB | 1200 秒 | 关闭 |

补充建议：

- 所有题目默认 `--network none`，避免比赛平台被当成下载器或扫描器。
- 保留失败产物，至少保存 `target/` 和 `os/axvisor/tmp/` 下的关键日志与镜像。
- 允许缓存 Cargo registry 与 git index，但不允许在评测运行时联网下载新 guest 镜像。

## 5. 各赛道判题配置

### 5.1 Host 题

推荐入口：

```bash
cargo test -p <crate>
```

附加质量门：

```bash
cargo fmt --all -- --check
cargo xtask clippy
```

适用对象：

- `scripts/test/std_crates.csv` 中已经纳入 host 测试的包
- 仅依赖宿主机单元测试即可稳定判断正确性的基础组件

### 5.2 ArceOS 题

统一主入口：

```bash
cargo arceos test qemu --target riscv64gc-unknown-none-elf
```

出题建议：

- 优先复用 `test-suit/arceos/` 里的现有 harness。
- 一题只聚焦一条系统链路，例如任务调度、文件系统或网络链路，不要把多个系统问题混在一起。
- 如果某题只依赖单一示例，也可以给选手保留一个更轻量的本地复现命令，但最终官方评测仍应回到统一入口。

### 5.3 StarryOS 题

统一主入口：

```bash
cargo starry test qemu --target riscv64
```

额外要求：

- rootfs 必须由主办方预先放入官方环境。
- 不允许要求选手在比赛时运行额外下载脚本。
- hidden tests 最好以“不同用户态程序输入”或“不同 syscall 组合”形式存在，而不是改变基础环境。

### 5.4 Axvisor 题

统一主入口：

```bash
cargo axvisor test qemu --target aarch64
```

额外要求：

- 所有 `guest` 镜像、`vmconfigs`、`rootfs` 和 QEMU 配置都要在镜像或挂载卷中提前准备好。
- Axvisor 题默认放到决赛或专题高阶赛，不进入大规模预赛判题集群。
- 如果题目依赖串口正则判定，要把标准答案中的 success/fail 模式固化在维护者说明中，避免口径漂移。

## 6. 离线资源布局

推荐在比赛平台侧单独挂载 `contest-assets/`，而不是把大镜像直接提交到仓库：

```text
contest-assets/
├── starry/
│   └── rootfs-riscv64.img
└── axvisor/
    ├── guests/
    ├── rootfs/
    └── vmconfigs/
```

组织规则：

- 仓库中只保存“资源位置约定”和“生成/使用方法”。
- 比赛平台保存实际大文件。
- hidden tests 与私密输入同样放在平台侧，不进入公开仓库。
- 平台侧资源路径和容器内运行路径可以不同，但必须固定映射关系：
  - `contest-assets/starry/rootfs-riscv64.img` 挂载到 `target/starry/rootfs-riscv64.img`
  - `contest-assets/axvisor/rootfs.img` 挂载到 `os/axvisor/tmp/rootfs.img`
  - `contest-assets/axvisor/vmconfigs/*.generated.toml` 挂载到 `os/axvisor/tmp/vmconfigs/*.generated.toml`

## 7. 评测流水线建议

建议每次选手提交后按以下顺序执行：

1. 检查提交格式是否完整。
2. 拉起官方比赛镜像。
3. 挂载只读仓库工作目录和平台侧私密资源。
4. 运行 visible tests。
5. 运行 hidden tests。
6. 运行格式与 clippy 质量门。
7. 收集日志、diff、失败产物。
8. 进入人工复核或自动计分。

## 8. hidden tests 的放置原则

- hidden tests 不进入仓库，不跟随公开题目包发布。
- 对 Host 题，hidden tests 优先覆盖额外边界条件，而不是完全不同的语义。
- 对 ArceOS / StarryOS / Axvisor 题，hidden tests 优先覆盖不同输入、不同调度顺序、不同资源布局，避免选手只对公开样例打补丁。
- 只要 hidden tests 引入新的环境变量或额外镜像，都必须先记录到 `judge-manifest.yaml`。

## 9. 推荐的最小命令集

对主办方来说，整场比赛真正需要冻结的命令只有 7 条：

```bash
cargo test -p <crate>
cargo xtask test
cargo fmt --all -- --check
cargo xtask clippy
cargo arceos test qemu --target riscv64gc-unknown-none-elf
cargo starry test qemu --target riscv64
cargo axvisor test qemu --target aarch64
```

其中 `cargo xtask test` 更适合在赛后回收测试资产时使用，用来确认 host 路径上的回归测试能正式并入项目。
