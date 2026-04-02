#!/usr/bin/env python3
"""Check docs/starryos-syscall-compat-matrix.yaml against probe artifacts."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import yaml


def resolve_expected(expected_dir: Path, probe: str) -> tuple[Path | None, Path | None]:
    """Prefer expected/user/* then expected/* (root)."""
    line_candidates = [
        expected_dir / "user" / f"{probe}.line",
        expected_dir / f"{probe}.line",
    ]
    line_file = next((p for p in line_candidates if p.is_file()), None)
    cases_candidates = [
        expected_dir / "user" / f"{probe}.cases",
        expected_dir / f"{probe}.cases",
    ]
    cases_file = next((p for p in cases_candidates if p.is_file()), None)
    return line_file, cases_file


def main() -> int:
    ap = argparse.ArgumentParser(
        description="partial|aligned: contract_probe requires contract/*.c and expected .line or .cases; "
        "divergent: requires tracking_issue http(s) URL; if contract_probe set, same artifact rules."
    )
    ap.add_argument(
        "--matrix",
        type=Path,
        default=Path("docs/starryos-syscall-compat-matrix.yaml"),
    )
    ap.add_argument("--root", type=Path, default=None)
    args = ap.parse_args()
    root = args.root
    if root is None:
        root = args.matrix.resolve().parent.parent

    data = yaml.safe_load(args.matrix.read_text(encoding="utf-8"))
    entries = data.get("entries") or []
    contract_dir = root / "test-suit" / "starryos" / "probes" / "contract"
    expected_dir = root / "test-suit" / "starryos" / "probes" / "expected"
    errors: list[str] = []

    for e in entries:
        if not isinstance(e, dict):
            continue
        parity = str(e.get("parity") or "")
        probe = str(e.get("contract_probe") or "").strip()
        syscall = e.get("syscall", "?")

        if parity == "divergent":
            ti = str(e.get("tracking_issue") or "").strip()
            if not ti.startswith(("http://", "https://")):
                errors.append(
                    f"{syscall}: parity divergent requires tracking_issue "
                    f"(http(s) URL), see docs/starryos-syscall-compat-divergence.md"
                )
            if probe:
                c_file = contract_dir / f"{probe}.c"
                line_file, cases_file = resolve_expected(expected_dir, probe)
                if not c_file.is_file():
                    errors.append(f"{syscall}: missing contract {c_file.relative_to(root)}")
                if line_file is None and cases_file is None:
                    errors.append(
                        f"{syscall}: expected expected/user/{probe}.line|.cases or "
                        f"expected/{probe}.line|.cases for probe {probe}"
                    )
            continue

        if parity not in ("partial", "aligned"):
            continue
        if not probe:
            continue
        c_file = contract_dir / f"{probe}.c"
        line_file, cases_file = resolve_expected(expected_dir, probe)
        if not c_file.is_file():
            errors.append(f"{syscall}: missing contract {c_file.relative_to(root)}")
        if line_file is None and cases_file is None:
            errors.append(
                f"{syscall}: expected expected/user/{probe}.line|.cases or "
                f"expected/{probe}.line|.cases for probe {probe}"
            )

    if errors:
        print("Compat matrix probe check failed:", file=sys.stderr)
        for msg in errors:
            print(f"  {msg}", file=sys.stderr)
        return 1
    print("Compat matrix OK: partial/aligned rows have contract + expected; divergent rows have tracking_issue.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
