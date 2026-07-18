use crate::analytics;
use crate::distribution;
use crate::llm;
use crate::models::*;
use crate::scoring;
use crate::secrets::redact;
use crate::sidecar;
use crate::skills;
use crate::time;
use crate::AppState;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

const BOSS_CITY_CODES_JSON: &str = include_str!("../../sidecar/vendor/city_codes.json");
static SUPPORTED_SCRAPE_CITIES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let cities = serde_json::from_str::<HashMap<String, String>>(BOSS_CITY_CODES_JSON)
        .expect("bundled BOSS city map must be valid JSON");
    assert!(cities.len() >= 100, "bundled BOSS city map is incomplete");
    cities.into_keys().collect()
});

fn is_supported_scrape_city(city: &str) -> bool {
    SUPPORTED_SCRAPE_CITIES.contains(city)
}

async fn ensure_chrome_available() -> Result<(), String> {
    let environment = sidecar::request(json!({"op":"environment_status","params":{}})).await?;
    if environment
        .get("chrome")
        .and_then(|value| value.get("installed"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("chrome_missing: BOSS 功能需要 Google Chrome，请从 https://www.google.com/chrome/ 安装后重试".into());
    }
    Ok(())
}

const MAX_RESUME_FILE_BYTES: usize = 25 * 1024 * 1024;
const MAX_RESUME_BASE64_BYTES: usize = MAX_RESUME_FILE_BYTES.div_ceil(3) * 4;

struct ImportArtifacts {
    input_path: PathBuf,
    image_dir: PathBuf,
}

impl ImportArtifacts {
    fn new(input_path: PathBuf) -> Self {
        let stem = input_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let image_dir = input_path.with_file_name(format!("{stem}-pages"));
        Self {
            input_path,
            image_dir,
        }
    }
}

impl Drop for ImportArtifacts {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.input_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!("failed to remove resume import file: {error}"),
        }
        match std::fs::remove_dir_all(&self.image_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!("failed to remove resume import images: {error}"),
        }
    }
}

