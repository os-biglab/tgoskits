# `host-rsext4-dedicated` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `cargo test -p rsext4` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |
| `visible-02` | `cargo test -p axfs-ng-vfs` | 覆盖题目主功能链路 | 命令失败、断言失败或输出不符合预期 |

## 2. hidden tests

| 类别 | 覆盖点 | 输入来源 |
| --- | --- | --- |
| `hidden-metadata` | 补目录项、extent 与元数据边界条件 | 平台侧文件系统输入集 |
| `hidden-regression` | 验证相邻文件系统路径未被修坏 | 平台侧隐藏回归集 |
| `hidden-vfs-integration` | 验证 ext4 与 VFS 的协同行为 | 平台侧集成测试集 |

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo xtask clippy`

## 4. 题型说明

- 本题判题 profile：`host-standard`
- 预计用时：`120` 分钟
- 校内试运行目标：验证文件系统题的说明成本与 hidden tests 命中率。
