#!/usr/bin/env python3
from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import venv


ROOT = pathlib.Path(__file__).resolve().parents[1]
SIDECAR = ROOT / "sidecar"
BINARIES = ROOT / "src-tauri" / "binaries"


def host_target() -> str:
    output = subprocess.check_output(["rustc", "-vV"], text=True)
    for line in output.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    raise RuntimeError("Unable to determine Rust host target")


def main() -> int:
    target = os.environ.get("TAURI_TARGET") or host_target()
    suffix = ".exe" if target.endswith("windows-msvc") else ""
    BINARIES.mkdir(parents=True, exist_ok=True)
    build_venv = SIDECAR / ".build-venv"
    if not build_venv.exists():
        venv.EnvBuilder(with_pip=True).create(build_venv)
    build_python = build_venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    uv = shutil.which("uv")
    if uv:
        environment = dict(os.environ)
        environment.setdefault("UV_CACHE_DIR", str(ROOT / ".uv-cache"))
        subprocess.check_call([uv, "pip", "install", "--python", str(build_python), "-r", str(SIDECAR / "requirements-build.txt")], env=environment)
    else:
        subprocess.check_call([str(build_python), "-m", "pip", "install", "--disable-pip-version-check", "-r", str(SIDECAR / "requirements-build.txt")])
    name = "job-assistant-sidecar"
    work_path = SIDECAR / "build"
    dist_path = SIDECAR / "dist"
    for directory in (work_path, dist_path):
        resolved = directory.resolve()
        if resolved.parent != SIDECAR.resolve():
            raise RuntimeError(f"Refusing to clean unexpected build path: {resolved}")
        shutil.rmtree(resolved, ignore_errors=True)
        resolved.mkdir(parents=True, exist_ok=True)
    subprocess.check_call([
        str(build_python), "-m", "PyInstaller", "--noconfirm", "--clean", "--onefile",
        "--name", name,
        "--paths", str(SIDECAR),
        "--collect-all", "rendercv",
        "--collect-all", "rendercv_fonts",
        "--collect-all", "typst",
        "--collect-all", "pypdfium2",
        "--collect-all", "PIL",
        "--hidden-import", "vendor.boss_cdp_raw",
        "--distpath", str(dist_path),
        "--workpath", str(work_path),
        "--specpath", str(work_path),
        str(SIDECAR / "worker.py"),
    ])
    built = SIDECAR / "dist" / f"{name}{suffix}"
    destination = BINARIES / f"{name}-{target}{suffix}"
    shutil.copy2(built, destination)
    print(f"Built {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
