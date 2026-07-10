#!/usr/bin/env python3
"""One-line JSON RPC worker used by the Tauri backend.

The process reads one request from stdin and writes JSON Lines to stdout. Vendor
and RenderCV output is captured so stdout always remains machine-readable.
"""

from __future__ import annotations

import contextlib
import io
import json
import os
import pathlib
import re
import sys
import tempfile
import traceback
import uuid
from collections import Counter
from datetime import datetime, timedelta, timezone
from typing import Any


SHANGHAI_TZ = timezone(timedelta(hours=8), name="Asia/Shanghai")


def configure_utf8_stdio() -> None:
    """Keep the JSONL boundary UTF-8 even on Chinese Windows code pages."""
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            reconfigure(encoding="utf-8", errors="strict")


configure_utf8_stdio()


def emit(kind: str, **payload: Any) -> None:
    print(json.dumps({"type": kind, **payload}, ensure_ascii=False), flush=True)


def now() -> str:
    return datetime.now(SHANGHAI_TZ).isoformat()


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
    experience = next((item for item in labels if "年" in item or item in {"应届生", "经验不限", "在校生"}), "")
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
        f"## 本次岗位观察\n\n"
        f"- 本次整理 **{len(jobs)}** 个“{keyword}”岗位，范围为 {city}。\n"
        f"- 高频技能为 **{top_skills}**。\n"
        f"- 最常见经验要求是 **{top_experience}**，学历要求以 **{top_degree}** 为主。\n\n"
        f"> 建议先在简历前半页突出与高频技能直接相关、可量化的项目成果。"
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
    boss = load_boss_module()
    ensure_boss_session(boss, int(params.get("loginTimeout", 300)))
    return {"loggedIn": True}


def ensure_boss_session(boss: Any, login_timeout: int) -> None:
    with contextlib.redirect_stdout(io.StringIO()) as captured:
        code = boss.run_setup_chrome(
            9222,
            copy_login_state=False,
            reset_profile=False,
            wait_login=True,
            login_timeout=login_timeout,
        )
    if code != 0:
        raise RuntimeError(captured.getvalue().strip() or "BOSS 登录未完成。")


def scrape_jobs(params: dict[str, Any]) -> dict[str, Any]:
    boss = load_boss_module()
    keyword = str(params.get("keyword") or "AI Agent").strip()
    city = str(params.get("city") or "上海").strip()
    pages = max(1, min(int(params.get("pages") or 3), 10))
    city_name, city_code = boss.resolve_city(city)
    city_code = str(city_code).strip()
    if not re.fullmatch(r"\d{9}", city_code):
        raise RuntimeError(f"无法识别城市“{city}”，请填写城市中文名（例如：上海）或 9 位 BOSS 城市代码。")
    filters: dict[str, str] = {}
    mapping = {"salary": "salary", "experience": "experience", "degree": "degree", "companyScale": "scale"}
    for input_name, scraper_name in mapping.items():
        if params.get(input_name):
            filters[scraper_name] = str(params[input_name])
    emit("progress", progress=12, message="正在启动或连接 BOSS 专用浏览器")
    ensure_boss_session(boss, int(params.get("loginTimeout", 300)))
    emit("progress", progress=24, message=f"登录状态正常，城市：{city_name}（{city_code}）")
    with tempfile.TemporaryDirectory(prefix="ai-job-scrape-") as temporary:
        output = str(pathlib.Path(temporary) / "jobs.json")
        details_output = str(pathlib.Path(temporary) / "details.json")
        with contextlib.redirect_stdout(io.StringIO()):
            listing = boss.scrape_list(keyword, city_code, pages, filters, output, cdp_port=9222, fmt="json", allow_dom_fallback=False)
            emit("progress", progress=55, message="岗位列表抓取完成")
            details = boss.scrape_details(listing, None, details_output, cdp_port=9222, fmt="json") if listing.get("jobs") else []

    if not listing.get("jobs"):
        raise RuntimeError(
            f"BOSS 未返回岗位：关键词“{keyword}”，城市“{city_name}”（{city_code}）。"
            "请确认登录未过期、页面没有验证码，并尝试更宽泛的关键词。"
        )

    detail_by_id = {str(detail.get("job_id")): detail for detail in details or []}
    jobs = [normalize_job(raw, detail_by_id.get(str(raw.get("job_id")))) for raw in listing.get("jobs", [])]
    emit("progress", progress=78, message="岗位详情抓取完成")
    return {
        "jobs": jobs,
        "reportMarkdown": market_report(jobs, keyword, city_name),
        "resolvedCity": city_name,
        "cityCode": city_code,
    }


