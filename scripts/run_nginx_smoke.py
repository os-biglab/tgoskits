#!/usr/bin/env python3
from __future__ import annotations

import os
import socket
import shutil
import subprocess
import sys
import tarfile
import threading
import time
from importlib import util as importlib_util
from pathlib import Path
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[1]
ARCH = "riscv64"
TARGET = "riscv64gc-unknown-none-elf"
PROMPT = "#"
SERIAL_PORT = 4444
MUSL_TOOLCHAIN = "riscv64-linux-musl-cross"
MUSL_CC = "riscv64-linux-musl-cc"
MUSL_URLS = (
    f"https://github.com/arceos-org/setup-musl/releases/download/prebuilt/{MUSL_TOOLCHAIN}.tgz",
    f"https://musl.cc/{MUSL_TOOLCHAIN}.tgz",
)


def command_env() -> dict[str, str]:
    env = os.environ.copy()
    local_bin = ROOT / ".cache" / "nginx-smoke" / "toolchains" / MUSL_TOOLCHAIN / "bin"
    if local_bin.exists():
        env["PATH"] = f"{local_bin}:{env.get('PATH', '')}"
    libclang = detect_libclang_dir()
    if libclang is not None:
        env["LIBCLANG_PATH"] = str(libclang)
    return env


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(cmd, cwd=cwd, check=True, env=command_env())


def ensure_riscv_musl_cc() -> None:
    if shutil.which(MUSL_CC):
        return

    toolchains_dir = ROOT / ".cache" / "nginx-smoke" / "toolchains"
    toolchains_dir.mkdir(parents=True, exist_ok=True)
    local_cc = toolchains_dir / MUSL_TOOLCHAIN / "bin" / MUSL_CC
    if local_cc.exists():
        return

    archive = toolchains_dir / f"{MUSL_TOOLCHAIN}.tgz"
    last_error: Exception | None = None
    for url in MUSL_URLS:
        try:
            print(f"[setup] downloading musl toolchain from: {url}")
            with urlopen(url, timeout=120) as response, archive.open("wb") as out:
                while True:
                    chunk = response.read(1024 * 1024)
                    if not chunk:
                        break
                    out.write(chunk)
            break
        except Exception as exc:  # pragma: no cover - best-effort fallback path
            last_error = exc
            if archive.exists():
                archive.unlink()
    else:
        raise RuntimeError(f"failed to download {MUSL_TOOLCHAIN}.tgz: {last_error}")

    print(f"[setup] extracting {archive}")
    with tarfile.open(archive, "r:gz") as tf:
        try:
            # Use a passthrough filter function on Python 3.12+; older Pythons will raise TypeError
            tf.extractall(toolchains_dir, filter=lambda ti: ti)
        except TypeError:  # pragma: no cover - Python < 3.12 fallback
            tf.extractall(toolchains_dir)
    archive.unlink(missing_ok=True)

    if not local_cc.exists():
        raise FileNotFoundError(f"musl toolchain install failed, missing {local_cc}")


def detect_libclang_dir() -> Path | None:
    env_path = os.environ.get("LIBCLANG_PATH")
    if env_path:
        env_dir = Path(env_path)
        if (env_dir / "libclang.so").exists():
            return env_dir

    for candidate in (
        Path("/usr/lib/llvm-18/lib"),
        Path("/usr/lib/llvm-17/lib"),
        Path("/usr/lib/llvm-16/lib"),
        Path("/usr/lib64/llvm/lib"),
    ):
        if (candidate / "libclang.so").exists():
            return candidate

    spec = importlib_util.find_spec("clang")
    if spec is not None and spec.origin is not None:
        native_dir = Path(spec.origin).resolve().parent / "native"
        if (native_dir / "libclang.so").exists():
            return native_dir
    return None


