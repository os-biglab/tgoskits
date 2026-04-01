# Pilot Challenge Packages

本目录包含 `pilot-batch.yaml` 中 8 道校内试运行题的 challenge 包骨架。
这些目录不是标准答案，而是供主办方继续填充题面、隐藏测试与维护者说明的起点。

## 题目索引

| 题目 ID | 标题 | 赛道 | 难度 | 预计用时 | 目录 |
| --- | --- | --- | --- | --- | --- |
| `host-memory-allocators-bundle` | 内存分配审计包 | `host` | `L1` | `60` 分钟 | `challenges/pilot/host-memory-allocators-bundle` |
| `host-sync-timer-bundle` | 同步与计时审计包 | `host` | `L1` | `75` 分钟 | `challenges/pilot/host-sync-timer-bundle` |
| `host-io-access-bundle` | IO 与权限审计包 | `host` | `L1` | `60` 分钟 | `challenges/pilot/host-io-access-bundle` |
| `host-axsched-dedicated` | 调度器专属题 | `host` | `L2` | `90` 分钟 | `challenges/pilot/host-axsched-dedicated` |
| `host-rsext4-dedicated` | ext4 与 VFS 专属题 | `host` | `L2` | `120` 分钟 | `challenges/pilot/host-rsext4-dedicated` |
| `host-smoltcp-dedicated` | 网络协议栈专属题 | `host` | `L2` | `120` 分钟 | `challenges/pilot/host-smoltcp-dedicated` |
| `arceos-axtask-dedicated` | ArceOS 任务与同步专属题 | `arceos` | `L2` | `120` 分钟 | `challenges/pilot/arceos-axtask-dedicated` |
| `arceos-axfs-dedicated` | ArceOS 文件系统与 shell 专属题 | `arceos` | `L2` | `150` 分钟 | `challenges/pilot/arceos-axfs-dedicated` |

## 使用方法

1. 先确认对应题目已经登记在 `component-ledger.yaml`。
2. 在各题目录下补齐真实 bug、hidden tests 和维护者说明。
3. 用官方比赛镜像执行各题 `repro.sh`，确认公开入口可复现。
4. 再把题目接入实际比赛平台。
