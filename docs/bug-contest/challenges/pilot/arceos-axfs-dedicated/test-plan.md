# `arceos-axfs-dedicated` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo arceos test qemu --target riscv64gc-unknown-none-elf` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-fs-edge` | 补路径解析、目录操作与文件系统边界条件 | 平台侧文件系统输入集 |
| `hidden-block-path` | 验证块设备链路与 shell 命令协同 | 平台侧块设备测试集 |
| `hidden-regression` | 验证相邻运行时与文件系统路径未回归 | 平台侧回归集 |

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo xtask clippy`

## 4. 题型说明

- 本题判题 profile：`arceos-riscv64`
- 预计用时：`150` 分钟
- 校内试运行目标：验证块设备与文件系统链路题在官方镜像中的稳定性。
