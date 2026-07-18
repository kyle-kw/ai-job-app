from __future__ import annotations

import contextlib
import io
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import uuid
from collections import Counter
from typing import Any

try:
    from worker_protocol import APP_VERSION, PROTOCOL_VERSION, emit, now, print_current_exception
except ImportError:
    from sidecar.worker_protocol import (
        APP_VERSION,
        PROTOCOL_VERSION,
        emit,
        now,
        print_current_exception,
    )

_active_boss: Any | None = None
_cleaning_boss = False


def split_pipe(value: Any) -> list[str]:
    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    return [part.strip() for part in str(value or "").split("|") if part.strip()]


def stable_job_id(external_id: str, company: str, title: str, location: str) -> str:
    source = external_id or f"{company}|{title}|{location}"
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"boss:{source}"))


def normalize_job(raw: dict[str, Any], detail: dict[str, Any] | None = None) -> dict[str, Any]:
    detail = detail or {}
    labels = split_pipe(raw.get("job_labels") or raw.get("tags") or detail.get("tags_list"))
    experience = next(
        (item for item in labels if "年" in item or item in {"应届生", "经验不限", "在校生"}), ""
    )
    degree_values = {"初中", "中专", "高中", "大专", "本科", "硕士", "博士", "学历不限"}
    degree = next((item for item in labels if any(value in item for value in degree_values)), "")
    external_id = str(raw.get("job_id") or raw.get("encrypt_job_id") or "")
    company = str(raw.get("boss_name") or detail.get("company") or "")
    # BOSS list data uses boss_name for the company display in the reference scraper.
    company = str(detail.get("company") or company)
    title = str(raw.get("title") or detail.get("title") or "")
    location = str(raw.get("location") or detail.get("location") or "")
    skills = split_pipe(raw.get("skills"))
    skills.extend(str(item) for item in detail.get("skill_tags", []) if str(item).strip())
    skills = list(dict.fromkeys(skills))
    seen = now()
    return {
        "id": stable_job_id(external_id, company, title, location),
        "source": "boss",
        "externalId": external_id,
        "title": title,
        "company": company,
        "salary": str(raw.get("salary") or detail.get("salary") or "薪资面议"),
        "location": location,
        "experience": experience,
        "degree": degree,
        "companyScale": str(raw.get("company_scale") or ""),
        "companyStage": str(raw.get("company_stage") or ""),
        "industry": str(raw.get("company_industry") or ""),
        "skills": skills,
        "welfare": split_pipe(raw.get("welfare")),
        "description": str(detail.get("jd") or ""),
        "sourceUrl": str(raw.get("job_link") or detail.get("job_link") or detail.get("link") or ""),
        "bossName": None,
        "bossTitle": str(raw.get("boss_title") or "") or None,
        "firstSeen": seen,
        "lastSeen": seen,
        "isNew": True,
        "fit": None,
        "greeting": None,
        "patches": [],
    }


def market_report(jobs: list[dict[str, Any]], keyword: str, city: str) -> str:
    skills = Counter(skill for job in jobs for skill in job.get("skills", []))
    experience = Counter(job.get("experience") for job in jobs if job.get("experience"))
    degrees = Counter(job.get("degree") for job in jobs if job.get("degree"))
    top_skills = "、".join(item for item, _ in skills.most_common(6)) or "数据不足"
    top_experience = experience.most_common(1)[0][0] if experience else "数据不足"
    top_degree = degrees.most_common(1)[0][0] if degrees else "数据不足"
    return (
        f"## 本次岗位样本观察\n\n"
        f"- 本次整理 **{len(jobs)}** 个“{keyword}”本地岗位样本，搜索范围为 {city}。\n"
        f"- 当前有限样本中反复出现的技能包括 **{top_skills}**。\n"
        f"- 当前样本最常见的经验要求是 **{top_experience}**，学历要求以 **{top_degree}** 为主。\n\n"
        f"> 这些结果只代表本次有限页样本。简历可优先核对相关真实经历；市场要求不能作为候选人经历证据。"
    )


def load_boss_module():
    try:
        from vendor import boss_cdp_raw as boss
    except ImportError:
        from sidecar.vendor import boss_cdp_raw as boss
    if not boss.require_runtime_dependencies("requests", "websocket"):
        raise RuntimeError("BOSS 抓取依赖缺失，请重新构建 sidecar。")
    return boss


