#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import shlex
import subprocess
import sys
from dataclasses import dataclass, field
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
class Stage:
    name: str
    agent: str
    description: str = ""
    command: list[str] = field(default_factory=list)
    working_dir: str = "."


@dataclass
class Task:
    task_id: str
    title: str
    target: str = ""
    description: str = ""
    allowed_paths: list[str] = field(default_factory=list)
    output_dir: str = ".copilot/runs"
    stages: list[Stage] = field(default_factory=list)


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


def read_prompt_template(root: Path, agent: str) -> str:
    template = root / ".copilot" / "framework" / "prompts" / f"{agent}.md"
    if template.exists():
        return template.read_text(encoding="utf-8").rstrip()
    return f"# {agent}\n\nNo template found for this role.\n"


def render_prompt(root: Path, task: Task, stage: Stage, run_dir: Path) -> str:
    template = read_prompt_template(root, stage.agent)
    allowed_paths = "\n".join(f"- {item}" for item in task.allowed_paths) or "- (none)"
    command = " ".join(stage.command) if stage.command else "(no command)"
    lines = [
        f"# Run: {task.task_id}",
        f"- Title: {task.title}",
        f"- Target: {task.target or '(unspecified)'}",
        f"- Stage: {stage.name}",
        f"- Agent: {stage.agent}",
        f"- Working directory: {stage.working_dir}",
        f"- Command: {command}",
        "",
        "## Allowed paths",
        allowed_paths,
        "",
        "## Stage notes",
        stage.description or "(none)",
        "",
        "## Template",
        template,
        "",
        f"Run directory: {run_dir}",
    ]
    return "\n".join(lines)


def plan_run(root: Path, task: Task, manifest_path: Path) -> Path:
    run_dir = make_run_dir(root, task, task.output_dir)
    (run_dir / "stages").mkdir()
    (run_dir / "manifest.raw.toml").write_text(manifest_path.read_text(encoding="utf-8"), encoding="utf-8")
    normalized = {
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
                "description": stage.description,
                "command": stage.command,
                "working_dir": stage.working_dir,
            }
            for stage in task.stages
        ],
    }
    (run_dir / "manifest.normalized.json").write_text(json.dumps(normalized, indent=2) + "\n", encoding="utf-8")
    for index, stage in enumerate(task.stages, start=1):
        prompt = render_prompt(root, task, stage, run_dir)
        stage_name = safe_path(stage.name)
        stage_file = run_dir / "stages" / f"{index:02d}-{stage_name}.prompt.md"
        stage_file.write_text(prompt, encoding="utf-8")
        status = {
            "index": index,
            "name": stage.name,
            "agent": stage.agent,
            "status": "prepared",
            "command": stage.command,
            "working_dir": stage.working_dir,
        }
        (run_dir / "stages" / f"{index:02d}-{stage_name}.status.json").write_text(json.dumps(status, indent=2) + "\n", encoding="utf-8")
    return run_dir


def execute_run(root: Path, task: Task, manifest_path: Path) -> Path:
    run_dir = plan_run(root, task, manifest_path)
    for index, stage in enumerate(task.stages, start=1):
        if not stage.command:
            continue
        workdir = (root / stage.working_dir).resolve()
        stage_name = safe_path(stage.name)
        log_path = run_dir / "stages" / f"{index:02d}-{stage_name}.log"
        status_path = run_dir / "stages" / f"{index:02d}-{stage_name}.status.json"
        status = {
            "index": index,
            "name": stage.name,
            "agent": stage.agent,
            "status": "running",
            "command": stage.command,
            "working_dir": stage.working_dir,
        }
        status_path.write_text(json.dumps(status, indent=2) + "\n", encoding="utf-8")
        with log_path.open("w", encoding="utf-8") as log_file:
            proc = subprocess.run(
                stage.command,
                cwd=workdir,
                stdout=log_file,
                stderr=subprocess.STDOUT,
                text=True,
            )
        status["status"] = "passed" if proc.returncode == 0 else "failed"
        status["returncode"] = proc.returncode
        status_path.write_text(json.dumps(status, indent=2) + "\n", encoding="utf-8")
        if proc.returncode != 0:
            raise subprocess.CalledProcessError(proc.returncode, stage.command)
    return run_dir


def print_summary(task: Task, run_dir: Path | None) -> None:
    print(f"task: {task.task_id}")
    print(f"title: {task.title}")
    print(f"target: {task.target or '(unspecified)'}")
    print(f"run_dir: {run_dir if run_dir is not None else '(not created)'}")
    print("stages:")
    for stage in task.stages:
        command = " ".join(stage.command) if stage.command else "(prompt only)"
        print(f"  - {stage.name} [{stage.agent}] {command}")


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
    except (OSError, ManifestError, tomllib.TOMLDecodeError) as exc:
        print(f"manifest error: {exc}", file=sys.stderr)
        return 2

    if args.command == "validate":
        print_summary(task, None)
        return 0

    try:
        if args.command == "plan":
            run_dir = plan_run(root, task, manifest_path)
        else:
            run_dir = execute_run(root, task, manifest_path)
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"framework error: {exc}", file=sys.stderr)
        return 1

    print_summary(task, run_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
