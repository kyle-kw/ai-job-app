from __future__ import annotations

import os
import pathlib
import tempfile
from typing import Any

try:
    from resume_parser import display_degree, render_date_fields, rendercv_phone
except ImportError:
    from sidecar.resume_parser import display_degree, render_date_fields, rendercv_phone

RESUME_COLOR_THEMES = {
    "pine": {"accent": "#176B57", "links": "#0B7A67"},
    "navy": {"accent": "#1F407A", "links": "#005CB8"},
    "graphite": {"accent": "#24292F", "links": "#24292F"},
}

RESUME_BOLD_KEYWORDS = [
    "Dify",
    "FastAPI",
    "Docker",
    "Docker Compose",
    "PostgreSQL",
    "vLLM",
    "SGLang",
    "llama.cpp",
    "MinerU",
    "Milvus",
    "OpenAI",
    "Linux",
    "Prometheus",
    "Grafana",
    "Triton",
    "PP-OCRv6",
    "PP-StructureV3",
    "PaddleOCR-VL-1.6",
]


def resume_design(color_theme: str) -> dict[str, Any]:
    colors = RESUME_COLOR_THEMES.get(color_theme)
    if colors is None:
        raise ValueError("不支持的简历颜色主题。")
    accent = colors["accent"]
    return {
        "theme": "classic",
        "page": {
            "size": "a4",
            "top_margin": "1.2cm",
            "bottom_margin": "1.2cm",
            "left_margin": "1.35cm",
            "right_margin": "1.35cm",
            "show_footer": False,
            "show_top_note": False,
        },
        "colors": {
            "name": accent,
            "headline": accent,
            "connections": accent,
            "section_titles": accent,
            "links": colors["links"],
        },
        "typography": {
            "line_spacing": "0.72em",
            "alignment": "left",
            "font_family": {
                "body": "Microsoft YaHei",
                "name": "Microsoft YaHei",
                "headline": "Microsoft YaHei",
                "connections": "Microsoft YaHei",
                "section_titles": "Microsoft YaHei",
            },
            "font_size": {
                "body": "10.2pt",
                "name": "25pt",
                "headline": "10.6pt",
                "connections": "9.8pt",
                "section_titles": "1.28em",
            },
        },
        "header": {
            "alignment": "center",
            "space_below_name": "0.22cm",
            "space_below_headline": "0.24cm",
            "space_below_connections": "0.32cm",
            "connections": {
                "phone_number_format": "international",
                "show_icons": False,
                "separator": "|",
                "space_between_connections": "0.32cm",
            },
        },
        "section_titles": {
            "type": "with_full_line",
            "space_above": "0.42cm",
            "space_below": "0.22cm",
        },
        "sections": {
            "space_between_regular_entries": "0.42cm",
            "space_between_text_based_entries": "0.14cm",
            "show_time_spans_in": [],
        },
        "entries": {
            "date_and_location_width": "4.6cm",
            "side_space": "0cm",
            "space_between_columns": "0.24cm",
            "allow_page_break": False,
            "short_second_row": False,
            "summary": {"space_above": "0.06cm"},
            "highlights": {
                "space_left": "0.05cm",
                "space_above": "0.08cm",
                "space_between_items": "0.06cm",
                "space_between_bullet_and_text": "0.32em",
            },
        },
        "templates": {
            "experience_entry": {
                "main_column": "**COMPANY**, POSITION\nSUMMARY\nHIGHLIGHTS",
                "date_and_location_column": "LOCATION · DATE",
            },
            "education_entry": {
                "main_column": "**INSTITUTION**, AREA\nSUMMARY\nHIGHLIGHTS",
                "degree_column": "**DEGREE**",
                "date_and_location_column": "LOCATION · DATE",
            },
        },
    }