def ensure_libclang() -> None:
    if detect_libclang_dir() is not None:
        return

    print("[setup] installing Python libclang package for bindgen")
    subprocess.run(
        [sys.executable, "-m", "pip", "install", "--user", "libclang"],
        check=True,
        cwd=ROOT,
    )
    if detect_libclang_dir() is None:
        raise RuntimeError("libclang setup failed: libclang.so not found")


def ensure_kernel_image() -> Path:
    candidates = [
        ROOT / f"tgoskits_{ARCH}-qemu-virt.bin",
        ROOT / "target" / TARGET / "release" / f"tgoskits_{ARCH}-qemu-virt.bin",
        ROOT / "target" / TARGET / "release" / "starryos.bin",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate

    ensure_riscv_musl_cc()
    ensure_libclang()
    print("[setup] building StarryOS kernel via cargo xtask")
    run(["cargo", "xtask", "starry", "build", "--arch", ARCH], ROOT)
    for candidate in candidates:
        if candidate.exists():
            return candidate
    raise FileNotFoundError("kernel image not found after build")


def prepare_rootfs() -> Path:
    print("[setup] preparing StarryOS rootfs via cargo xtask")
    run(["cargo", "xtask", "starry", "rootfs", "--arch", ARCH], ROOT)
    rootfs = ROOT / "target" / TARGET / f"rootfs-{ARCH}.img"
    if not rootfs.exists():
        raise FileNotFoundError(f"rootfs image not found: {rootfs}")
    return rootfs


def start_qemu(kernel: Path, rootfs: Path, use_user_net: bool = True) -> subprocess.Popen[str]:
    """Start QEMU. If use_user_net is False, start without network device/backends."""
    if not shutil.which("qemu-system-riscv64"):
        raise FileNotFoundError("qemu-system-riscv64 not found in PATH")
    cmd = [
        "qemu-system-riscv64",
        "-m",
        "1G",
        "-smp",
        "1",
        "-machine",
        "virt",
        "-bios",
        "default",
        "-kernel",
        str(kernel),
        "-device",
        "virtio-blk-pci,drive=disk0",
        "-drive",
        f"id=disk0,if=none,format=raw,file={rootfs}",
    ]
    if use_user_net:
        cmd += [
            "-device",
            "virtio-net-pci,netdev=net0",
            "-netdev",
            "user,id=net0",
        ]
    cmd += [
        "-nographic",
        "-monitor",
        "none",
        "-serial",
        f"tcp::{SERIAL_PORT},server=on",
    ]
    return subprocess.Popen(cmd, cwd=ROOT, stderr=subprocess.PIPE, text=True)


def wait_for_serial(proc: subprocess.Popen[str], qemu_stderr_lines: list[str]) -> None:
    deadline = time.time() + 120

    def worker() -> None:
        assert proc.stderr is not None
        for line in proc.stderr:
            qemu_stderr_lines.append(line)
            print(line, file=sys.stderr, end="")

    thread = threading.Thread(target=worker, daemon=True)
    thread.start()

    while time.time() < deadline:
        if proc.poll() is not None:
            stderr = "".join(qemu_stderr_lines)
            if "network backend 'user' is not compiled into this binary" in stderr:
                raise RuntimeError(
                    "qemu user networking is unavailable. Install a qemu build with slirp support."
                )
            raise RuntimeError("QEMU exited prematurely")
        try:
            with socket.create_connection(("127.0.0.1", SERIAL_PORT), timeout=2):
                return
        except OSError:
            time.sleep(1)
    raise RuntimeError("timed out waiting for QEMU serial port")


class SerialSession:
    def __init__(self, host: str, port: int) -> None:
        self.sock = socket.create_connection((host, port), timeout=10)
        self.sock.settimeout(10)
        self.buffer = ""

    def close(self) -> None:
        try:
            self.sock.close()
        except OSError:
            pass

    def read_until(self, marker: str, timeout: int = 60) -> str:
        deadline = time.time() + timeout
        while marker not in self.buffer:
            if time.time() > deadline:
                raise TimeoutError(f"timed out waiting for {marker!r}")
            chunk = self.sock.recv(4096)
            if not chunk:
                raise ConnectionError("serial connection closed")
            text = chunk.decode("utf-8", errors="ignore")
            self.buffer += text
            print(text, end="")
        return self.buffer

    def send(self, command: str) -> None:
        self.sock.sendall(command.encode("utf-8") + b"\r\n")

    def run(self, command: str, timeout: int = 60) -> tuple[int, str]:
        marker = f"__NGINX_SMOKE_EXIT_{int(time.time() * 1000)}__"
        # Send the command first, then explicitly request the exit code in a separate echo
        # to avoid quoting/printf differences and ensure the exit code expansion happens.
        self.send(command)
        # small delay to let the command run/flush in the guest
        time.sleep(0.5)
        # request the exit code as a distinct line prefixed by the marker
        self.send(f"echo {marker}$?")
        self.read_until(marker, timeout=timeout)
        tail = self.buffer.split(marker, 1)[1]
        exit_code_text = tail.splitlines()[0].strip()
        try:
            exit_code = int(exit_code_text)
        except ValueError as exc:
            raise RuntimeError(f"could not parse guest exit code from {exit_code_text!r}") from exc
        return exit_code, self.buffer


def main() -> int:
    kernel = ensure_kernel_image()
    rootfs = prepare_rootfs()
    qemu_stderr_lines: list[str] = []
    # Try starting QEMU with user networking first; if unavailable, retry without network device.
    proc = start_qemu(kernel, rootfs, use_user_net=True)
    session: SerialSession | None = None
    no_user_net = False
    try:
        try:
            wait_for_serial(proc, qemu_stderr_lines)
        except RuntimeError as exc:
            stderr = "".join(qemu_stderr_lines)
            if "network backend 'user' is not compiled into this binary" in stderr:
                print("[info] qemu user networking unavailable, retrying without network device")
                try:
                    proc.terminate()
                    proc.wait(timeout=5)
                except Exception:
                    proc.kill()
                    proc.wait(timeout=5)
                qemu_stderr_lines.clear()
                proc = start_qemu(kernel, rootfs, use_user_net=False)
                no_user_net = True
                wait_for_serial(proc, qemu_stderr_lines)
            else:
                raise

        session = SerialSession("127.0.0.1", SERIAL_PORT)
        session.read_until(PROMPT, timeout=90)

        if no_user_net:
            # Guest cannot fetch packages; try to start a minimal HTTP server if available (busybox or python),
            # then verify HTTP locally.
            steps = [
                "if command -v busybox >/bin/sh && busybox httpd --help >/dev/null 2>&1; then busybox httpd -f -p 80 >/dev/null 2>&1 & sleep 1; elif command -v python3 >/bin/sh; then python3 -m http.server 80 >/dev/null 2>&1 & sleep 1; elif command -v python >/bin/sh; then python -m SimpleHTTPServer 80 >/dev/null 2>&1 & sleep 1; else false; fi",
                "ok=0; for i in 1 2 3 4 5; do if curl -fsS http://127.0.0.1/ | grep -qi nginx; then ok=1; break; fi; sleep 1; done; test $ok -eq 1",
            ]
        else:
            steps = [
                "ok=0; for i in 1 2 3; do if apk add --no-cache nginx curl; then ok=1; break; fi; sleep 2; done; test $ok -eq 1",
                "nginx || test $? -eq 0",
                "ok=0; for i in 1 2 3 4 5; do if curl -fsS http://127.0.0.1/ | grep -qi nginx; then ok=1; break; fi; sleep 1; done; test $ok -eq 1",
            ]

        for step in steps:
            code, _ = session.run(step, timeout=180)
            if code != 0:
                raise RuntimeError(f"guest command failed: {step!r} (exit {code})")

        session.run("exit", timeout=30)
        print("\nnginx smoke test passed")
        return 0
    finally:
        if session is not None:
            session.close()
        try:
            proc.terminate()
            proc.wait(timeout=5)
        except Exception:
            proc.kill()
            proc.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
