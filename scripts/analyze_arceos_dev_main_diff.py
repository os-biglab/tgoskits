#!/usr/bin/env python3
"""Analyze dev-vs-main divergence for arceos-org repositories."""

from __future__ import annotations

import argparse
from collections import defaultdict
import csv
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
REPOS_CSV = REPO_ROOT / "scripts" / "repo" / "repos.csv"
USER_AGENT = "tgoskits-arceos-dev-main-diff/1.0"
TOP_LEVEL_GROUP_DIRS = {
    "api",
    "apps",
    "benches",
    "components",
    "crates",
    "docs",
    "examples",
    "modules",
    "platforms",
    "scripts",
    "src",
    "tests",
    "tools",
}


@dataclass(frozen=True)
class RepoRecord:
    owner: str
    repo: str
    url: str
    category: str
    target_dir: str


@dataclass(frozen=True)
class CompareResult:
    owner: str
    repo: str
    ahead_by: int | None
    behind_by: int | None
    diff_lines: int | None
    major_changes: str
    status: str
    detail: str


def normalize_github_repo(url: str) -> tuple[str, str] | None:
    prefix = "https://github.com/"
    if not url.startswith(prefix):
        return None
    path = url[len(prefix) :].strip().rstrip("/")
    if path.endswith(".git"):
        path = path[:-4]
    parts = path.split("/")
    if len(parts) < 2:
        return None
    owner, repo = parts[0], parts[1]
    return owner, repo


def run(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        check=check,
        text=True,
        capture_output=True,
    )


