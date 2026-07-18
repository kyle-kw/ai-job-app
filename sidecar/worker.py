#!/usr/bin/env python3
"""One-line JSON RPC entry point for the Tauri sidecar."""

from __future__ import annotations

import atexit
import json
import os  # noqa: F401 - retained as a compatibility patch point for worker tests
import signal
import subprocess  # noqa: F401 - retained as a compatibility patch point for worker tests
import sys
from typing import Any

try:
    import browser_ops
    import pdf_renderer
    import resume_parser
    import worker_protocol
except ImportError:
    from sidecar import browser_ops, pdf_renderer, resume_parser, worker_protocol

RESUME_BOLD_KEYWORDS = pdf_renderer.RESUME_BOLD_KEYWORDS
RESUME_COLOR_THEMES = pdf_renderer.RESUME_COLOR_THEMES
profile_to_rendercv = pdf_renderer.profile_to_rendercv
render_resume = pdf_renderer.render_resume
resume_design = pdf_renderer.resume_design

base_profile = resume_parser.base_profile
display_degree = resume_parser.display_degree
entry_dates = resume_parser.entry_dates
extract_docx_text = resume_parser.extract_docx_text
extract_resume = resume_parser.extract_resume
extract_text = resume_parser.extract_text
first_value = resume_parser.first_value
normalize_date_pair = resume_parser.normalize_date_pair
profile_from_text = resume_parser.profile_from_text
profile_from_yaml = resume_parser.profile_from_yaml
render_date_fields = resume_parser.render_date_fields
render_pdf_page = resume_parser.render_pdf_page
rendercv_date = resume_parser.rendercv_date
rendercv_phone = resume_parser.rendercv_phone
split_skill_items = resume_parser.split_skill_items

APP_VERSION = worker_protocol.APP_VERSION
PROTOCOL_VERSION = worker_protocol.PROTOCOL_VERSION
SHANGHAI_TZ = worker_protocol.SHANGHAI_TZ
PROTOCOL_STDOUT = worker_protocol.PROTOCOL_STDOUT

load_boss_module = browser_ops.load_boss_module
close_boss_session = browser_ops.close_boss_session
ensure_boss_session = browser_ops.ensure_boss_session
installed_chrome_version = browser_ops.installed_chrome_version
normalize_job = browser_ops.normalize_job
market_report = browser_ops.market_report
now = worker_protocol.now
split_pipe = browser_ops.split_pipe
stable_job_id = browser_ops.stable_job_id

_active_boss: Any | None = None
_cleaning_boss = False


def configure_utf8_stdio() -> None:
    worker_protocol.configure_utf8_stdio()


def emit(kind: str, **payload: Any) -> bool:
    worker_protocol.PROTOCOL_STDOUT = PROTOCOL_STDOUT
    return worker_protocol.emit(kind, **payload)


def print_current_exception() -> None:
    worker_protocol.print_current_exception()


def _sync_browser_hooks() -> None:
    browser_ops.load_boss_module = load_boss_module
    browser_ops.close_boss_session = close_boss_session
    browser_ops.emit = emit


def _sync_state_from_browser() -> None:
    global _active_boss, _cleaning_boss
    _active_boss = browser_ops._active_boss
    _cleaning_boss = browser_ops._cleaning_boss


def setup_boss(params: dict[str, Any]) -> dict[str, Any]:
    _sync_browser_hooks()
    result = browser_ops.setup_boss(params)
    _sync_state_from_browser()
    return result


def close_boss(params: dict[str, Any]) -> dict[str, Any]:
    _sync_browser_hooks()
    result = browser_ops.close_boss(params)
    _sync_state_from_browser()
    return result


def environment_status(params: dict[str, Any]) -> dict[str, Any]:
    _sync_browser_hooks()
    return browser_ops.environment_status(params)


def clear_boss_data(params: dict[str, Any]) -> dict[str, Any]:
    _sync_browser_hooks()
    return browser_ops.clear_boss_data(params)


def scrape_jobs(params: dict[str, Any]) -> dict[str, Any]:
    _sync_browser_hooks()
    result = browser_ops.scrape_jobs(params)
    _sync_state_from_browser()
    return result


def cleanup_active_boss() -> None:
    _sync_browser_hooks()
    browser_ops.cleanup_active_boss()
    _sync_state_from_browser()


def handle_sigterm(_signum: int, _frame: Any) -> None:
    cleanup_active_boss()
    raise SystemExit(143)


OPERATIONS = {
    "setup_boss": setup_boss,
    "close_boss": close_boss,
    "clear_boss_data": clear_boss_data,
    "environment_status": environment_status,
    "scrape_jobs": scrape_jobs,
    "extract_resume": extract_resume,
    "render_resume": render_resume,
    "ping": lambda params: {"python": sys.version, "ok": True},
}

atexit.register(cleanup_active_boss)
if hasattr(signal, "SIGTERM"):
    signal.signal(signal.SIGTERM, handle_sigterm)


def main() -> int:
    line = sys.stdin.readline()
    if not line:
        emit("result", ok=False, error="没有收到请求。")
        return 2
    try:
        request = json.loads(line)
        operation = str(request.get("op") or "")
        handler = OPERATIONS.get(operation)
        if handler is None:
            raise RuntimeError(f"未知 sidecar 操作：{operation}")
        emit("progress", progress=5, message=f"开始 {operation}")
        data = handler(dict(request.get("params") or {}))
        emit("result", ok=True, data=data)
        return 0
    except Exception as error:  # noqa: BLE001 - process boundary must serialize every failure
        print_current_exception()
        emit("result", ok=False, error=str(error))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
