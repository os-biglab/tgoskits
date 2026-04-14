#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import shlex
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python < 3.11
    import tomli as tomllib  # type: ignore[no-redef]


ROLE_ORDER = (
    "orchestrator",
    "planner",
    "implementer",
    "reviewer",
    "test-debugger",
    "doc-writer",
)


class ManifestError(ValueError):
    pass


@dataclass
class StagePolicy:
    invoke_copilot: bool = True
    autopilot: bool = False
    max_attempts: int = 1
    retry_on_failure: bool = False
    command_timeout_sec: int = 0
    max_autopilot_continues: int = 0


@dataclass
class Stage:
    name: str
    agent: str
    model: str = ""
    description: str = ""
    command: list[str] = field(default_factory=list)
    working_dir: str = "."
    policy: StagePolicy = field(default_factory=StagePolicy)


@dataclass
class Task:
    task_id: str
    title: str
    target: str = ""
    description: str = ""
    allowed_paths: list[str] = field(default_factory=list)
    output_dir: str = ".copilot/runs"
    stages: list[Stage] = field(default_factory=list)


@dataclass
class FrameworkConfig:
    default_model: str = "gpt-5-mini"
    stage_models: dict[str, str] = field(default_factory=dict)
    copilot_cmd: str = "copilot"
    copilot_args: list[str] = field(
        default_factory=lambda: [
            "--allow-all-tools",
            "--allow-all-paths",
            "--allow-all-urls",
            "--no-ask-user",
            "--silent",
        ]
    )
    default_policy: StagePolicy = field(default_factory=StagePolicy)
    stage_policies: dict[str, StagePolicy] = field(default_factory=dict)


def find_repo_root(start: Path) -> Path:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=start,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return start
    return Path(result.stdout.strip())


def load_manifest(path: Path) -> Task:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    task_data = data.get("task")
    if not isinstance(task_data, dict):
        raise ManifestError("manifest must contain a [task] table")

    task_id = _require_str(task_data, "id")
    title = _require_str(task_data, "title")
    target = task_data.get("target", "")
    description = task_data.get("description", "")
    allowed_paths = _require_str_list(task_data.get("allowed_paths", []), "allowed_paths")
    output_dir = task_data.get("output_dir", ".copilot/runs")
    stages_data = data.get("stages", [])
    if not isinstance(stages_data, list) or not stages_data:
        raise ManifestError("manifest must contain at least one [[stages]] entry")

    stages = [load_stage(item, index) for index, item in enumerate(stages_data, start=1)]
    return Task(
        task_id=task_id,
        title=title,
        target=target,
        description=description,
        allowed_paths=allowed_paths,
        output_dir=output_dir,
        stages=stages,
    )


def _as_bool(value: Any, key: str) -> bool:
    if not isinstance(value, bool):
        raise ManifestError(f"{key} must be a bool")
    return value


def _as_int(value: Any, key: str) -> int:
    if not isinstance(value, int):
        raise ManifestError(f"{key} must be an int")
    return value


