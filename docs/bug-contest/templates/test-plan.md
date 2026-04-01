# `<challenge-id>` 测试计划

## 1. visible tests

| 用例 ID | 入口命令 | 目标行为 | 失败信号 |
| --- | --- | --- | --- |
| `visible-01` | `<visible-command>` | 基础功能路径可复现问题 | `<failure-signal>` |
| `visible-02` | `<visible-command>` | 关键边界条件或错误路径可观察 | `<failure-signal>` |

说明：

- visible tests 只暴露定位方向，不暴露完整答案空间。
- 命令必须能在官方镜像里稳定执行。

## 2. hidden tests

| 维度 | 覆盖目标 | 输入来源 | 平台侧资源 |
| --- | --- | --- | --- |
| `hidden-boundary` | 额外边界条件 | `<input-family>` | `none` |
| `hidden-regression` | 防止投机补丁 | `<input-family>` | `none` |
| `hidden-integration` | 跨模块联动 | `<input-family>` | `<asset-path-if-needed>` |

说明：

- hidden tests 的真实输入和脚本不进入公开仓库。
- 如果 hidden tests 需要额外 rootfs、镜像或 guest 配置，必须同步登记到 `challenge-manifest.yaml` 的 `offline_assets`。

## 3. 质量门

- `cargo fmt --all -- --check`
- `cargo xtask clippy`

## 4. 人工复核触发条件

- visible tests 通过但 hidden tests 失败
- patch 过大或修改越界
- 输出行为正确但根因分析明显不成立
