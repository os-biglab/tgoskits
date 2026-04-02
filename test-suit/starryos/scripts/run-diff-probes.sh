#!/usr/bin/sh
set -eu
PKG="$(cd "$(dirname "$0")/.." && pwd)"
WS="$(cd "$PKG/../.." && pwd)"
OUT="${PROBE_OUT:-$PKG/probes/build-riscv64}"
QEMU_RV64="${QEMU_RV64:-qemu-riscv64}"
# user (default): qemu-riscv64 + expected/*.line or expected/user/*.line
# guest-alpine323: qemu-system + expected/guest-alpine323/ (needs STARRY_LINUX_GUEST_IMAGE)
ORACLE_TRACK="${VERIFY_ORACLE_TRACK:-user}"
GUEST_SCRIPT="$WS/scripts/run_linux_guest_oracle.sh"

usage() {
  echo "Usage: $0 {build|oracle|oracle-guest|verify-oracle|verify-oracle-all|help}" >&2
  echo "  build                 run build-probes.sh" >&2
  echo "  oracle [name]         run \$OUT/<name> under \$QEMU_RV64 (user track)" >&2
  echo "  oracle-guest [name]  run probe under qemu-system (guest track; needs STARRY_LINUX_GUEST_IMAGE)" >&2
  echo "  verify-oracle [name]  diff vs expected (track from VERIFY_ORACLE_TRACK)" >&2
  echo "  verify-oracle-all     every expected *.line / *.cases for current track" >&2
  echo "Env: VERIFY_ORACLE_TRACK=user|guest-alpine323 (default user)" >&2
  echo "     VERIFY_STRICT=1    missing qemu / guest kernel => exit 2 where applicable" >&2
  exit 1
}

resolve_line() {
  p="$1"
  if [ "$ORACLE_TRACK" = "guest-alpine323" ]; then
    echo "$PKG/probes/expected/guest-alpine323/${p}.line"
    return
  fi
  if [ -f "$PKG/probes/expected/user/${p}.line" ]; then
    echo "$PKG/probes/expected/user/${p}.line"
  else
    echo "$PKG/probes/expected/${p}.line"
  fi
}

resolve_cases() {
  p="$1"
  if [ "$ORACLE_TRACK" = "guest-alpine323" ]; then
    echo "$PKG/probes/expected/guest-alpine323/${p}.cases"
    return
  fi
  if [ -f "$PKG/probes/expected/user/${p}.cases" ]; then
    echo "$PKG/probes/expected/user/${p}.cases"
  else
    echo "$PKG/probes/expected/${p}.cases"
  fi
}

verify_one() {
  p="$1"
  cases="$(resolve_cases "$p")"
  linef="$(resolve_line "$p")"
  if [ -f "$cases" ] && [ -f "$linef" ]; then
    echo "verify-oracle: both .cases and .line for probe $p" >&2
    return 1
  fi
  if [ ! -f "$cases" ] && [ ! -f "$linef" ]; then
    echo "Missing expected for probe $p (.line or .cases) [track=$ORACLE_TRACK]" >&2
    return 1
  fi
  test -x "$OUT/$p" || { echo "Missing $OUT/$p — run: $0 build" >&2; return 1; }

  if [ "$ORACLE_TRACK" = "guest-alpine323" ]; then
    if [ ! -f "$GUEST_SCRIPT" ]; then
      echo "Missing $GUEST_SCRIPT" >&2
      return 1
    fi
    if [ -z "${STARRY_LINUX_GUEST_IMAGE:-}" ]; then
      if [ "${VERIFY_STRICT:-0}" = 1 ]; then
        echo "STRICT: STARRY_LINUX_GUEST_IMAGE not set for guest track" >&2
        return 2
      fi
      echo "SKIP guest-oracle $p: STARRY_LINUX_GUEST_IMAGE not set" >&2
      return 0
    fi
    if [ -f "$cases" ]; then
      t1="$(mktemp)"
      t2="$(mktemp)"
      env STARRY_LINUX_GUEST_IMAGE="$STARRY_LINUX_GUEST_IMAGE" bash "$GUEST_SCRIPT" "$OUT/$p" | tr -d '\r' | "$PKG/scripts/extract-case-lines.sh" >"$t1"
      sort -u "$cases" >"$t2"
      if ! cmp -s "$t1" "$t2"; then
        echo "DIFF oracle $p (.cases) [guest]:" >&2
        diff -u "$t2" "$t1" >&2 || true
        rm -f "$t1" "$t2"
        return 1
      fi
      rm -f "$t1" "$t2"
      echo "verify-oracle OK: $p (guest .cases)"
      return 0
    fi
    got="$(env STARRY_LINUX_GUEST_IMAGE="$STARRY_LINUX_GUEST_IMAGE" bash "$GUEST_SCRIPT" "$OUT/$p" | tr -d '\r' | grep -m1 '^CASE ' || true)"
    want="$(cat "$linef")"
    if [ "$got" != "$want" ]; then
      echo "DIFF oracle $p [guest]:" >&2
      echo "  want: $want" >&2
      echo "  got:  $got" >&2
      return 1
    fi
    echo "verify-oracle OK: $p (guest) -> $want"
    return 0
  fi

  if ! command -v "$QEMU_RV64" >/dev/null 2>&1; then
    if [ "${VERIFY_STRICT:-0}" = 1 ]; then
      echo "STRICT: missing $QEMU_RV64 (set VERIFY_STRICT=0 to allow SKIP)" >&2
      return 2
    fi
    echo "SKIP: $QEMU_RV64 not installed" >&2
    return 0
  fi
  if [ -f "$cases" ]; then
    t1="$(mktemp)"
    t2="$(mktemp)"
    "$QEMU_RV64" "$OUT/$p" 2>/dev/null | tr -d '\r' | "$PKG/scripts/extract-case-lines.sh" >"$t1"
    sort -u "$cases" >"$t2"
    if ! cmp -s "$t1" "$t2"; then
      echo "DIFF oracle $p (.cases):" >&2
      diff -u "$t2" "$t1" >&2 || true
      rm -f "$t1" "$t2"
      return 1
    fi
    rm -f "$t1" "$t2"
    echo "verify-oracle OK: $p (structured .cases)"
    return 0
  fi
  got="$("$QEMU_RV64" "$OUT/$p" 2>/dev/null | tr -d '\r' | grep -m1 '^CASE ' || true)"
  want="$(cat "$linef")"
  if [ "$got" != "$want" ]; then
    echo "DIFF oracle $p:" >&2
    echo "  want: $want" >&2
    echo "  got:  $got" >&2
    return 1
  fi
  echo "verify-oracle OK: $p -> $want"
  return 0
}