fn validate_resume_import_size(
    encoded_bytes: usize,
    decoded_bytes: Option<usize>,
) -> Result<(), String> {
    if encoded_bytes > MAX_RESUME_BASE64_BYTES
        || decoded_bytes.is_some_and(|size| size > MAX_RESUME_FILE_BYTES)
    {
        Err("简历文件不能超过 25 MiB。".into())
    } else {
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResumeExtractionOutput {
    profile: ResumeProfile,
    #[serde(default)]
    raw_text: String,
    #[serde(default)]
    pages: Vec<ResumeExtractionPage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResumeExtractionPage {
    page_number: usize,
    #[serde(default)]
    text: String,
    #[serde(default)]
    image_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GreetingOutput {
    text: String,
}

mod jobs;
mod resumes;
mod scrape;
mod settings;

pub use jobs::*;
pub use resumes::*;
pub use scrape::*;
pub use settings::*;

#[cfg(test)]
use jobs::{serialize_jobs_json, validate_export_path};
fn new_task(kind: &str, title: &str) -> TaskRun {
    let now = time::shanghai_rfc3339();
    TaskRun {
        id: Uuid::new_v4().to_string(),
        kind: kind.into(),
        title: title.into(),
        state: "queued".into(),
        progress: 0,
        message: "等待开始".into(),
        recoverable_error: None,
        created_at: now.clone(),
        updated_at: now,
        logs: vec![],
    }
}

fn update_task(
    app: &AppHandle,
    db: &crate::db::Database,
    task: &mut TaskRun,
    state: &str,
    progress: i64,
    message: &str,
    error: Option<String>,
) {
    task.state = state.into();
    task.progress = progress;
    task.message = message.into();
    task.recoverable_error = error.map(|value| redact(&value));
    task.updated_at = time::shanghai_rfc3339();
    task.logs
        .push(format!("[{}] {}", time::shanghai_clock(), redact(message)));
    let _ = db.save_task(task);
    emit_task(app, task);
}

fn emit_task(app: &AppHandle, task: &TaskRun) {
    let _ = app.emit("task://event", task);
}

fn facts_from_profile(profile: &ResumeProfile) -> Vec<ResumeFact> {
    let mut facts = vec![];
    for skill in profile.flattened_skills() {
        facts.push(ResumeFact {
            id: Uuid::new_v4().to_string(),
            category: "skill".into(),
            value: skill,
            source: format!("{} · 专业技能", profile.source_file_name),
            confidence: 0.95,
            confirmed: false,
        });
    }
    for (experience_index, experience) in profile.experiences.iter().enumerate() {
        let role = [experience.company.trim(), experience.position.trim()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let dates = [experience.start_date.trim(), experience.end_date.trim()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("—");
        if !role.is_empty() {
            facts.push(ResumeFact {
                id: Uuid::new_v4().to_string(),
                category: "experience".into(),
                value: if dates.is_empty() {
                    role
                } else {
                    format!("{role}（{dates}）")
                },
                source: format!("工作经历 {} · {}", experience_index + 1, experience.company),
                confidence: 0.95,
                confirmed: false,
            });
        }
        for highlight in &experience.highlights {
            if highlight.trim().is_empty() {
                continue;
            }
            facts.push(ResumeFact {
                id: Uuid::new_v4().to_string(),
                category: "experience".into(),
                value: highlight.clone(),
                source: format!("工作经历 {} · {}", experience_index + 1, experience.company),
                confidence: 0.9,
                confirmed: false,
            });
        }
    }
    for education in &profile.education {
        let dates = [education.start_date.trim(), education.end_date.trim()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("—");
        let degree = if education.degree == "其他" && !education.degree_detail.trim().is_empty() {
            education.degree_detail.trim()
        } else {
            education.degree.trim()
        };
        let mut values = [education.institution.trim(), education.area.trim(), degree]
            .into_iter()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !dates.is_empty() {
            values.push(dates);
        }
        if !values.is_empty() {
            facts.push(ResumeFact {
                id: Uuid::new_v4().to_string(),
                category: "education".into(),
                value: values.join(" · "),
                source: format!("{} · 教育经历", profile.source_file_name),
                confidence: 0.95,
                confirmed: false,
            });
        }
    }
    for (project_index, project) in profile.projects.iter().enumerate() {
        let values = std::iter::once(project.summary.as_str())
            .chain(project.highlights.iter().map(String::as_str))
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>();
        let values = if values.is_empty() && !project.name.trim().is_empty() {
            vec![project.name.as_str()]
        } else {
            values
        };
        for value in values {
            facts.push(ResumeFact {
                id: Uuid::new_v4().to_string(),
                category: "project".into(),
                value: value.to_string(),
                source: format!("项目经历 {} · {}", project_index + 1, project.name),
                confidence: 0.9,
                confirmed: false,
            });
        }
    }
    for certification in &profile.certifications {
        if certification.name.trim().is_empty() {
            continue;
        }
        let detail = [certification.issuer.trim(), certification.date.trim()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        facts.push(ResumeFact {
            id: Uuid::new_v4().to_string(),
            category: "certification".into(),
            value: if detail.is_empty() {
                certification.name.clone()
            } else {
                format!("{} · {detail}", certification.name)
            },
            source: format!("{} · 证书资质", profile.source_file_name),
            confidence: 0.95,
            confirmed: false,
        });
    }
    facts
}

fn merge_missing_profile_facts(profile: &mut ResumeProfile) {
    let mut seen = profile
        .facts
        .iter()
        .map(|fact| {
            format!(
                "{}\u{0}{}",
                fact.category,
                normalize_fact_value(&fact.value)
            )
        })
        .collect::<HashSet<_>>();
    for fact in facts_from_profile(profile) {
        let key = format!(
            "{}\u{0}{}",
            fact.category,
            normalize_fact_value(&fact.value)
        );
        if seen.insert(key) {
            profile.facts.push(fact);
        }
    }
}

fn normalize_fact_value(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrape_city_map_accepts_full_offline_snapshot() {
        assert!(SUPPORTED_SCRAPE_CITIES.len() >= 300);
        assert!(is_supported_scrape_city("上海"));
        assert!(is_supported_scrape_city("赣州"));
        assert!(is_supported_scrape_city("全国"));
        assert!(!is_supported_scrape_city("不存在城市"));
    }

    #[test]
    fn resume_import_size_limit_checks_encoded_and_decoded_payloads() {
        assert!(
            validate_resume_import_size(MAX_RESUME_BASE64_BYTES, Some(MAX_RESUME_FILE_BYTES))
                .is_ok()
        );
        assert!(validate_resume_import_size(MAX_RESUME_BASE64_BYTES + 1, None).is_err());
        assert!(validate_resume_import_size(4, Some(MAX_RESUME_FILE_BYTES + 1)).is_err());
    }

    #[test]
    fn import_artifact_guard_removes_source_and_rendered_pages() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("resume.pdf");
        let pages = directory.path().join("resume-pages");
        std::fs::write(&input, b"resume").unwrap();
        std::fs::create_dir(&pages).unwrap();
        std::fs::write(pages.join("page-1.png"), b"image").unwrap();
        drop(ImportArtifacts::new(input.clone()));
        assert!(!input.exists());
        assert!(!pages.exists());
    }

    #[test]
    fn job_json_export_is_pretty_utf8_camel_case_and_validates_extensions() {
        let job: Job = serde_json::from_value(json!({
            "id":"job-1","source":"boss","externalId":"external-1","title":"AI 工程师",
            "company":"示例公司","salary":"20-30K","location":"上海·浦东新区",
            "experience":"3-5年","degree":"本科","companyScale":"100-499人",
            "companyStage":"B轮","industry":"人工智能","skills":["Python"],"welfare":[],
            "description":"负责 AI 平台研发","sourceUrl":"https://example.com/job",
            "firstSeen":"2026-01-01","lastSeen":"2026-01-02","isNew":true
        }))
        .unwrap();
        let bytes = serialize_jobs_json(&[job]).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("[\n"));
        assert!(text.contains("AI 工程师"));
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value[0]["externalId"], "external-1");
        assert!(value[0].get("external_id").is_none());

        let directory = tempfile::tempdir().unwrap();
        assert!(validate_export_path(
            directory
                .path()
                .join("jobs.JSON")
                .to_string_lossy()
                .to_string(),
            "json",
            "岗位 JSON"
        )
        .is_ok());
        assert!(validate_export_path(
            directory
                .path()
                .join("jobs.html")
                .to_string_lossy()
                .to_string(),
            "json",
            "岗位 JSON"
        )
        .is_err());
    }

    #[test]
    fn profile_fact_merge_covers_data_and_finance_sections_without_duplicates() {
        let mut profile: ResumeProfile = serde_json::from_value(json!({
            "id":"resume","name":"","headline":"财务分析师","email":"","phone":"","location":"","website":"","summary":"",
            "templateId":"finance-accounting",
            "professionalSkills":[{"id":"skills","label":"财务系统与办公工具","items":["Excel"]}],
            "experiences":[{"id":"exp","company":"示例公司","position":"财务会计","location":"上海","startDate":"2022.01","endDate":"至今","highlights":["月结周期缩短至 4 天"]}],
            "education":[{"id":"edu","institution":"示例大学","area":"会计学","degree":"本科","startDate":"2018.09","endDate":"2022.06","highlights":[]}],
            "projects":[{"id":"project","name":"预算分析","summary":"建立预算差异分析","startDate":"","endDate":"","highlights":[]}],
            "certifications":[{"id":"cert","name":"初级会计资格","issuer":"示例机构","date":"2022.09"}],
            "facts":[{"id":"existing","category":"skill","value":"excel","source":"导入","confidence":0.99,"confirmed":true}],
            "preferences":{"targetRoles":[],"cities":[],"remotePreference":"flexible","energizingTasks":[],"drainingTasks":[],"hardConstraints":[]},
            "sourceFileName":"resume.pdf","updatedAt":"","version":1
        })).unwrap();

        merge_missing_profile_facts(&mut profile);

        assert_eq!(
            profile
                .facts
                .iter()
                .filter(|fact| fact.category == "skill")
                .count(),
            1
        );
        assert!(profile
            .facts
            .iter()
            .any(|fact| fact.category == "experience" && fact.value.contains("财务会计")));
        assert!(profile
            .facts
            .iter()
            .any(|fact| fact.category == "experience" && fact.value.contains("月结")));
        assert!(profile
            .facts
            .iter()
            .any(|fact| fact.category == "education" && fact.value.contains("会计学")));
        assert!(profile
            .facts
            .iter()
            .any(|fact| fact.category == "project" && fact.value.contains("预算差异")));
        assert!(profile
            .facts
            .iter()
            .any(|fact| fact.category == "certification" && fact.value.contains("初级会计资格")));
        assert!(
            profile
                .facts
                .iter()
                .find(|fact| fact.id == "existing")
                .unwrap()
                .confirmed
        );
    }
}
