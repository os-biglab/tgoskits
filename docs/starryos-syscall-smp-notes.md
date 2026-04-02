# StarryOS Syscall 与 SMP（S0-6）

## 已落地：`-smp 2` QEMU 模板

- **`test-suit/starryos/qemu-riscv64-smp2.toml`**：在默认 `qemu-riscv64.toml` 基础上增加 **`-smp` `2`**。
- **`cargo xtask starry test qemu --qemu-config test-suit/starryos/qemu-riscv64-smp2.toml`**：使用该模板生成临时测试配置（仍配合 **`--test-disk-image`** 等参数）。
- 便捷脚本：**`test-suit/starryos/scripts/run-starry-probe-qemu-smp2.sh <probe>`**（等价于单核版 **`run-starry-probe-qemu.sh`** + SMP TOML）。

日常命令摘要见 **`docs/starryos-probes-daily.md`**。

## 建议用法

- 对 **errno / 零长度 IO** 等确定性探针，在单核通过后可再跑一遍 SMP 冒烟，确认无回归。
- **`futex` / `ppoll`** 等多核语义或竞态相关项：勿单独依赖固定 `expected/*.line`；需单独设计用例与匹配策略。

## 与矩阵的关系

可在 **`docs/starryos-syscall-compat-matrix.yaml`** 的 `notes` 中标注「单核 + SMP2 冒烟已跑」；同步原语类待专用矩阵后再填 `parity`。