def load_arceos_org_repos(csv_path: Path) -> list[RepoRecord]:
    seen: set[tuple[str, str]] = set()
    repos: list[RepoRecord] = []
    with csv_path.open(newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            url = (row.get("url") or "").strip()
            normalized = normalize_github_repo(url)
            if normalized is None:
                continue
            owner, repo = normalized
            if owner != "arceos-org":
                continue
            key = (owner, repo)
            if key in seen:
                continue
            seen.add(key)
            repos.append(
                RepoRecord(
                    owner=owner,
                    repo=repo,
                    url=url,
                    category=(row.get("category") or "").strip(),
                    target_dir=(row.get("target_dir") or "").strip(),
                )
            )
    return sorted(repos, key=lambda item: item.repo)


def summarize_path(path: str) -> str:
    normalized = path.strip()
    if not normalized:
        return "(root)"
    parts = normalized.split("/")
    if parts[0] in {"src", "tests", "examples", "benches", "docs"}:
        return parts[0]
    if len(parts) >= 2 and parts[0] in TOP_LEVEL_GROUP_DIRS:
        return "/".join(parts[:2])
    return parts[0]


def collect_top_changed_paths(numstat_output: str, *, limit: int = 3) -> list[str]:
    area_weights: dict[str, int] = defaultdict(int)
    binary_files = 0
    for line in numstat_output.splitlines():
        parts = line.split("\t", 2)
        if len(parts) < 3:
            continue
        added, deleted, path = parts
        if added == "-" or deleted == "-":
            binary_files += 1
            continue
        try:
            weight = int(added) + int(deleted)
        except ValueError:
            continue
        area_weights[summarize_path(path)] += weight

    ranked = sorted(area_weights.items(), key=lambda item: (-item[1], item[0]))
    top_areas = [f"{name}({weight} lines)" for name, weight in ranked[:limit]]
    if binary_files:
        top_areas.append(f"binary_files({binary_files})")
    return top_areas


def collect_commit_subjects(tmp_path: Path, revspec: str, *, limit: int = 2) -> list[str]:
    proc = run(
        ["git", "log", "--format=%s", "--no-merges", revspec],
        cwd=tmp_path,
    )
    subjects: list[str] = []
    seen: set[str] = set()
    for line in proc.stdout.splitlines():
        subject = " ".join(line.split())
        if not subject or subject in seen:
            continue
        seen.add(subject)
        subjects.append(subject)
        if len(subjects) >= limit:
            break
    return subjects


def shorten_subject(subject: str, *, max_len: int = 72) -> str:
    if len(subject) <= max_len:
        return subject
    return subject[: max_len - 3].rstrip() + "..."


def summarize_major_changes(
    tmp_path: Path,
    *,
    ahead_by: int,
    snapshot_numstat: str,
) -> str:
    pieces: list[str] = []

    top_paths = collect_top_changed_paths(snapshot_numstat)
    if top_paths:
        pieces.append("dev_paths: " + ", ".join(top_paths))

    merge_base_proc = run(
        ["git", "merge-base", "refs/remotes/origin/main", "refs/remotes/origin/dev"],
        cwd=tmp_path,
        check=False,
    )
    merge_base = merge_base_proc.stdout.strip()

    if ahead_by > 0:
        dev_revspec = (
            f"{merge_base}..refs/remotes/origin/dev"
            if merge_base_proc.returncode == 0 and merge_base
            else "refs/remotes/origin/dev"
        )
        dev_subjects = collect_commit_subjects(tmp_path, dev_revspec)
        if dev_subjects:
            pieces.append("dev_commits: " + " | ".join(shorten_subject(item) for item in dev_subjects))

    return "; ".join(pieces)


def compare_branches(owner: str, repo: str) -> CompareResult:
    repo_url = f"https://github.com/{owner}/{repo}.git"
    major_changes = ""
    with tempfile.TemporaryDirectory(prefix=f"{repo}-") as tmpdir:
        tmp_path = Path(tmpdir)
        try:
            run(["git", "init"], cwd=tmp_path)
            run(["git", "remote", "add", "origin", repo_url], cwd=tmp_path)
            run(
                [
                    "git",
                    "fetch",
                    "--filter=blob:none",
                    "--no-tags",
                    "origin",
                    "refs/heads/main:refs/remotes/origin/main",
                    "refs/heads/dev:refs/remotes/origin/dev",
                ],
                cwd=tmp_path,
            )
            proc = run(
                [
                    "git",
                    "rev-list",
                    "--left-right",
                    "--count",
                    "refs/remotes/origin/main...refs/remotes/origin/dev",
                ],
                cwd=tmp_path,
            )
            numstat_proc = run(
                [
                    "git",
                    "diff",
                    "--numstat",
                    "refs/remotes/origin/main..refs/remotes/origin/dev",
                ],
                cwd=tmp_path,
            )
        except subprocess.CalledProcessError as exc:
            detail = (exc.stderr or exc.stdout or "").strip()
            if "couldn't find remote ref" in detail or "fatal: couldn't find remote ref" in detail:
                return CompareResult(owner, repo, None, None, None, "", "MISSING", detail)
            return CompareResult(owner, repo, None, None, None, "", "ERROR", detail or f"git exit {exc.returncode}")

        parts = proc.stdout.strip().split()
        if len(parts) != 2:
            return CompareResult(
                owner,
                repo,
                None,
                None,
                None,
                "",
                "ERROR",
                f"unexpected rev-list output: {proc.stdout.strip()}",
            )
        try:
            behind_by = int(parts[0])
            ahead_by = int(parts[1])
        except ValueError:
            return CompareResult(
                owner,
                repo,
                None,
                None,
                None,
                "",
                "ERROR",
                f"unexpected rev-list output: {proc.stdout.strip()}",
            )

        diff_lines = 0
        for line in numstat_proc.stdout.splitlines():
            parts = line.split("\t", 2)
            if len(parts) < 2:
                continue
            added, deleted = parts[0], parts[1]
            if added == "-" or deleted == "-":
                continue
            try:
                diff_lines += int(added) + int(deleted)
            except ValueError:
                return CompareResult(
                    owner,
                    repo,
                    ahead_by,
                    behind_by,
                    None,
                    "",
                    "ERROR",
                    f"unexpected numstat output: {line}",
                )

        major_changes = summarize_major_changes(
            tmp_path,
            ahead_by=ahead_by,
            snapshot_numstat=numstat_proc.stdout,
        )

    if ahead_by == 0 and behind_by == 0:
        status = "IDENTICAL"
    elif ahead_by > 0 and behind_by == 0:
        status = "AHEAD"
    elif ahead_by == 0 and behind_by > 0:
        status = "BEHIND"
    else:
        status = "DIVERGED"
    return CompareResult(owner, repo, ahead_by, behind_by, diff_lines, major_changes, status, "")


def print_table(results: list[CompareResult]) -> None:
    print(
        f"{'repo':32} {'ahead(dev-main)':>15} {'behind(dev-main)':>16} "
        f"{'diff_lines':>12} {'status':10} major_changes"
    )
    print("-" * 160)
    for item in results:
        ahead = "-" if item.ahead_by is None else str(item.ahead_by)
        behind = "-" if item.behind_by is None else str(item.behind_by)
        diff_lines = "-" if item.diff_lines is None else str(item.diff_lines)
        status = item.status
        if item.detail:
            status = f"{status}: {item.detail}"
        print(f"{item.repo:32} {ahead:>15} {behind:>16} {diff_lines:>12} {status:10} {item.major_changes}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Analyze dev-vs-main divergence for arceos-org repositories listed in scripts/repo/repos.csv."
    )
    parser.add_argument(
        "--csv",
        default=str(REPOS_CSV),
        help=f"Path to repos.csv. Default: {REPOS_CSV}",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print JSON instead of a text table.",
    )
    args = parser.parse_args()

    csv_path = Path(args.csv).resolve()
    repos = load_arceos_org_repos(csv_path)
    if not repos:
        print(f"no arceos-org repositories found in {csv_path}", file=sys.stderr)
        return 1

    results: list[CompareResult] = []
    for repo in repos:
        results.append(compare_branches(repo.owner, repo.repo))

    if args.json:
        print(
            json.dumps(
                [
                    {
                        "owner": item.owner,
                        "repo": item.repo,
                        "ahead_by": item.ahead_by,
                        "behind_by": item.behind_by,
                        "diff_lines": item.diff_lines,
                        "major_changes": item.major_changes,
                        "status": item.status,
                        "detail": item.detail,
                    }
                    for item in results
                ],
                ensure_ascii=False,
                indent=2,
            )
        )
    else:
        print_table(results)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