def extract_text(path: pathlib.Path) -> tuple[str, dict[str, Any] | None]:
    suffix = path.suffix.lower()
    if suffix == ".pdf":
        from pypdf import PdfReader
        text = "\n".join(page.extract_text() or "" for page in PdfReader(str(path)).pages)
        if len(text.strip()) < 30:
            raise RuntimeError("PDF 没有可读取的文本层，请改用 DOCX、YAML 或粘贴文本。")
        return text, None
    if suffix == ".docx":
        from docx import Document
        document = Document(str(path))
        text = "\n".join(paragraph.text for paragraph in document.paragraphs if paragraph.text.strip())
        if len(text.strip()) < 20:
            raise RuntimeError("DOCX 中没有足够的可读取文本。")
        return text, None
    if suffix in {".yaml", ".yml"}:
        import yaml
        data = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        return path.read_text(encoding="utf-8"), data
    raise RuntimeError("仅支持 PDF、DOCX、YAML 和 YML。")


def first_value(data: dict[str, Any], *keys: str) -> str:
    for key in keys:
        value = data.get(key)
        if value is not None:
            return str(value)
    return ""


def profile_from_yaml(data: dict[str, Any], file_name: str) -> dict[str, Any]:
    cv = data.get("cv") if isinstance(data.get("cv"), dict) else data
    sections = cv.get("sections") if isinstance(cv.get("sections"), dict) else {}
    experience_entries: list[dict[str, Any]] = []
    education_entries: list[dict[str, Any]] = []
    skills: list[str] = []
    summary = ""
    for section_name, entries in sections.items():
        name = str(section_name).lower()
        entries = entries if isinstance(entries, list) else []
        if any(word in name for word in ["education", "教育"]):
            for entry in entries:
                if isinstance(entry, dict):
                    education_entries.append({
                        "institution": first_value(entry, "institution"), "area": first_value(entry, "area"),
                        "degree": first_value(entry, "degree"), "startDate": first_value(entry, "start_date", "startDate"),
                        "endDate": first_value(entry, "end_date", "endDate", "date"), "highlights": [str(item) for item in entry.get("highlights", [])],
                    })
        elif any(word in name for word in ["experience", "工作", "职业经历"]):
            for entry in entries:
                if isinstance(entry, dict):
                    experience_entries.append({
                        "company": first_value(entry, "company", "institution"),
                        "position": first_value(entry, "position", "title"),
                        "location": first_value(entry, "location"),
                        "startDate": first_value(entry, "start_date", "startDate"),
                        "endDate": first_value(entry, "end_date", "endDate", "date"),
                        "highlights": [str(item) for item in entry.get("highlights", [])],
                    })
        elif "skill" in name or "技能" in name:
            for entry in entries:
                if isinstance(entry, str): skills.extend(re.split(r"[,，、|]", entry))
                elif isinstance(entry, dict): skills.extend(re.split(r"[,，、|]", first_value(entry, "details", "label")))
        elif any(word in name for word in ["summary", "profile", "简介"]):
            summary = " ".join(str(item) for item in entries)
    return base_profile(file_name, first_value(cv, "name"), first_value(cv, "email"), first_value(cv, "phone"), first_value(cv, "location"), first_value(cv, "website"), first_value(cv, "headline"), summary, skills, experience_entries, education_entries)


KNOWN_SKILLS = ["Python", "Java", "Golang", "Rust", "TypeScript", "JavaScript", "Svelte", "React", "Vue", "FastAPI", "Django", "Flask", "LangChain", "RAG", "Docker", "Kubernetes", "Redis", "MySQL", "PostgreSQL", "PyTorch", "TensorFlow", "AWS", "Azure"]


def profile_from_text(text: str, file_name: str) -> dict[str, Any]:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    email_match = re.search(r"[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}", text)
    phone_match = re.search(r"(?<!\d)(?:\+?86[- ]?)?1[3-9]\d(?:[- ]?\d){8}(?!\d)", text)
    name = next((line for line in lines[:8] if 1 < len(line) <= 12 and not re.search(r"[@\d:/]", line)), "")
    skills = [skill for skill in KNOWN_SKILLS if re.search(rf"(?<![A-Za-z]){re.escape(skill)}(?![A-Za-z])", text, re.IGNORECASE)]
    headline = next((line for line in lines[:12] if any(word in line.lower() for word in ["工程师", "开发", "产品", "designer", "engineer"])), "")
    summary = next((line for line in lines if 35 <= len(line) <= 180), "")
    return base_profile(file_name, name, email_match.group(0) if email_match else "", phone_match.group(0) if phone_match else "", "", "", headline, summary, skills, [], [])