def setup_boss(params: dict[str, Any]) -> dict[str, Any]:
    global _active_boss
    boss = load_boss_module()
    _active_boss = boss
    reset_requested = bool(params.get("resetProfile", False))
    outcome: dict[str, Any] = {
        "loginSucceeded": False,
        "resetRequested": reset_requested,
        "cleanupSucceeded": True,
        "closedProcesses": 0,
        "error": None,
    }

    def record_cleanup(cleanup: dict[str, Any]) -> None:
        outcome["closedProcesses"] += int(cleanup["closedProcesses"])
        if not cleanup["cleanupSucceeded"]:
            outcome["cleanupSucceeded"] = False
            cleanup_error = str(cleanup.get("error") or "无法确认专用 Chrome 已关闭。")
            if not outcome["error"]:
                outcome["error"] = cleanup_error

    try:
        if reset_requested:
            # Never let prepare_cdp_profile delete the profile while one of its
            # Chrome processes is still alive. The check is scoped to the exact
            # --user-data-dir used by the sidecar, not the user's normal Chrome.
            reset_cleanup = close_boss_session(boss, progress=12, action="重新配置前")
            record_cleanup(reset_cleanup)
            if not reset_cleanup["cleanupSucceeded"]:
                raise RuntimeError(f"无法安全重置 BOSS 专用浏览器配置：{reset_cleanup['error']}")

        ensure_boss_session(
            boss,
            int(params.get("loginTimeout", 300)),
            reset_profile=reset_requested,
        )
        outcome["loginSucceeded"] = True
    except Exception as error:  # noqa: BLE001 -- setup failures are a structured outcome
        if not outcome["error"]:
            outcome["error"] = str(error)
    finally:
        record_cleanup(close_boss_session(boss, progress=90, action="配置结束后"))
        _active_boss = None

    return outcome


def ensure_boss_session(boss: Any, login_timeout: int, reset_profile: bool = False) -> None:
    with contextlib.redirect_stdout(io.StringIO()) as captured:
        code = boss.run_setup_chrome(
            9222,
            copy_login_state=False,
            reset_profile=reset_profile,
            wait_login=True,
            login_timeout=login_timeout,
        )
    if code != 0:
        raise RuntimeError(captured.getvalue().strip() or "BOSS 登录未完成。")


def close_boss_session(
    boss: Any,
    *,
    progress: int = 90,
    action: str = "任务结束后",
) -> dict[str, Any]:
    """Close and verify only Chrome processes using the isolated BOSS profile."""
    result: dict[str, Any] = {
        "cleanupSucceeded": False,
        "closedProcesses": 0,
        "error": None,
    }
    try:
        stopped = boss.stop_cdp_chrome(boss.DEFAULT_CDP_DATA_DIR)
        result["closedProcesses"] = int(stopped or 0)
        remaining = list(boss.chrome_pids_for_user_data_dir(boss.DEFAULT_CDP_DATA_DIR))
        if remaining:
            raise RuntimeError(f"仍检测到专用 Chrome 进程：{remaining}")
        result["cleanupSucceeded"] = True
        message = f"{action}已关闭并确认 BOSS 专用 Chrome（{stopped} 个进程）"
    except Exception as error:  # noqa: BLE001 -- cleanup failure must not discard scraped jobs
        result["error"] = str(error)
        message = f"{action}关闭 BOSS 专用 Chrome 失败：{error}"
    emit("progress", progress=progress, message=message)
    return result


def close_boss(_params: dict[str, Any]) -> dict[str, Any]:
    return close_boss_session(load_boss_module(), action="手动清理时")


def environment_status(_params: dict[str, Any]) -> dict[str, Any]:
    boss = load_boss_module()
    executable = pathlib.Path(str(boss.DEFAULT_CHROME_PATH)).expanduser()
    installed = executable.is_file()
    version = installed_chrome_version(executable) if installed else None
    return {
        "appVersion": APP_VERSION,
        "protocolVersion": PROTOCOL_VERSION,
        "chrome": {
            "installed": installed,
            "version": version,
            "executablePath": str(executable) if installed else None,
        },
    }


