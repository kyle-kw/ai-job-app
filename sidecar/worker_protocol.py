from __future__ import annotations

import json
import sys
import traceback
from datetime import datetime, timedelta, timezone
from typing import Any

SHANGHAI_TZ = timezone(timedelta(hours=8), name="Asia/Shanghai")
APP_VERSION = "0.2.4"
PROTOCOL_VERSION = "2"


def configure_utf8_stdio() -> None:
    """Keep the JSONL boundary UTF-8 even on Chinese Windows code pages."""
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            reconfigure(encoding="utf-8", errors="strict")


configure_utf8_stdio()
PROTOCOL_STDOUT = sys.stdout


def emit(kind: str, **payload: Any) -> bool:
    """Write one protocol event without crashing if the parent closed its pipe."""
    try:
        print(
            json.dumps({"type": kind, **payload}, ensure_ascii=False),
            file=PROTOCOL_STDOUT,
            flush=True,
        )
        return True
    except (OSError, ValueError, AttributeError):
        return False


def print_current_exception() -> None:
    """Best-effort traceback output for windowed/PyInstaller sidecars."""
    try:
        traceback.print_exc(file=sys.stderr)
    except (OSError, ValueError, AttributeError):
        pass


def now() -> str:
    return datetime.now(SHANGHAI_TZ).isoformat()