cmd="${1:-help}"
case "$cmd" in
  build)
    exec "$PKG/scripts/build-probes.sh"
    ;;
  oracle)
    p="${2:-write_stdout}"
    test -x "$OUT/$p" || { echo "Missing $OUT/$p — run: $0 build" >&2; exit 1; }
    if ! command -v "$QEMU_RV64" >/dev/null 2>&1; then
      echo "Missing $QEMU_RV64 (install qemu-user / qemu-system user package)" >&2
      exit 1
    fi
    "$QEMU_RV64" "$OUT/$p"
    ;;
  oracle-guest)
    p="${2:-write_stdout}"
    test -x "$OUT/$p" || { echo "Missing $OUT/$p — run: $0 build" >&2; exit 1; }
    if [ -z "${STARRY_LINUX_GUEST_IMAGE:-}" ]; then
      echo "STARRY_LINUX_GUEST_IMAGE is not set" >&2
      exit 1
    fi
    if [ ! -f "$GUEST_SCRIPT" ]; then
      echo "Missing $GUEST_SCRIPT" >&2
      exit 1
    fi
    exec env STARRY_LINUX_GUEST_IMAGE="$STARRY_LINUX_GUEST_IMAGE" bash "$GUEST_SCRIPT" "$OUT/$p"
    ;;
  verify-oracle)
    p="${2:-write_stdout}"
    set +e
    verify_one "$p"
    rc=$?
    set -e
    exit "$rc"
    ;;
  verify-oracle-all)
    failed=0
    strict_fail=0
    any=0
    donef="$(mktemp)"
    : >"$donef"
    trap 'rm -f "$donef"' EXIT
    if [ "$ORACLE_TRACK" = "guest-alpine323" ]; then
      list_exp="$(find "$PKG/probes/expected/guest-alpine323" -maxdepth 1 \( -name '*.line' -o -name '*.cases' \) 2>/dev/null || true)"
    else
      list_exp="$(
        { find "$PKG/probes/expected/user" -maxdepth 1 \( -name '*.line' -o -name '*.cases' \) 2>/dev/null
          find "$PKG/probes/expected" -maxdepth 1 \( -name '*.line' -o -name '*.cases' \) 2>/dev/null
        } | sort -u
      )"
    fi
    # shellcheck disable=SC2086
    for exp in $list_exp; do
      [ -f "$exp" ] || continue
      case "$exp" in
        */guest-alpine323/README.md) continue ;;
      esac
      any=1
      b=$(basename "$exp")
      case "$b" in
        *.line) base="${b%.line}" ;;
        *.cases) base="${b%.cases}" ;;
        *) continue ;;
      esac
      if grep -qxF "$base" "$donef"; then
        continue
      fi
      echo "$base" >>"$donef"
      set +e
      verify_one "$base"
      rc=$?
      set -e
      if [ "$rc" -eq 2 ]; then
        strict_fail=1
        failed=1
      elif [ "$rc" -ne 0 ]; then
        failed=1
      fi
    done
    if [ "$any" -eq 0 ]; then
      echo "No expected files for track=$ORACLE_TRACK" >&2
      exit 1
    fi
    if [ "$strict_fail" -eq 1 ]; then
      exit 2
    fi
    exit "$failed"
    ;;
  help|*)
    usage
    ;;
esac