def installed_chrome_version(executable: pathlib.Path) -> str | None:
    """Read the installed Chrome version using platform-appropriate metadata.

    Chrome's Windows ``--version`` output follows the active system code page,
    which made UTF-8 sidecars display mojibake and could also activate a browser
    process. Official Windows installs keep a version-named directory beside
    chrome.exe, so use that static metadata there. macOS and Linux do not use
    the Windows directory layout, and their ``--version`` output is UTF-8 safe.
    """
    if os.name != "nt":
        try:
            completed = subprocess.run(
                [str(executable), "--version"],
                capture_output=True,
                text=True,
                timeout=5,
                check=False,
                encoding="utf-8",
                errors="replace",
            )
        except (OSError, subprocess.SubprocessError):
            return None
        return (completed.stdout or completed.stderr).strip() or None

    version_pattern = re.compile(r"^\d+(?:\.\d+){3}$")
    try:
        versions = [
            child.name
            for child in executable.parent.iterdir()
            if child.is_dir() and version_pattern.fullmatch(child.name)
        ]
    except OSError:
        return None
    if not versions:
        return None
    return max(versions, key=lambda value: tuple(int(part) for part in value.split(".")))


def clear_boss_data(_params: dict[str, Any]) -> dict[str, Any]:
    boss = load_boss_module()
    outcome = close_boss_session(boss, action="清除数据前")
    if not outcome["cleanupSucceeded"]:
        raise RuntimeError(outcome.get("error") or "无法确认 BOSS 专用 Chrome 已关闭")
    remaining = list(boss.chrome_pids_for_user_data_dir(boss.DEFAULT_CDP_DATA_DIR))
    if remaining:
        raise RuntimeError(f"仍检测到 BOSS 专用 Chrome 进程：{remaining}")
    deleted: list[str] = []
    for raw_path in (boss.DEFAULT_CDP_DATA_DIR, boss.DEFAULT_RESULT_DIR):
        path = pathlib.Path(str(raw_path)).expanduser()
        if path.exists():
            shutil.rmtree(path)
            deleted.append(str(path))
    return {"deleted": deleted, "remainingPids": []}


def scrape_jobs(params: dict[str, Any]) -> dict[str, Any]:
    global _active_boss
    if not str(params.get("keyword") or "").strip():
        raise ValueError("岗位关键词不能为空。")
    boss = load_boss_module()
    _active_boss = boss
    try:
        return _scrape_jobs(boss, params)
    finally:
        # Login timeouts, captcha errors, list/detail exceptions, and empty
        # results all travel through this path. Cleanup errors are reported as
        # progress but do not hide the original scrape result or exception.
        close_boss_session(boss)
        _active_boss = None