def _as_str(value: Any, key: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ManifestError(f"{key} must be a non-empty string")
    return value.strip()


def _as_str_list(value: Any, key: str) -> list[str]:
    if not isinstance(value, list):
        raise ManifestError(f"{key} must be a list of strings")
    result: list[str] = []
    for idx, item in enumerate(value, start=1):
        if not isinstance(item, str) or not item.strip():
            raise ManifestError(f"{key}[{idx}] must be a non-empty string")
        result.append(item.strip())
    return result


def parse_stage_policy(raw: dict[str, Any], key_prefix: str) -> StagePolicy:
    policy = StagePolicy()
    if "invoke_copilot" in raw:
        policy.invoke_copilot = _as_bool(raw["invoke_copilot"], f"{key_prefix}.invoke_copilot")
    if "autopilot" in raw:
        policy.autopilot = _as_bool(raw["autopilot"], f"{key_prefix}.autopilot")
    if "max_attempts" in raw:
        policy.max_attempts = _as_int(raw["max_attempts"], f"{key_prefix}.max_attempts")
    if "retry_on_failure" in raw:
        policy.retry_on_failure = _as_bool(raw["retry_on_failure"], f"{key_prefix}.retry_on_failure")
    if "command_timeout_sec" in raw:
        policy.command_timeout_sec = _as_int(raw["command_timeout_sec"], f"{key_prefix}.command_timeout_sec")
    if "max_autopilot_continues" in raw:
        policy.max_autopilot_continues = _as_int(
            raw["max_autopilot_continues"], f"{key_prefix}.max_autopilot_continues"
        )
    if policy.max_attempts < 1:
        raise ManifestError(f"{key_prefix}.max_attempts must be >= 1")
    if policy.command_timeout_sec < 0:
        raise ManifestError(f"{key_prefix}.command_timeout_sec must be >= 0")
    if policy.max_autopilot_continues < 0:
        raise ManifestError(f"{key_prefix}.max_autopilot_continues must be >= 0")
    return policy


def load_framework_config(root: Path) -> FrameworkConfig:
    config = FrameworkConfig()
    config_path = root / ".copilot" / "framework" / "config.toml"
    if not config_path.exists():
        return config

    data = tomllib.loads(config_path.read_text(encoding="utf-8"))
    if "model" in data:
        config.default_model = _as_str(data["model"], "model")
    if "default_model" in data:
        config.default_model = _as_str(data["default_model"], "default_model")
    if "copilot_cmd" in data:
        config.copilot_cmd = _as_str(data["copilot_cmd"], "copilot_cmd")
    if "copilot_args" in data:
        config.copilot_args = _as_str_list(data["copilot_args"], "copilot_args")

    default_policy_raw = data.get("default_stage_policy", {})
    if default_policy_raw:
        if not isinstance(default_policy_raw, dict):
            raise ManifestError("default_stage_policy must be a table")
        config.default_policy = parse_stage_policy(default_policy_raw, "default_stage_policy")

    stage_models_raw = data.get("stage_models", {})
    if not isinstance(stage_models_raw, dict):
        raise ManifestError("stage_models must be a table")
    for key, value in stage_models_raw.items():
        config.stage_models[str(key)] = _as_str(value, f"stage_models.{key}")

    stage_policies_raw = data.get("stage_policies", {})
    if not isinstance(stage_policies_raw, dict):
        raise ManifestError("stage_policies must be a table of tables")
    for key, raw in stage_policies_raw.items():
        if not isinstance(raw, dict):
            raise ManifestError(f"stage_policies.{key} must be a table")
        config.stage_policies[str(key)] = parse_stage_policy(raw, f"stage_policies.{key}")

    return config


def load_stage(raw: Any, index: int) -> Stage:
    if not isinstance(raw, dict):
        raise ManifestError(f"stage {index} must be a table")
    name = _require_str(raw, "name")
    agent = _require_str(raw, "agent")
    if agent not in ROLE_ORDER:
        raise ManifestError(f"stage {index} uses unknown agent {agent!r}")
    command = raw.get("command", [])
    if isinstance(command, str):
        command_list = shlex.split(command)
    elif isinstance(command, list):
        command_list = [str(item) for item in command]
    elif command is None:
        command_list = []
    else:
        raise ManifestError(f"stage {index} command must be a string or list")
    working_dir = str(raw.get("working_dir", "."))
    description = str(raw.get("description", ""))
    return Stage(name=name, agent=agent, description=description, command=command_list, working_dir=working_dir)


def _require_str(data: dict[str, Any], key: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ManifestError(f"missing or empty string field: {key}")
    return value.strip()


def _require_str_list(value: Any, key: str) -> list[str]:
    if not isinstance(value, list):
        raise ManifestError(f"{key} must be a list of strings")
    result: list[str] = []
    for item in value:
        if not isinstance(item, str) or not item.strip():
            raise ManifestError(f"{key} must contain only non-empty strings")
        result.append(item.strip())
    return result


def repo_root_from_args(args: argparse.Namespace) -> Path:
    if args.repo_root:
        return Path(args.repo_root).resolve()
    return find_repo_root(Path.cwd()).resolve()


def make_run_dir(root: Path, task: Task, output_dir: str) -> Path:
    timestamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    base = (root / output_dir).resolve()
    run_dir = base / safe_path(task.task_id) / timestamp
    run_dir.mkdir(parents=True, exist_ok=False)
    return run_dir


def safe_path(value: str) -> str:
    cleaned = re.sub(r"[^A-Za-z0-9._-]+", "-", value.strip())
    return cleaned.strip("-") or "task"


def resolve_stage_model(config: FrameworkConfig, stage: Stage) -> str:
    return (
        config.stage_models.get(stage.name)
        or config.stage_models.get(stage.agent)
        or config.default_model
    )


def resolve_stage_policy(config: FrameworkConfig, stage: Stage) -> StagePolicy:
    if stage.name in config.stage_policies:
        return config.stage_policies[stage.name]
    if stage.agent in config.stage_policies:
        return config.stage_policies[stage.agent]
    return config.default_policy


def read_prompt_template(root: Path, agent: str) -> str:
    template = root / ".copilot" / "framework" / "prompts" / f"{agent}.md"
    if template.exists():
        return template.read_text(encoding="utf-8").rstrip()
    return f"# {agent}\n\nNo template found for this role.\n"


def render_prompt(
    root: Path,
    task: Task,
    stage: Stage,
    run_dir: Path,
    attempt: int,
    max_attempts: int,
    previous_failure: str,
) -> str:
    template = read_prompt_template(root, stage.agent)
    allowed_paths = "\n".join(f"- {item}" for item in task.allowed_paths) or "- (none)"
    command = " ".join(stage.command) if stage.command else "(no command)"
    lines = [
        f"# Run: {task.task_id}",
        f"- Title: {task.title}",
        f"- Target: {task.target or '(unspecified)'}",
        f"- Stage: {stage.name}",
        f"- Agent: {stage.agent}",
        f"- Model: {stage.model or '(unset)'}",
        f"- Attempt: {attempt}/{max_attempts}",
        f"- Working directory: {stage.working_dir}",
        f"- Command: {command}",
        "",
        "## Stage policy",
        f"- invoke_copilot: {str(stage.policy.invoke_copilot).lower()}",
        f"- autopilot: {str(stage.policy.autopilot).lower()}",
        f"- max_attempts: {stage.policy.max_attempts}",
        f"- retry_on_failure: {str(stage.policy.retry_on_failure).lower()}",
        "",
        "## Allowed paths",
        allowed_paths,
        "",
        "## Stage notes",
        stage.description or "(none)",
        "",
        "## Previous attempt failure",
        previous_failure or "(none)",
        "",
        "## Template",
        template,
        "",
        f"Run directory: {run_dir}",
    ]
    return "\n".join(lines)


def write_json(path: Path, obj: dict[str, Any]) -> None:
    path.write_text(json.dumps(obj, indent=2) + "\n", encoding="utf-8")


def run_command_with_log(
    command: list[str],
    cwd: Path,
    log_path: Path,
    timeout_sec: int,
) -> tuple[int, str]:
    timeout = timeout_sec if timeout_sec > 0 else None
    try:
        proc = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        output = (proc.stdout or "") + (proc.stderr or "")
        log_path.write_text(output, encoding="utf-8")
        return proc.returncode, output
    except subprocess.TimeoutExpired as exc:
        out = (exc.stdout or "") + (exc.stderr or "")
        out += f"\n[timeout] command exceeded {timeout_sec} seconds\n"
        log_path.write_text(out, encoding="utf-8")
        return 124, out


def build_copilot_base_command(config: FrameworkConfig, stage: Stage) -> list[str]:
    cmd = [config.copilot_cmd, *config.copilot_args]
    if stage.model:
        cmd.extend(["--model", stage.model])
    if stage.policy.autopilot:
        cmd.append("--autopilot")
        if stage.policy.max_autopilot_continues > 0:
            cmd.extend(["--max-autopilot-continues", str(stage.policy.max_autopilot_continues)])
    return cmd


def run_copilot_prompt(
    config: FrameworkConfig,
    stage: Stage,
    prompt: str,
    cwd: Path,
    log_path: Path,
    timeout_sec: int,
) -> tuple[int, str, list[str]]:
    timeout = timeout_sec if timeout_sec > 0 else None
    base_cmd = build_copilot_base_command(config, stage)
    prompt_cmd = [*base_cmd, "-p", prompt]

    try:
        first = subprocess.run(prompt_cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
        first_output = (first.stdout or "") + (first.stderr or "")
    except subprocess.TimeoutExpired as exc:
        first_output = (exc.stdout or "") + (exc.stderr or "")
        first_output += f"\n[timeout] copilot prompt exceeded {timeout_sec} seconds\n"
        log_path.write_text(first_output, encoding="utf-8")
        return 124, first_output, prompt_cmd

    if first.returncode == 0:
        log_path.write_text(first_output, encoding="utf-8")
        return 0, first_output, prompt_cmd

    try:
        second = subprocess.run(base_cmd, cwd=cwd, input=prompt, capture_output=True, text=True, timeout=timeout)
        second_output = (second.stdout or "") + (second.stderr or "")
    except subprocess.TimeoutExpired as exc:
        second_output = (exc.stdout or "") + (exc.stderr or "")
        second_output += f"\n[timeout] copilot stdin fallback exceeded {timeout_sec} seconds\n"
        merged = (
            "[prompt-mode failed]\n"
            + first_output
            + "\n\n[stdin-fallback failed]\n"
            + second_output
        )
        log_path.write_text(merged, encoding="utf-8")
        return 124, merged, base_cmd

    merged = (
        "[prompt-mode failed]\n"
        + first_output
        + "\n\n[stdin-fallback output]\n"
        + second_output
    )
    log_path.write_text(merged, encoding="utf-8")
    if second.returncode == 0:
        return 0, merged, base_cmd
    return second.returncode, merged, base_cmd


def init_stage_status(index: int, stage: Stage) -> dict[str, Any]:
    return {
        "index": index,
        "name": stage.name,
        "agent": stage.agent,
        "model": stage.model,
        "status": "prepared",
        "policy": asdict(stage.policy),
        "command": stage.command,
        "working_dir": stage.working_dir,
        "attempts": [],
    }


def plan_run(root: Path, task: Task, manifest_path: Path, config: FrameworkConfig) -> Path:
    run_dir = make_run_dir(root, task, task.output_dir)
    (run_dir / "stages").mkdir()
    (run_dir / "manifest.raw.toml").write_text(manifest_path.read_text(encoding="utf-8"), encoding="utf-8")
    framework_config_path = root / ".copilot" / "framework" / "config.toml"
    if framework_config_path.exists():
        (run_dir / "framework.config.toml").write_text(framework_config_path.read_text(encoding="utf-8"), encoding="utf-8")

    normalized = {
        "framework": {
            "default_model": config.default_model,
            "stage_models": config.stage_models,
            "copilot_cmd": config.copilot_cmd,
            "copilot_args": config.copilot_args,
            "default_stage_policy": asdict(config.default_policy),
            "stage_policies": {name: asdict(policy) for name, policy in config.stage_policies.items()},
        },
        "task": {
            "id": task.task_id,
            "title": task.title,
            "target": task.target,
            "description": task.description,
            "allowed_paths": task.allowed_paths,
            "output_dir": task.output_dir,
        },
        "stages": [
            {
                "name": stage.name,
                "agent": stage.agent,
                "model": stage.model,
                "policy": asdict(stage.policy),
                "description": stage.description,
                "command": stage.command,
                "working_dir": stage.working_dir,
            }
            for stage in task.stages
        ],
    }
    write_json(run_dir / "manifest.normalized.json", normalized)

    for index, stage in enumerate(task.stages, start=1):
        stage_name = safe_path(stage.name)
        preview_prompt = render_prompt(
            root=root,
            task=task,
            stage=stage,
            run_dir=run_dir,
            attempt=1,
            max_attempts=stage.policy.max_attempts,
            previous_failure="",
        )
        (run_dir / "stages" / f"{index:02d}-{stage_name}.prompt.md").write_text(preview_prompt, encoding="utf-8")
        write_json(run_dir / "stages" / f"{index:02d}-{stage_name}.status.json", init_stage_status(index, stage))
    return run_dir


def run_stage(root: Path, run_dir: Path, task: Task, config: FrameworkConfig, index: int, stage: Stage) -> tuple[bool, dict[str, Any]]:
    stage_name = safe_path(stage.name)
    status_path = run_dir / "stages" / f"{index:02d}-{stage_name}.status.json"
    status = init_stage_status(index, stage)
    status["status"] = "running"
    write_json(status_path, status)

    workdir = (root / stage.working_dir).resolve()
    if not workdir.exists():
        status["status"] = "failed"
        status["failure_reason"] = f"working directory does not exist: {workdir}"
        write_json(status_path, status)
        return False, status

    previous_failure = ""
    attempts: list[dict[str, Any]] = []
    for attempt in range(1, stage.policy.max_attempts + 1):
        attempt_record: dict[str, Any] = {
            "attempt": attempt,
            "prompt_file": "",
        }

        prompt = render_prompt(
            root=root,
            task=task,
            stage=stage,
            run_dir=run_dir,
            attempt=attempt,
            max_attempts=stage.policy.max_attempts,
            previous_failure=previous_failure,
        )
        prompt_path = run_dir / "stages" / f"{index:02d}-{stage_name}.attempt-{attempt:02d}.prompt.md"
        prompt_path.write_text(prompt, encoding="utf-8")
        attempt_record["prompt_file"] = str(prompt_path)

        if stage.policy.invoke_copilot:
            copilot_log_path = run_dir / "stages" / f"{index:02d}-{stage_name}.attempt-{attempt:02d}.copilot.log"
            copilot_rc, _, copilot_cmd = run_copilot_prompt(
                config=config,
                stage=stage,
                prompt=prompt,
                cwd=workdir,
                log_path=copilot_log_path,
                timeout_sec=stage.policy.command_timeout_sec,
            )
            attempt_record["copilot"] = {
                "returncode": copilot_rc,
                "command": copilot_cmd,
                "log_file": str(copilot_log_path),
            }
            if copilot_rc != 0:
                previous_failure = f"copilot failed with return code {copilot_rc}; see {copilot_log_path.name}"
                attempts.append(attempt_record)
                status["attempts"] = attempts
                write_json(status_path, status)
                if not stage.policy.retry_on_failure:
                    status["status"] = "failed"
                    status["failure_reason"] = previous_failure
                    write_json(status_path, status)
                    return False, status
                continue

        if stage.command:
            command_log_path = run_dir / "stages" / f"{index:02d}-{stage_name}.attempt-{attempt:02d}.command.log"
            command_rc, _ = run_command_with_log(
                command=stage.command,
                cwd=workdir,
                log_path=command_log_path,
                timeout_sec=stage.policy.command_timeout_sec,
            )
            attempt_record["command"] = {
                "returncode": command_rc,
                "command": stage.command,
                "log_file": str(command_log_path),
            }
            attempts.append(attempt_record)
            status["attempts"] = attempts
            write_json(status_path, status)
            if command_rc == 0:
                status["status"] = "passed"
                write_json(status_path, status)
                return True, status

            previous_failure = f"command failed with return code {command_rc}; see {command_log_path.name}"
            if not stage.policy.retry_on_failure:
                status["status"] = "failed"
                status["failure_reason"] = previous_failure
                write_json(status_path, status)
                return False, status
            continue

        attempts.append(attempt_record)
        status["attempts"] = attempts
        status["status"] = "passed"
        write_json(status_path, status)
        return True, status

    status["status"] = "failed"
    status["failure_reason"] = previous_failure or "stage exhausted all attempts"
    status["attempts"] = attempts
    write_json(status_path, status)
    return False, status


def execute_run(root: Path, task: Task, manifest_path: Path, config: FrameworkConfig) -> Path:
    run_dir = plan_run(root, task, manifest_path, config)
    for index, stage in enumerate(task.stages, start=1):
        ok, stage_status = run_stage(root=root, run_dir=run_dir, task=task, config=config, index=index, stage=stage)
        if not ok:
            raise subprocess.CalledProcessError(
                returncode=1,
                cmd=f"stage {stage.name}",
                output=json.dumps(stage_status, ensure_ascii=False),
            )
    return run_dir


def print_summary(task: Task, run_dir: Path | None) -> None:
    print(f"task: {task.task_id}")
    print(f"title: {task.title}")
    print(f"target: {task.target or '(unspecified)'}")
    print(f"run_dir: {run_dir if run_dir is not None else '(not created)'}")
    print("stages:")
    for stage in task.stages:
        command = " ".join(stage.command) if stage.command else "(prompt only)"
        print(
            f"  - {stage.name} [{stage.agent}] {stage.model or '(unset)'} "
            f"copilot={str(stage.policy.invoke_copilot).lower()} "
            f"autopilot={str(stage.policy.autopilot).lower()} "
            f"attempts={stage.policy.max_attempts} {command}"
        )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="TGOSKits AI iteration framework runner")
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name, help_text in (
        ("validate", "validate the manifest"),
        ("plan", "generate prompt bundles"),
        ("run", "generate prompt bundles and execute stage commands"),
    ):
        subparser = subparsers.add_parser(name, help=help_text)
        subparser.add_argument("--repo-root", help="override repository root")
        subparser.add_argument("--manifest", required=True, help="path to the task manifest")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    manifest_path = Path(args.manifest).resolve()
    root = repo_root_from_args(args)
    try:
        task = load_manifest(manifest_path)
        config = load_framework_config(root)
        task.stages = [
            Stage(
                name=stage.name,
                agent=stage.agent,
                model=resolve_stage_model(config, stage),
                description=stage.description,
                command=stage.command,
                working_dir=stage.working_dir,
                policy=resolve_stage_policy(config, stage),
            )
            for stage in task.stages
        ]
    except (OSError, ManifestError, tomllib.TOMLDecodeError) as exc:
        print(f"manifest error: {exc}", file=sys.stderr)
        return 2

    if args.command == "validate":
        print_summary(task, None)
        return 0

    try:
        if args.command == "plan":
            run_dir = plan_run(root, task, manifest_path, config)
        else:
            run_dir = execute_run(root, task, manifest_path, config)
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"framework error: {exc}", file=sys.stderr)
        return 1

    print_summary(task, run_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

