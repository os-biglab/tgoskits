---
name: ai-iteration-framework
description: Build and operate the AI-driven kernel iteration pipeline for TGOSKits. Use this skill when you want to plan, implement, review, test, debug, or document a kernel/OS change through the framework.
---

# AI Iteration Framework

Use this skill to drive the TGOSKits iteration pipeline around a manifest, prompt bundle, and staged execution flow.

## What this skill covers

- One orchestration entrypoint for kernel, OS, and Linux-app support work
- Structured roles: `orchestrator`, `planner`, `implementer`, `reviewer`, `test-debugger`, `doc-writer`
- Prompt bundle generation and stage-by-stage execution
- `cargo xtask`-first validation and test commands

## Workflow

1. Load a manifest from `.copilot/framework/manifests/*.toml`
2. Generate a run directory under `.copilot/runs/`
3. Render prompt bundles from `.copilot/framework/prompts/`
4. Execute stage commands when the manifest provides them
5. Stop on failure, preserve logs, and hand the failure to `test-debugger`

## Local runner

Use the repository runner:

```bash
python3 scripts/ai_framework.py validate --manifest .copilot/framework/manifests/linux-app-example.toml
python3 scripts/ai_framework.py plan --manifest .copilot/framework/manifests/linux-app-example.toml
python3 scripts/ai_framework.py run --manifest .copilot/framework/manifests/linux-app-example.toml
```

## Role prompts

Each role has a reusable prompt template in:

- `.copilot/framework/prompts/orchestrator.md`
- `.copilot/framework/prompts/planner.md`
- `.copilot/framework/prompts/implementer.md`
- `.copilot/framework/prompts/reviewer.md`
- `.copilot/framework/prompts/test-debugger.md`
- `.copilot/framework/prompts/doc-writer.md`

## Hooks

Hook templates live in `.copilot/framework/hooks/` and are intended to be wired into external automation or local shell wrappers.

