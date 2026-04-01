# TGOSKits 找 Bug 比赛资产包

本目录用于沉淀 TGOSKits 组件化内核找 Bug 比赛的可执行资产，而不是只保存一份概念方案。
组织者可以把这里的文档、模板和清单直接作为赛季筹备材料使用。

## 目录说明

- `component-coverage-ledger.md`：赛季题库的人工可读台账，说明四层赛道如何选题。
- `component-ledger.yaml`：题库覆盖清单的结构化版本，可继续接脚本或表格系统。
- `judge-environment.md`：比赛镜像、判题命令、资源限制和离线资源准备规范。
- `judge-manifest.yaml`：判题环境的结构化配置基线。
- `challenge-packaging.md`：标准题目包规范与目录约定。
- `challenges/pilot/`：`pilot-batch.yaml` 对应的 8 道校内试运行题包骨架。
- `templates/`：题目说明、复现脚本、提交说明、维护者说明等模板文件。
- `governance-and-scoring.md`：评分、披露、复审、治理和奖项规则。
- `pilot-run.md`：校内试运行方案。
- `pilot-batch.yaml`：首批试运行题单的结构化版本。

## 使用顺序

1. 先看 `component-coverage-ledger.md`，确定本赛季要覆盖哪些组件和系统层次。
2. 再看 `judge-environment.md` 与 `judge-manifest.yaml`，固化比赛镜像和统一判题入口。
3. 参考 `challenge-packaging.md` 和 `templates/`，并从 `challenges/pilot/` 的现成骨架开始产出单题资产。
4. 依据 `governance-and-scoring.md` 完成评审、披露和奖项流程设计。
5. 最后按 `pilot-run.md` 与 `pilot-batch.yaml` 组织一次小规模试运行，验证难度与评测稳定性。