def base_profile(file_name: str, name: str, email: str, phone: str, location: str, website: str, headline: str, summary: str, skills: list[str], experiences: list[dict[str, Any]], education: list[dict[str, Any]]) -> dict[str, Any]:
    skills = list(dict.fromkeys(item.strip() for item in skills if item.strip()))
    facts = [{"id": str(uuid.uuid4()), "category": "skill", "value": skill, "source": f"{file_name} · 技能", "confidence": 0.95, "confirmed": True} for skill in skills]
    return {
        "id": "resume-master", "name": name, "headline": headline, "email": email, "phone": phone,
        "location": location, "website": website, "summary": summary, "skills": skills,
        "experiences": experiences, "education": education, "facts": facts,
        "preferences": {"targetRoles": [], "cities": [], "remotePreference": "flexible", "energizingTasks": [], "drainingTasks": [], "hardConstraints": []},
        "sourceFileName": file_name, "updatedAt": now(), "version": 1,
    }


def extract_resume(params: dict[str, Any]) -> dict[str, Any]:
    path = pathlib.Path(str(params["path"]))
    file_name = str(params.get("fileName") or path.name)
    text, yaml_data = extract_text(path)
    profile = profile_from_yaml(yaml_data, file_name) if isinstance(yaml_data, dict) else profile_from_text(text, file_name)
    return {"profile": profile, "rawText": text}


def profile_to_rendercv(profile: dict[str, Any]) -> dict[str, Any]:
    sections: dict[str, Any] = {}
    if profile.get("summary"):
        sections["个人简介"] = [profile["summary"]]
    if profile.get("skills"):
        sections["核心技能"] = ["、".join(profile["skills"])]
    if profile.get("experiences"):
        sections["工作经历"] = [
            {
                "company": item.get("company", ""), "position": item.get("position", ""), "location": item.get("location", ""),
                "start_date": item.get("startDate") or None, "end_date": item.get("endDate") or None,
                "highlights": item.get("highlights", []),
            }
            for item in profile["experiences"]
        ]
    if profile.get("education"):
        sections["教育经历"] = [
            {
                "institution": item.get("institution", ""), "area": item.get("area", ""), "degree": item.get("degree", ""),
                "start_date": item.get("startDate") or None, "end_date": item.get("endDate") or None,
                "highlights": item.get("highlights", []),
            }
            for item in profile["education"]
        ]
    return {
        "cv": {
            "name": profile.get("name") or "Candidate", "headline": profile.get("headline") or None,
            "location": profile.get("location") or None, "email": profile.get("email") or None,
            "phone": profile.get("phone") or None, "website": profile.get("website") or None,
            "sections": sections,
        },
        "design": {"theme": "engineeringresumes"},
        "locale": {"language": "mandarin_chinese"},
    }


def render_resume(params: dict[str, Any]) -> dict[str, Any]:
    try:
        import yaml
        from rendercv.renderer import pdf_png
        from rendercv.renderer.typst import generate_typst
        from rendercv.schema.rendercv_model_builder import build_rendercv_dictionary_and_model
    except ImportError as error:
        raise RuntimeError("RenderCV 运行时未安装，请使用生产 sidecar 或安装 sidecar 依赖。") from error

    output_path = pathlib.Path(str(params["outputPath"])).resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    yaml_path = output_path.with_suffix(".yaml")
    typst_path = output_path.with_suffix(".typ")
    data = profile_to_rendercv(dict(params["profile"]))
    yaml_text = yaml.safe_dump(data, allow_unicode=True, sort_keys=False)
    yaml_path.write_text(yaml_text, encoding="utf-8")
    _, model = build_rendercv_dictionary_and_model(
        yaml_text,
        input_file_path=yaml_path,
        output_folder=output_path.parent,
        typst_path=typst_path,
        pdf_path=output_path,
        dont_generate_png=True,
        dont_generate_markdown=True,
        dont_generate_html=True,
    )
    # RenderCV 2.8 imports the Font Awesome Typst package even when the selected
    # theme disables all icons. Replace that unused network import with a local
    # no-op implementation so resume rendering remains fully offline.
    package_path = pdf_png.get_package_path()
    for library in package_path.glob("preview/rendercv/*/lib.typ"):
        source = library.read_text(encoding="utf-8")
        source = source.replace(
            '#import "@preview/fontawesome:0.6.0": fa-icon',
            '#let fa-icon(name, size: 1em) = none',
        )
        library.write_text(source, encoding="utf-8")
    pdf_png.get_typst_compiler.cache_clear()

    generated_typst = generate_typst(model)
    generated_pdf = pdf_png.generate_pdf(model, generated_typst)
    if generated_pdf is None or not pathlib.Path(generated_pdf).exists():
        raise RuntimeError("RenderCV 没有生成 PDF，请检查简历字段。")
    return {"path": str(pathlib.Path(generated_pdf).resolve()), "yamlPath": str(yaml_path)}


OPERATIONS = {
    "setup_boss": setup_boss,
    "scrape_jobs": scrape_jobs,
    "extract_resume": extract_resume,
    "render_resume": render_resume,
    "ping": lambda params: {"python": sys.version, "ok": True},
}


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
        traceback.print_exc(file=sys.stderr)
        emit("result", ok=False, error=str(error))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