def profile_to_rendercv(profile: dict[str, Any], color_theme: str = "navy") -> dict[str, Any]:
    section_values: dict[str, tuple[str, Any]] = {}
    if profile.get("summary"):
        section_values["summary"] = ("个人简介", [profile["summary"]])
    skill_groups = profile.get("professionalSkills") or []
    if not skill_groups and profile.get("skills"):
        skill_groups = [{"label": "核心技能", "items": profile["skills"]}]
    rendered_skill_groups = []
    for group in skill_groups:
        items = [str(item).strip() for item in group.get("items") or [] if str(item).strip()]
        if items:
            rendered_skill_groups.append(
                {"label": group.get("label") or "专业技能", "details": ", ".join(items)}
            )
    if rendered_skill_groups:
        section_values["professionalSkills"] = ("专业技能", rendered_skill_groups)
    if profile.get("experiences"):
        experience_entries = []
        for item in profile["experiences"]:
            start_date, end_date = render_date_fields(item)
            experience_entries.append(
                {
                    "company": item.get("company", ""),
                    "position": item.get("position", ""),
                    "location": item.get("location", ""),
                    "start_date": start_date,
                    "end_date": end_date,
                    "highlights": [
                        value for value in item.get("highlights", []) if str(value).strip()
                    ],
                }
            )
        section_values["experiences"] = ("工作经历", experience_entries)
    if profile.get("projects"):
        project_entries = []
        for item in profile["projects"]:
            start_date, end_date = render_date_fields(item)
            project_entries.append(
                {
                    "name": item.get("name", ""),
                    "summary": item.get("summary", ""),
                    "start_date": start_date,
                    "end_date": end_date,
                    "highlights": [
                        value for value in item.get("highlights", []) if str(value).strip()
                    ],
                }
            )
        section_values["projects"] = ("项目经历", project_entries)
    if profile.get("certifications"):
        section_values["certifications"] = (
            "证书 / 专业资质",
            [
                " · ".join(
                    part
                    for part in [item.get("name", ""), item.get("issuer", ""), item.get("date", "")]
                    if part
                )
                for item in profile["certifications"]
            ],
        )
    if profile.get("education"):
        education_entries = []
        for item in profile["education"]:
            start_date, end_date = render_date_fields(item)
            education_entries.append(
                {
                    "institution": item.get("institution", ""),
                    "area": item.get("area", ""),
                    "degree": display_degree(item),
                    "start_date": start_date,
                    "end_date": end_date,
                    "highlights": [
                        value for value in item.get("highlights", []) if str(value).strip()
                    ],
                }
            )
        section_values["education"] = ("教育经历", education_entries)
    orders = {
        "ai-engineering": [
            "summary",
            "professionalSkills",
            "projects",
            "experiences",
            "certifications",
            "education",
        ],
        "data-analysis": [
            "summary",
            "professionalSkills",
            "experiences",
            "projects",
            "certifications",
            "education",
        ],
        "finance-accounting": [
            "summary",
            "experiences",
            "certifications",
            "professionalSkills",
            "education",
            "projects",
        ],
        "general": [
            "summary",
            "experiences",
            "professionalSkills",
            "projects",
            "certifications",
            "education",
        ],
    }
    sections: dict[str, Any] = {}
    for key in orders.get(
        str(profile.get("templateId") or "ai-engineering"), orders["ai-engineering"]
    ):
        if key in section_values and section_values[key][1]:
            title, entries = section_values[key]
            sections[title] = entries
    return {
        "cv": {
            "name": profile.get("name") or "Candidate",
            "headline": profile.get("headline") or None,
            "location": profile.get("location") or None,
            "email": profile.get("email") or None,
            "phone": rendercv_phone(profile.get("phone")),
            "website": profile.get("website") or None,
            "sections": sections,
        },
        "design": resume_design(color_theme),
        "locale": {"language": "mandarin_chinese"},
        "settings": {"bold_keywords": RESUME_BOLD_KEYWORDS},
    }


def render_resume(params: dict[str, Any]) -> dict[str, Any]:
    try:
        import yaml
        from rendercv.renderer import pdf_png
        from rendercv.renderer.typst import generate_typst
        from rendercv.schema.rendercv_model_builder import build_rendercv_dictionary_and_model
    except ImportError as error:
        raise RuntimeError(
            "RenderCV 运行时未安装，请使用生产 sidecar 或安装 sidecar 依赖。"
        ) from error

    output_path = pathlib.Path(str(params["outputPath"])).resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    data = profile_to_rendercv(dict(params["profile"]), str(params.get("colorTheme") or "navy"))
    yaml_text = yaml.safe_dump(data, allow_unicode=True, sort_keys=False)
    with tempfile.TemporaryDirectory(prefix="resume-render-") as temporary:
        temporary_path = pathlib.Path(temporary)
        yaml_path = temporary_path / "resume.yaml"
        typst_path = temporary_path / "resume.typ"
        yaml_path.write_text(yaml_text, encoding="utf-8")
        _, model = build_rendercv_dictionary_and_model(
            yaml_text,
            input_file_path=yaml_path,
            output_folder=temporary_path,
            typst_path=typst_path,
            pdf_path=output_path,
            dont_generate_png=True,
            dont_generate_markdown=True,
            dont_generate_html=True,
        )
        # RenderCV 2.8 imports Font Awesome even when a theme disables icons.
        # get_package_path() returns a process-private temporary copy. Patch that
        # copy atomically so the installed RenderCV package is never modified.
        package_path = pdf_png.get_package_path()
        for library in package_path.glob("preview/rendercv/*/lib.typ"):
            source = library.read_text(encoding="utf-8")
            source = source.replace(
                '#import "@preview/fontawesome:0.6.0": fa-icon',
                "#let fa-icon(name, size: 1em) = none",
            )
            replacement = library.with_suffix(".typ.tmp")
            replacement.write_text(source, encoding="utf-8")
            os.replace(replacement, library)
        pdf_png.get_typst_compiler.cache_clear()

        generated_typst = generate_typst(model)
        generated_pdf = pdf_png.generate_pdf(model, generated_typst)
        if generated_pdf is None or not pathlib.Path(generated_pdf).exists():
            raise RuntimeError("RenderCV 没有生成 PDF，请检查简历字段。")
    return {"path": str(output_path)}
