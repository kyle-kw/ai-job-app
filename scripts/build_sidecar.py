#!/usr/bin/env python3
from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
SIDECAR = ROOT / "sidecar"
BINARIES = ROOT / "src-tauri" / "binaries"
PYTHON_VERSION = "3.13.6"
UV_VERSION = "0.11.24"


def host_target() -> str:
    output = subprocess.check_output(["rustc", "-vV"], text=True)
    for line in output.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    raise RuntimeError("Unable to determine Rust host target")


def main() -> int:
    if ".".join(str(value) for value in sys.version_info[:3]) != PYTHON_VERSION:
        raise RuntimeError(f"Sidecar build requires Python {PYTHON_VERSION}; found {sys.version.split()[0]}")
    target = os.environ.get("TAURI_TARGET") or host_target()
    suffix = ".exe" if target.endswith("windows-msvc") else ""
    BINARIES.mkdir(parents=True, exist_ok=True)
    build_venv = SIDECAR / ".build-venv"
    uv = shutil.which("uv")
    if not uv:
        raise RuntimeError(f"uv {UV_VERSION} is required to build the sidecar")
    installed_uv = subprocess.check_output([uv, "--version"], text=True).split()[1]
    if installed_uv != UV_VERSION:
        raise RuntimeError(f"Sidecar build requires uv {UV_VERSION}; found {installed_uv}")
    environment = dict(os.environ)
    environment.setdefault("UV_CACHE_DIR", str(ROOT / ".uv-cache"))
    environment["UV_PROJECT_ENVIRONMENT"] = str(build_venv)
    subprocess.check_call([
        uv, "sync", "--project", str(SIDECAR), "--locked", "--group", "build",
        "--no-install-project", "--python", PYTHON_VERSION,
    ], env=environment)
    build_python = build_venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    name = "job-assistant-sidecar"
    work_path = SIDECAR / "build"
    dist_path = SIDECAR / "dist"
    for directory in (work_path, dist_path):
        resolved = directory.resolve()
        if resolved.parent != SIDECAR.resolve():
            raise RuntimeError(f"Refusing to clean unexpected build path: {resolved}")
        shutil.rmtree(resolved, ignore_errors=True)
        resolved.mkdir(parents=True, exist_ok=True)
    pyinstaller_args = [
        str(build_python), "-m", "PyInstaller", "--noconfirm", "--clean", "--onefile",
    ]
    if os.name == "nt":
        pyinstaller_args.append("--noconsole")
    pyinstaller_args.extend([
        "--name", name,
        "--paths", str(SIDECAR),
        "--collect-all", "rendercv",
        "--collect-all", "rendercv_fonts",
        "--collect-all", "typst",
        "--collect-all", "pypdfium2",
        "--collect-all", "PIL",
        "--hidden-import", "vendor.boss_cdp_raw",
        "--add-data", f"{SIDECAR / 'vendor' / 'city_codes.json'}{os.pathsep}vendor",
        "--distpath", str(dist_path),
        "--workpath", str(work_path),
        "--specpath", str(work_path),
        str(SIDECAR / "worker.py"),
    ])
    subprocess.check_call(pyinstaller_args)
    built = SIDECAR / "dist" / f"{name}{suffix}"
    destination = BINARIES / f"{name}-{target}{suffix}"
    shutil.copy2(built, destination)
    print(f"Built {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
