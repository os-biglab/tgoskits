#!/usr/bin/env python3
from __future__ import annotations

import io
import os
import re
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from importlib import util as importlib_util
from pathlib import Path
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[1]
ARCH = "riscv64"
TARGET = "riscv64gc-unknown-none-elf"
PROMPT = "root@starry:/root #"
SERIAL_PORT = 4444
ALPINE_REPO = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/main/riscv64"
NGINX_APK = "nginx-1.28.3-r0.apk"
PCRE2_APK = "pcre2-10.47-r0.apk"
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
        except Exception as exc:  # pragma: no cover - fallback path
            last_error = exc
            if archive.exists():
                archive.unlink()
    else:
        raise RuntimeError(f"failed to download {MUSL_TOOLCHAIN}.tgz: {last_error}")

    print(f"[setup] extracting {archive}")
    with tarfile.open(archive, "r:gz") as tf:
        try:
            tf.extractall(toolchains_dir, filter="data")
        except TypeError:  # pragma: no cover - Python < 3.12 fallback
            tf.extractall(toolchains_dir)
    archive.unlink(missing_ok=True)

    if not local_cc.exists():
        raise FileNotFoundError(f"musl toolchain install failed, missing {local_cc}")


def ensure_kernel_image() -> Path:
    candidates = [
        ROOT / f"target/{TARGET}/release/starryos.bin",
        ROOT / f"target/{TARGET}/release/starryos",
        ROOT / f"target/{TARGET}/release/tgoskits_{ARCH}-qemu-virt.bin",
        ROOT / f"tgoskits_{ARCH}-qemu-virt.bin",
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
    rootfs = ROOT / "target" / TARGET / f"rootfs-{ARCH}.img"
    rootfs.unlink(missing_ok=True)
    local_rootfs = ROOT / "os" / "StarryOS" / f"rootfs-{ARCH}.img"
    if local_rootfs.exists():
        print(f"[setup] copying local rootfs from {local_rootfs}")
        shutil.copy2(local_rootfs, rootfs)
    else:
        print("[setup] preparing StarryOS rootfs via cargo xtask")
        last_error: subprocess.CalledProcessError | None = None
        for attempt in range(1, 4):
            try:
                run(["cargo", "xtask", "starry", "rootfs", "--arch", ARCH], ROOT)
                break
            except subprocess.CalledProcessError as exc:
                last_error = exc
                print(f"[setup] rootfs attempt {attempt} failed, retrying...", file=sys.stderr)
                time.sleep(3)
        else:
            assert last_error is not None
            raise last_error
    if not rootfs.exists():
        raise FileNotFoundError(f"rootfs image not found: {rootfs}")
    return rootfs


def download_apk(name: str) -> bytes:
    url = f"{ALPINE_REPO}/{name}"
    last_error: Exception | None = None
    for attempt in range(1, 4):
        try:
            print(f"[setup] downloading {name} from {url}")
            with urlopen(url, timeout=120) as response:
                return response.read()
        except Exception as exc:  # pragma: no cover - network fallback
            last_error = exc
            print(f"[setup] download attempt {attempt} for {name} failed: {exc}", file=sys.stderr)
            time.sleep(3)
    assert last_error is not None
    raise last_error


def extract_apk(apk_bytes: bytes, workdir: Path) -> Path:
    extracted = workdir / "apk"
    extracted.mkdir(parents=True, exist_ok=True)
    with tarfile.open(fileobj=io.BytesIO(apk_bytes), mode="r:gz") as tf:
        for member in tf.getmembers():
            if member.name.startswith("."):
                continue
            if member.isdir():
                (extracted / member.name).mkdir(parents=True, exist_ok=True)
                continue
            if member.issym():
                link_path = extracted / member.name
                link_path.parent.mkdir(parents=True, exist_ok=True)
                if link_path.exists():
                    link_path.unlink()
                os.symlink(member.linkname, link_path)
                continue
            if member.isfile():
                dest = extracted / member.name
                dest.parent.mkdir(parents=True, exist_ok=True)
                with tf.extractfile(member) as src, dest.open("wb") as dst:
                    shutil.copyfileobj(src, dst)
                dest.chmod(member.mode & 0o777)
    return extracted


def image_path_exists(image: Path, path: str) -> bool:
    result = subprocess.run(
        ["debugfs", "-R", f"stat {path}", str(image)],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode == 0 and "Inode:" in result.stdout


def debugfs_batch(image: Path, commands: list[str]) -> None:
    with tempfile.NamedTemporaryFile("w", delete=False) as fp:
        for line in commands:
            fp.write(line.rstrip() + "\n")
        path = Path(fp.name)
    try:
        subprocess.run(["debugfs", "-w", "-f", str(path), str(image)], check=True)
    finally:
        path.unlink(missing_ok=True)


def write_tree_to_rootfs(image: Path, source_root: Path) -> None:
    dirs: list[str] = []
    files: list[tuple[Path, str]] = []
    symlinks: list[tuple[Path, str]] = []
    for src in source_root.rglob("*"):
        rel = src.relative_to(source_root).as_posix()
        if src.is_dir():
            dirs.append(rel)
        elif src.is_symlink():
            symlinks.append((src, rel))
        elif src.is_file():
            files.append((src, rel))

    commands: list[str] = ["cd /"]
    created_dirs: set[str] = set()
    wanted_dirs: set[str] = set()
    for dst in dirs:
        path = Path(dst)
        while str(path) not in (".", ""):
            wanted_dirs.add(path.as_posix())
            path = path.parent
    wanted_dirs.update({"run/nginx"})

    for dst in sorted(wanted_dirs, key=lambda item: item.count("/")):
        if image_path_exists(image, dst) or dst in created_dirs:
            continue
        parent = str(Path(dst).parent)
        if parent not in (".", "") and parent not in created_dirs and not image_path_exists(image, parent):
            commands.append(f"mkdir {parent}")
            created_dirs.add(parent)
        commands.append(f"mkdir {dst}")
        created_dirs.add(dst)

    def append_file_ops(src: Path, dst: str) -> None:
        parent = str(Path(dst).parent)
        basename = Path(dst).name
        if parent not in (".", ""):
            commands.append(f"cd {parent}")
        commands.append(f"write {src} {basename}")
        mode = src.stat().st_mode & 0o777
        if mode != 0o644:
            commands.append(f"set_inode_field {basename} mode 0{mode:03o}")
        if parent not in (".", ""):
            commands.append("cd /")

    for src, dst in files:
        parent = str(Path(dst).parent)
        if parent not in (".", "") and parent not in created_dirs and not image_path_exists(image, parent):
            commands.append(f"mkdir {parent}")
            created_dirs.add(parent)
        append_file_ops(src, dst)

    for src, dst in symlinks:
        parent = str(Path(dst).parent)
        basename = Path(dst).name
        if parent not in (".", "") and parent not in created_dirs and not image_path_exists(image, parent):
            commands.append(f"mkdir {parent}")
            created_dirs.add(parent)
        if parent not in (".", ""):
            commands.append(f"cd {parent}")
        commands.append(f"symlink {basename} {os.readlink(src)}")
        if parent not in (".", ""):
            commands.append("cd /")

    if commands:
        debugfs_batch(image, commands)


def inject_nginx_into_rootfs(rootfs: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="nginx-smoke-") as td:
        tmp = Path(td)
        nginx_dir = extract_apk(download_apk(NGINX_APK), tmp)
        pcre2_dir = extract_apk(download_apk(PCRE2_APK), tmp / "pcre2")
        nginx_conf = nginx_dir / "etc" / "nginx" / "nginx.conf"
        if nginx_conf.exists():
            text = nginx_conf.read_text()
            nginx_conf.write_text(text.replace("user nginx;", "user root;"))
        write_tree_to_rootfs(rootfs, pcre2_dir)
        write_tree_to_rootfs(rootfs, nginx_dir)


def start_qemu(kernel: Path, rootfs: Path) -> subprocess.Popen[str]:
    if not shutil.which("qemu-system-riscv64"):
        raise FileNotFoundError("qemu-system-riscv64 not found in PATH")
    return subprocess.Popen(
        [
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
            "-nographic",
            "-monitor",
            "none",
            "-serial",
            "tcp::4444,server=on",
        ],
        cwd=ROOT,
        stderr=subprocess.PIPE,
        text=True,
    )


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
            raise RuntimeError(f"QEMU exited prematurely: {stderr.strip()}")
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
        self.send(f"{command}; printf '{marker}%s\\n' \"$?\"")
        deadline = time.time() + timeout
        while True:
            marker_index = self.buffer.rfind(marker)
            if marker_index != -1:
                tail = self.buffer[marker_index + len(marker) :]
                match = re.match(r"(?P<code>\d+)\r?\n", tail)
                if match is not None:
                    return int(match.group("code")), self.buffer
            if time.time() > deadline:
                raise TimeoutError(f"timed out waiting for exit marker {marker!r}")
            chunk = self.sock.recv(4096)
            if not chunk:
                raise ConnectionError("serial connection closed")
            text = chunk.decode("utf-8", errors="ignore")
            self.buffer += text
            print(text, end="")


def main() -> int:
    kernel = ensure_kernel_image()
    rootfs = prepare_rootfs()
    inject_nginx_into_rootfs(rootfs)

    qemu_stderr_lines: list[str] = []
    proc = start_qemu(kernel, rootfs)
    session: SerialSession | None = None
    try:
        wait_for_serial(proc, qemu_stderr_lines)
        session = SerialSession("127.0.0.1", SERIAL_PORT)
        session.read_until(PROMPT, timeout=90)
        session.run("stty -echo", timeout=30)

        steps = [
            "/usr/sbin/nginx",
        ]
        for step in steps:
            code, _ = session.run(step, timeout=180)
            if code != 0:
                raise RuntimeError(f"guest command failed: {step!r} (exit {code})")

        session.send("exit")
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
