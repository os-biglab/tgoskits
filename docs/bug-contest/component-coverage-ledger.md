# 组件覆盖台账

本台账把比赛题库固定为四层结构：`components/*`、ArceOS、StarryOS、Axvisor。
设计目标不是“给每个组件各出一道题”，而是保证题库既能覆盖关键组件，又能被大学生在有限时间内完成。

结构化版本见 `component-ledger.yaml`。本文适合组织者直接阅读和开会讨论。

## 选题原则

- `审计包`：把 3-5 个知识点相近、判题入口相近的小组件组合成一个 challenge bundle，适合热身和预赛。
- `专属题`：面向高价值或上下文复杂的单组件/单链路题目，强调更完整的复现与回归测试。
- `决赛专属`：依赖 rootfs、guest 镜像、vmconfig 或复杂 QEMU 配置的题目，只放入决赛。
- `开放披露`：不公开题面，允许选手在比赛周期内提交真实问题、补丁和回归测试。

## 赛季结构建议

| 赛层 | 题目数量建议 | 主要来源 | 目标 |
| --- | --- | --- | --- |
| 热身 | 2-3 道 | `components/*` + 1 道 ArceOS | 教会选手进入仓库、跑最小命令 |
| 线上预赛 | 16-20 道 | Host 组件 + ArceOS + 少量 Starry | 兼顾可比性、自动判题和学习曲线 |
| 决赛 | 6-8 道 | Starry 深水区 + Axvisor + 跨系统基础组件 | 拉开梯度，验证系统推理能力 |
| 开放披露 | 全赛季开放 | 全仓库 | 直接提升仓库质量 |

## `components/*` 赛道

这一层优先复用 `scripts/test/std_crates.csv` 中已经进入 host 测试路径的包。
原则是让选手先在宿主机上完成定位、修复和回归，再逐步进入系统级题目。

| 题目 ID | 进入方式 | 覆盖组件 | 主要 bug 类 | 推荐判题配置 | 面向对象 |
| --- | --- | --- | --- | --- | --- |
| `host-memory-allocators-bundle` | 审计包 | `axallocator`、`bitmap-allocator`、`range-alloc-arceos` | 分配边界、双重回收、空闲区间合并 | `host-standard` | `systems-first`、`hybrid` |
| `host-sync-timer-bundle` | 审计包 | `kspin`、`lazyinit`、`timer_list`、`cpumask`、`int_ratio` | 锁作用域、初始化时序、定时器重排 | `host-standard` | `systems-first`、`hybrid` |
| `host-io-access-bundle` | 审计包 | `axio`、`axpoll`、`cap_access`、`axerrno` | 权限检查、状态转换、错误码映射 | `host-standard` | `security-first`、`hybrid` |
| `host-axsched-dedicated` | 专属题 | `axsched` | runqueue 不变量、抢占与时间片 | `host-standard` | `systems-first`、`hybrid` |
| `host-rsext4-dedicated` | 专属题 | `rsext4`、`axfs-ng-vfs` | 元数据健壮性、路径语义、输入边界 | `host-standard` | `security-first`、`hybrid` |
| `host-smoltcp-dedicated` | 专属题 | `smoltcp` | 报文解析、状态机、分片重组 | `host-standard` | `security-first`、`hybrid` |

## ArceOS 赛道

ArceOS 赛道优先固定在 `riscv64gc-unknown-none-elf`，统一使用 `cargo arceos test qemu --target riscv64gc-unknown-none-elf` 作为主评测入口。
这样既能对齐现有 CI，又能控制比赛维护成本。

| 题目 ID | 进入方式 | 覆盖组件/路径 | 重点 harness | 主要 bug 类 | 阶段 |
| --- | --- | --- | --- | --- | --- |
| `arceos-axtask-dedicated` | 专属题 | `axtask`、`axsync`、`test-suit/arceos/rust/task/*` | `arceos-parallel`、`arceos-priority`、`arceos-wait-queue` | 调度时序、wait queue 竞争 | 热身 / 预赛 |
| `arceos-axnet-dedicated` | 专属题 | `axnet`、`axdriver`、`test-suit/arceos/rust/net/*` | `arceos-httpclient`、`arceos-httpserver` | 网络接线、初始化竞态、socket 生命周期 | 预赛 |
| `arceos-axfs-dedicated` | 专属题 | `axfs`、`axruntime`、`test-suit/arceos/rust/fs/*` | `arceos-shell` | 路径解析、块设备链路、shell 回归 | 预赛 |
| `arceos-axhal-driver-dedicated` | 专属题 | `axhal`、`axdriver`、`axconfig` | 以平台/驱动组合题为主 | 中断初始化、寄存器配置、feature 组合 | 预赛 / 决赛 |
| `arceos-runtime-feature-dedicated` | 专属题 | `axruntime`、`axfeat`、`arceos_api`、`axstd` | API 与 feature 接线 | feature gate 漏接、API 语义不一致 | 预赛 |
| `arceos-platform-bringup-dedicated` | 专属题 | `axplat-*`、`axhal` | 平台 bring-up | 早期启动映射、串口/时钟配置 | 决赛 |