def _scrape_jobs(boss: Any, params: dict[str, Any]) -> dict[str, Any]:
    keyword = str(params.get("keyword") or "").strip()
    if not keyword:
        raise ValueError("岗位关键词不能为空。")
    city = str(params.get("city") or "上海").strip()
    pages = max(1, min(int(params.get("pages") or 1), 5))
    completed_detail_external_ids = {
        str(external_id).strip()
        for external_id in (params.get("completedDetailExternalIds") or [])
        if str(external_id).strip()
    }
    city_name, city_code = boss.resolve_city(city)
    city_code = str(city_code).strip()
    if not re.fullmatch(r"\d{9}", city_code):
        raise RuntimeError(
            f"无法识别城市“{city}”，请填写城市中文名（例如：上海）或 9 位 BOSS 城市代码。"
        )
    filters: dict[str, str] = {}
    mapping = {
        "salary": "salary",
        "experience": "experience",
        "degree": "degree",
        "companyScale": "scale",
    }
    for input_name, scraper_name in mapping.items():
        if params.get(input_name):
            filters[scraper_name] = str(params[input_name])
    emit("progress", progress=12, message="正在检查 BOSS 登录状态；若出现登录界面，请完成登录")
    ensure_boss_session(boss, int(params.get("loginTimeout", 300)))
    emit("progress", progress=24, message=f"登录状态正常，城市：{city_name}（{city_code}）")
    with tempfile.TemporaryDirectory(prefix="ai-job-scrape-") as temporary:
        output = str(pathlib.Path(temporary) / "jobs.json")
        details_output = str(pathlib.Path(temporary) / "details.json")
        with contextlib.redirect_stdout(io.StringIO()):
            listing = boss.scrape_list(
                keyword,
                city_code,
                pages,
                filters,
                output,
                cdp_port=9222,
                fmt="json",
                allow_dom_fallback=False,
                on_job=lambda raw: emit("job", phase="list", job=normalize_job(raw)),
            )

            listed_jobs = list(listing.get("jobs") or [])
            detail_candidates = [
                raw
                for raw in listed_jobs
                if str(raw.get("job_id") or raw.get("encrypt_job_id") or "").strip()
                not in completed_detail_external_ids
            ]
            skipped_existing = len(listed_jobs) - len(detail_candidates)
            detail_state = {
                "total": len(listed_jobs),
                "succeeded": 0,
                "skipped": skipped_existing,
                "failed": 0,
                "processed": skipped_existing,
            }

            emit(
                "progress",
                progress=55,
                message=(
                    f"岗位列表抓取完成，共 {len(listed_jobs)} 个；"
                    f"已有详情跳过 {skipped_existing} 个"
                ),
                detailTotal=len(listed_jobs),
                detailProcessed=skipped_existing,
                detailSucceeded=0,
                detailSkipped=skipped_existing,
                detailFailed=0,
            )

            def emit_detail(raw: dict[str, Any], detail: dict[str, Any]) -> None:
                if str(detail.get("jd") or "").strip():
                    emit("job", phase="detail", job=normalize_job(raw, detail))

            def emit_detail_progress(**detail_progress: Any) -> None:
                detail_state["succeeded"] = int(detail_progress.get("succeeded") or 0)
                detail_state["skipped"] = skipped_existing + int(
                    detail_progress.get("skipped") or 0
                )
                detail_state["failed"] = int(detail_progress.get("failed") or 0)
                detail_state["processed"] = skipped_existing + int(
                    detail_progress.get("processed") or 0
                )
                total = len(listed_jobs)
                progress = 55 + int(22 * detail_state["processed"] / max(total, 1))
                emit(
                    "progress",
                    progress=min(progress, 77),
                    message=(
                        "正在抓取岗位详情："
                        f"成功 {detail_state['succeeded']}，"
                        f"跳过 {detail_state['skipped']}，"
                        f"失败 {detail_state['failed']}"
                        f"（{detail_state['processed']}/{total}）"
                    ),
                    detailTotal=total,
                    detailProcessed=detail_state["processed"],
                    detailSucceeded=detail_state["succeeded"],
                    detailSkipped=detail_state["skipped"],
                    detailFailed=detail_state["failed"],
                )

            detail_listing = {**listing, "jobs": detail_candidates, "total": len(detail_candidates)}
            details = (
                boss.scrape_details(
                    detail_listing,
                    None,
                    details_output,
                    cdp_port=9222,
                    fmt="json",
                    on_detail=emit_detail,
                    on_progress=emit_detail_progress,
                )
                if detail_candidates
                else []
            )

    if not listing.get("jobs"):
        raise RuntimeError(
            f"BOSS 未返回岗位：关键词“{keyword}”，城市“{city_name}”（{city_code}）。"
            "请确认登录未过期、页面没有验证码，并尝试更宽泛的关键词。"
        )

    detail_by_id = {str(detail.get("job_id")): detail for detail in details or []}
    jobs = [
        normalize_job(raw, detail_by_id.get(str(raw.get("job_id"))))
        for raw in listing.get("jobs", [])
    ]
    emit(
        "progress",
        progress=78,
        message=(
            "岗位详情抓取完成："
            f"成功 {detail_state['succeeded']}，"
            f"跳过 {detail_state['skipped']}，"
            f"失败 {detail_state['failed']}"
        ),
        detailTotal=len(listing.get("jobs") or []),
        detailProcessed=detail_state["processed"],
        detailSucceeded=detail_state["succeeded"],
        detailSkipped=detail_state["skipped"],
        detailFailed=detail_state["failed"],
    )
    return {
        "jobs": jobs,
        "reportMarkdown": market_report(jobs, keyword, city_name),
        "resolvedCity": city_name,
        "cityCode": city_code,
        "detailSummary": detail_state,
    }


def cleanup_active_boss() -> None:
    global _active_boss, _cleaning_boss
    if _active_boss is None or _cleaning_boss:
        return
    _cleaning_boss = True
    try:
        close_boss_session(_active_boss, action="进程退出前")
    except Exception:  # noqa: BLE001 - shutdown cleanup is best effort
        print_current_exception()
    finally:
        _active_boss = None
        _cleaning_boss = False