## StarryOS 赛道

StarryOS 赛道默认固定在 `riscv64`，并要求 rootfs 由官方环境预置。
这层题目更适合做“Linux 兼容性 + 用户态行为 + 内核修复”的组合题。

| 题目 ID | 进入方式 | 覆盖组件/路径 | 主要 bug 类 | 阶段 |
| --- | --- | --- | --- | --- |
| `starry-process-signal-dedicated` | 专属题 | `starry-process`、`starry-signal`、`os/StarryOS/kernel` | 信号递送、进程状态迁移、兼容性差异 | 预赛 / 决赛 |
| `starry-vm-dedicated` | 决赛专属 | `starry-vm`、`os/StarryOS/kernel` | 地址空间边界、缺页与权限错误 | 决赛 |
| `starry-syscall-compat-dedicated` | 专属题 | `os/StarryOS/kernel`、`os/StarryOS/starryos` | errno/返回值不一致、用户缓冲区检查 | 预赛 / 决赛 |
| `starry-rootfs-apps-dedicated` | 决赛专属 | `test-suit/starryos`、`os/StarryOS/starryos` | rootfs 用户程序诱发的系统回归 | 决赛 |

## Axvisor 赛道

Axvisor 相关题目只建议在决赛使用。
原因不是它不重要，而是它对 guest 镜像、`vmconfigs`、板级配置和离线资源的依赖明显更重，如果提前放到预赛会把比赛变成“拼环境大赛”。

| 题目 ID | 进入方式 | 覆盖组件/路径 | 主要 bug 类 | 阶段 |
| --- | --- | --- | --- | --- |
| `axvisor-vcpu-state-dedicated` | 决赛专属 | `axvcpu`、`arm_vcpu`、`riscv_vcpu`、`os/axvisor/src` | vCPU 状态保存恢复、多核同步 | 决赛 |
| `axvisor-guest-memory-dedicated` | 决赛专属 | `axaddrspace`、`axvm`、`os/axvisor/src` | guest 地址边界、页权限与异常路径 | 决赛 |
| `axvisor-device-emulation-dedicated` | 决赛专属 | `axdevice`、`axdevice_base`、`os/axvisor/src` | MMIO、virtio 状态机、输入验证 | 决赛 |
| `axvisor-hypercall-boundary-dedicated` | 决赛专属 | `axhvc`、`axvisor_api`、`os/axvisor/src` | Hypercall 参数校验、共享内存边界 | 决赛 |
| `axvisor-board-vmconfig-dedicated` | 决赛专属 | `axvmconfig`、`platform/*`、`os/axvisor/configs/*` | 板级与 VM 配置不一致、启动参数错误 | 决赛 |

## 开放披露赛道

| 题目 ID | 进入方式 | 覆盖范围 | 交付物 | 说明 |
| --- | --- | --- | --- | --- |
| `open-responsible-disclosure-lane` | 私密提交 | `components/`、`os/arceos/`、`os/StarryOS/`、`os/axvisor/` | 漏洞报告、补丁、回归测试 | 直接服务仓库质量改进，不与公开题面混用 |

## 建议的题库配比

- 预赛：
  - `components/*` 赛道 6 道
  - ArceOS 赛道 5-6 道
  - StarryOS 赛道 3-4 道
  - 开放披露赛道持续开放
- 决赛：
  - Axvisor 赛道 3-4 道
  - StarryOS 深水区 2 道
  - ArceOS / 跨系统基础组件 2 道

## 台账维护规则

- 任何新题都要先补到 `component-ledger.yaml`，再进入出题流程。
- 如果一个基础组件已经在预赛中作为主赛题出现，不要在同一赛季重复出同质题。
- 决赛题必须在台账中明确依赖的离线资源，特别是 rootfs、guest 镜像和 `vmconfigs`。
- 真实披露题一旦确认可合入，应在赛后把回归测试回流到 `cargo xtask test`、`test-suit/arceos`、`test-suit/starryos` 或 Axvisor 测试入口。
