use crate::analytics;
use crate::db::{
    ensure_resume_item_ids, Database, InterviewPreparationCacheRecord,
    ReportCompetitivenessCacheRecord,
};
use crate::distribution;
use crate::llm;
use crate::models::*;
use crate::scoring;
use crate::secrets::redact;
use crate::skills;
use crate::time;
use crate::AppState;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

const FIT_SKILL_VERSION: &str = "job-fit@1.1.0";
const INTERVIEW_SKILL_VERSION: &str = "interview-preparation@1.0.0";
const RESUME_COVERAGE_SKILL_VERSION: &str = "resume-coverage@1.1.0";
const REPORT_COMPETITIVENESS_SKILL_VERSION: &str = "report-competitiveness@1.0.0";
const MAX_DESCRIPTION_COVERAGE_REQUIREMENTS: usize = 20;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelInterviewPreparation {
    summary: String,
    #[serde(default)]
    skills: Vec<ModelInterviewSkill>,
    #[serde(default)]
    project_ideas: Vec<String>,
    #[serde(default)]
    practice_questions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelInterviewSkill {
    name: String,
    #[serde(default)]
    gap: Option<String>,
    action: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CoverageRequirement {
    id: String,
    label: String,
    kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelResumeCoverageOutput {
    #[serde(default)]
    items: Vec<ModelResumeCoverageItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelResumeCoverageItem {
    id: String,
    status: String,
    #[serde(default)]
    resume_paths: Vec<String>,
    #[serde(default)]
    evidence_fact_ids: Vec<String>,
    #[serde(default)]
    rationale: String,
}

pub(crate) fn mark_fit_cache_status(
    jobs: &mut [Job],
    resume: Option<&ResumeProfile>,
    provider: Option<&AiProviderConfig>,
) {
    for job in jobs {
        let expected_hash = resume.map(|resume| fit_input_hash(job, resume, provider));
        let Some(fit) = job.fit.as_mut() else {
            continue;
        };
        if resume.is_none() {
            fit.cache_status = "stale".into();
            continue;
        }
        if fit.input_hash.is_empty() {
            fit.cache_status = "legacy".into();
            continue;
        }
        fit.cache_status = if expected_hash.as_deref() == Some(fit.input_hash.as_str()) {
            "fresh"
        } else {
            "stale"
        }
        .into();
    }
}

mod competitiveness;
mod coverage;
mod fit;
mod interview;
mod resume_chat;
mod settings;

pub use competitiveness::*;
pub use coverage::*;
pub use fit::*;
pub use interview::*;
pub use resume_chat::*;
pub use settings::*;

#[cfg(test)]
use competitiveness::validate_model_report_competitiveness;
#[cfg(test)]
use resume_chat::{
    resolve_resume_market_context, resume_edit_introduces_factual_content,
    validate_market_edit_evidence, validate_resume_chat_context_mode,
};
fn allowed_resume_paths() -> &'static [&'static str] {
    &[
        "/name",
        "/headline",
        "/email",
        "/phone",
        "/location",
        "/website",
        "/summary",
        "/templateId",
        "/professionalSkills",
        "/experiences",
        "/education",
        "/projects",
        "/certifications",
    ]
}

fn resume_path_label(path: &str) -> Option<&'static str> {
    Some(match path {
        "/name" => "姓名",
        "/headline" => "职业标题",
        "/email" => "邮箱",
        "/phone" => "电话",
        "/location" => "所在地",
        "/website" => "个人主页",
        "/summary" => "个人简介",
        "/templateId" => "简历结构模板",
        "/professionalSkills" => "专业技能",
        "/experiences" => "工作经历",
        "/education" => "教育经历",
        "/projects" => "项目经历",
        "/certifications" => "证书 / 专业资质",
        _ => return None,
    })
}

fn validate_resume_after(path: &str, after: &Value) -> Result<(), String> {
    match path {
        "/name" | "/headline" | "/email" | "/phone" | "/location" | "/website" | "/summary"
            if after.is_string() =>
        {
            Ok(())
        }
        "/templateId" => match after.as_str() {
            Some("general" | "ai-engineering" | "data-analysis" | "finance-accounting") => Ok(()),
            _ => Err("unsafe_proposal: 简历模板无效。".into()),
        },
        "/professionalSkills" => {
            serde_json::from_value::<Vec<ProfessionalSkillGroup>>(after.clone())
                .map(|_| ())
                .map_err(|_| "unsafe_proposal: 专业技能分组结构无效。".into())
        }
        "/experiences" => serde_json::from_value::<Vec<ResumeExperience>>(after.clone())
            .map(|_| ())
            .map_err(|_| "unsafe_proposal: 工作经历结构无效。".into()),
        "/education" => serde_json::from_value::<Vec<ResumeEducation>>(after.clone())
            .map(|_| ())
            .map_err(|_| "unsafe_proposal: 教育经历结构无效。".into()),
        "/projects" => serde_json::from_value::<Vec<ResumeProject>>(after.clone())
            .map(|_| ())
            .map_err(|_| "unsafe_proposal: 项目经历结构无效。".into()),
        "/certifications" => serde_json::from_value::<Vec<ResumeCertification>>(after.clone())
            .map(|_| ())
            .map_err(|_| "unsafe_proposal: 证书资质结构无效。".into()),
        _ => Err("unsafe_proposal: 修改字段或类型不受支持。".into()),
    }
}

fn validate_chat_messages(messages: &[ResumeChatMessage]) -> Result<(), String> {
    if messages.is_empty() || messages.len() > 20 {
        return Err("invalid_request: 对话应包含 1–20 条消息。".into());
    }
    let total: usize = messages
        .iter()
        .map(|message| message.content.chars().count())
        .sum();
    if total > 20_000
        || messages.iter().any(|message| {
            message.content.trim().is_empty()
                || message.content.chars().count() > 2_000
                || !matches!(message.role.as_str(), "user" | "assistant")
        })
    {
        return Err("invalid_request: 消息为空或超过长度限制。".into());
    }
    Ok(())
}

fn allowed_fact_category(category: &str) -> bool {
    matches!(
        category,
        "identity" | "experience" | "education" | "skill" | "project" | "certification" | "other"
    )
}

fn validate_numeric_claims(
    before: &Value,
    after: &Value,
    confirmed_text: &str,
    candidate_text: &str,
) -> Result<(), String> {
    let before_text = before.to_string();
    let after_text = after.to_string();
    let supported = format!("{before_text} {confirmed_text} {candidate_text}");
    if number_tokens(&after_text)
        .into_iter()
        .any(|token| !supported.contains(&token))
    {
        return Err("unsafe_proposal: 修改包含没有事实依据的新数字。".into());
    }
    Ok(())
}

fn validate_new_skills(
    path: &str,
    before: &Value,
    after: &Value,
    resume: &ResumeProfile,
    candidates: &[ResumeFactCandidate],
) -> Result<(), String> {
    if path != "/professionalSkills" {
        return Ok(());
    }
    let before = serde_json::from_value::<Vec<ProfessionalSkillGroup>>(before.clone())
        .unwrap_or_default()
        .into_iter()
        .flat_map(|group| group.items)
        .collect::<Vec<_>>();
    let after = serde_json::from_value::<Vec<ProfessionalSkillGroup>>(after.clone())
        .map_err(|_| "unsafe_proposal: 专业技能分组结构无效。".to_string())?
        .into_iter()
        .flat_map(|group| group.items)
        .collect::<Vec<_>>();
    let supported = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed && fact.category == "skill")
        .map(|fact| fact.value.to_lowercase())
        .chain(
            candidates
                .iter()
                .filter(|fact| fact.category == "skill")
                .map(|fact| fact.value.to_lowercase()),
        )
        .collect::<Vec<_>>()
        .join(" ");
    for skill in after {
        if !before.iter().any(|item| item.eq_ignore_ascii_case(&skill))
            && !supported.contains(&skill.to_lowercase())
        {
            return Err(format!(
                "unsafe_proposal: 新技能“{skill}”没有已确认事实依据。"
            ));
        }
    }
    Ok(())
}

fn number_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_ascii_digit() || (character == '.' && !current.is_empty()) {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn fit_report_uses_chinese(report: &FitReport) -> bool {
    fn contains_han(value: &str) -> bool {
        value
            .chars()
            .any(|character| matches!(character as u32, 0x3400..=0x9fff | 0xf900..=0xfaff))
    }

    contains_han(&report.summary)
        && contains_han(&report.recommendation)
        && !report.dimensions.is_empty()
        && report
            .dimensions
            .iter()
            .all(|dimension| contains_han(&dimension.label) && contains_han(&dimension.note))
        && report
            .hard_constraints
            .iter()
            .all(|constraint| contains_han(&constraint.label) && contains_han(&constraint.note))
        && report.strengths.iter().all(|item| contains_han(item))
        && report.gaps.iter().all(|item| contains_han(item))
}

fn fit_input_hash(
    job: &Job,
    resume: &ResumeProfile,
    provider: Option<&AiProviderConfig>,
) -> String {
    let mut skills = job.skills.clone();
    skills.sort_by_key(|value| value.to_lowercase());
    skills.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    hash_json(&json!({
        "job": {
            "title": job.title,
            "salary": job.salary,
            "location": job.location,
            "experience": job.experience,
            "degree": job.degree,
            "companyScale": job.company_scale,
            "industry": job.industry,
            "skills": skills,
            "description": job.description,
            "requirements": job.structured_details.as_ref().map(|value| &value.requirements)
        },
        "resumeId": resume.id,
        "resumeVersion": resume.version,
        "preferences": resume.preferences,
        "provider": provider.map(|value| json!({"id":value.id,"baseUrl":value.base_url,"model":value.model})).unwrap_or_else(|| json!("local-fit-v1")),
        "skillVersion": FIT_SKILL_VERSION
    }))
}

fn dataset_hash(jobs: &[Job]) -> String {
    let mut values = jobs
        .iter()
        .map(|job| {
            let mut skills = job.skills.clone();
            skills.sort_by_key(|value| value.to_lowercase());
            json!({
                "id":job.id,"title":job.title,"company":job.company,"salary":job.salary,
                "location":job.location,"experience":job.experience,"degree":job.degree,
                "companyScale":job.company_scale,"industry":job.industry,"skills":skills,
                "description":job.description,
                "requirements":job.structured_details.as_ref().map(|value| &value.requirements)
            })
        })
        .collect::<Vec<_>>();
    values.sort_by_key(|value| value["id"].as_str().unwrap_or_default().to_string());
    hash_json(&json!(values))
}

fn coverage_requirements(job: &Job) -> Vec<CoverageRequirement> {
    let mut values = Vec::<(String, String)>::new();
    if let Some(details) = &job.structured_details {
        for requirement in &details.requirements {
            values.push((requirement.clone(), "requirement".into()));
        }
    }
    for skill in &job.skills {
        values.push((skill.clone(), "skill".into()));
    }
    if job
        .structured_details
        .as_ref()
        .is_none_or(|details| details.requirements.is_empty())
    {
        let mut description_count = 0;
        for sentence in job.description.split(['。', '；', ';', '\n']) {
            let sentence = sentence.trim();
            if (6..=140).contains(&sentence.chars().count()) {
                values.push((sentence.into(), "requirement".into()));
                description_count += 1;
                if description_count >= MAX_DESCRIPTION_COVERAGE_REQUIREMENTS {
                    break;
                }
            }
        }
    }
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|(label, kind)| {
            let normalized = normalize_coverage_text(&label);
            if normalized.is_empty() || !seen.insert(normalized.clone()) {
                return None;
            }
            Some(CoverageRequirement {
                id: coverage_requirement_id(&normalized),
                label,
                kind,
            })
        })
        .collect()
}

fn coverage_requirement_id(normalized: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in normalized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("requirement-{hash:016x}")
}

fn normalize_coverage_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace() && !"，,。；;：:、·|/\\()[]（）【】_-".contains(*character)
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn coverage_resume_paths(resume: &ResumeProfile) -> Vec<String> {
    let mut paths = vec!["/headline".into(), "/summary".into()];
    paths.extend(
        (0..resume.professional_skills.len()).map(|index| format!("/professionalSkills/{index}")),
    );
    paths.extend((0..resume.experiences.len()).map(|index| format!("/experiences/{index}")));
    paths.extend((0..resume.projects.len()).map(|index| format!("/projects/{index}")));
    paths.extend((0..resume.education.len()).map(|index| format!("/education/{index}")));
    paths.extend((0..resume.certifications.len()).map(|index| format!("/certifications/{index}")));
    paths
}

fn coverage_job_fingerprint(job: &Job) -> Result<String, String> {
    let payload = serde_json::to_vec(&json!({
        "id": job.id, "skills": job.skills, "description": job.description, "structuredDetails": job.structured_details
    })).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(payload)))
}

fn summarize_resume_coverage(mut report: ResumeCoverageReport) -> ResumeCoverageReport {
    report.covered_count = report
        .items
        .iter()
        .filter(|item| item.status == "covered")
        .count() as i64;
    report.strengthenable_count = report
        .items
        .iter()
        .filter(|item| item.status == "strengthenable")
        .count() as i64;
    report.gap_count = report
        .items
        .iter()
        .filter(|item| item.status == "gap")
        .count() as i64;
    report.unknown_count = report
        .items
        .iter()
        .filter(|item| item.status == "unknown")
        .count() as i64;
    report
}

fn validate_model_coverage_report(
    job_id: String,
    target: ResumeTargetRef,
    target_version: i64,
    resume: &ResumeProfile,
    requirements: &[CoverageRequirement],
    output: ModelResumeCoverageOutput,
) -> ResumeCoverageReport {
    let requirement_ids = requirements
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let allowed_paths = coverage_resume_paths(resume)
        .into_iter()
        .collect::<HashSet<_>>();
    let confirmed_ids = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();
    let mut model_items = HashMap::new();
    for item in output.items {
        if requirement_ids.contains(item.id.as_str()) && !model_items.contains_key(item.id.as_str())
        {
            model_items.insert(item.id.clone(), item);
        }
    }
    let mut items = Vec::new();
    for requirement in requirements {
        let model = model_items.remove(&requirement.id);
        let mut resume_paths = model
            .as_ref()
            .map(|item| {
                item.resume_paths
                    .iter()
                    .filter(|path| allowed_paths.contains(path.as_str()))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        resume_paths.sort();
        resume_paths.dedup();
        let mut evidence_fact_ids = model
            .as_ref()
            .map(|item| {
                item.evidence_fact_ids
                    .iter()
                    .filter(|id| confirmed_ids.contains(id.as_str()))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        evidence_fact_ids.sort();
        evidence_fact_ids.dedup();
        let requested_status = model
            .as_ref()
            .map(|item| item.status.as_str())
            .unwrap_or("unknown");
        let status = match requested_status {
            "covered" if !resume_paths.is_empty() => "covered",
            "strengthenable" if !evidence_fact_ids.is_empty() => "strengthenable",
            "gap" if resume_paths.is_empty() && evidence_fact_ids.is_empty() => "gap",
            "unknown" => "unknown",
            _ => "unknown",
        };
        if status == "gap" || status == "unknown" {
            resume_paths.clear();
            evidence_fact_ids.clear();
        }
        let rationale = model
            .as_ref()
            .map(|item| item.rationale.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or("模型未提供足够的可验证证据。")
            .chars()
            .take(300)
            .collect();
        items.push(ResumeCoverageItem {
            id: requirement.id.clone(),
            label: requirement.label.clone(),
            kind: requirement.kind.clone(),
            status: status.into(),
            resume_paths,
            evidence_fact_ids,
            rationale,
        });
    }
    summarize_resume_coverage(ResumeCoverageReport {
        job_id,
        target,
        target_version,
        source: "ai".into(),
        generated_at: time::shanghai_rfc3339(),
        items,
        covered_count: 0,
        strengthenable_count: 0,
        gap_count: 0,
        unknown_count: 0,
    })
}

fn provider_fingerprint(provider: &AiProviderConfig) -> String {
    hash_json(&json!({"id":provider.id,"baseUrl":provider.base_url,"model":provider.model}))
}

fn keyword_scope_key(keywords: &[ReportKeyword]) -> String {
    let mut keys = keywords
        .iter()
        .map(|keyword| keyword.key.clone())
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    hash_json(&json!({"keywordKeys": keys}))
}

fn interview_cache_key(
    scope_key: &str,
    dataset_hash: &str,
    resume: Option<&ResumeProfile>,
    provider_fingerprint: &str,
) -> String {
    hash_json(&json!({
        "skillVersion": INTERVIEW_SKILL_VERSION,
        "scopeKey": scope_key,
        "datasetHash": dataset_hash,
        "resume": resume.map(|value| json!({"id":value.id,"version":value.version})),
        "provider": provider_fingerprint
    }))
}

fn hash_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

fn sanitized_job_for_ai(job: &Job) -> Value {
    json!({
        "id":job.id,"title":job.title,"company":job.company,"salary":job.salary,
        "location":job.location,"experience":job.experience,"degree":job.degree,
        "companyScale":job.company_scale,"industry":job.industry,"skills":job.skills,
        "description":job.description,"structuredDetails":job.structured_details
    })
}

fn sanitized_resume_for_fit(resume: &ResumeProfile) -> Value {
    json!({
        "id":resume.id,"version":resume.version,"headline":resume.headline,
        "summary":resume.summary,"skills":resume.flattened_skills(),"experiences":resume.experiences,
        "education":resume.education,"facts":resume.facts.iter().filter(|fact| fact.confirmed).collect::<Vec<_>>(),
        "preferences":resume.preferences
    })
}

fn sanitized_resume_for_interview(resume: &ResumeProfile) -> Value {
    json!({
        "headline":resume.headline,"skills":resume.flattened_skills(),
        "facts":resume.facts.iter().filter(|fact| fact.confirmed).collect::<Vec<_>>(),
        "targetRoles":resume.preferences.target_roles
    })
}

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
    db: &Database,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fit_report(summary: &str, recommendation: &str, note: &str) -> FitReport {
        FitReport {
            overall_score: 80,
            confidence: 100,
            verdict: "strong".into(),
            recommendation: recommendation.into(),
            summary: summary.into(),
            dimensions: vec![FitDimension {
                key: "technical".into(),
                label: "技术匹配".into(),
                score: Some(80),
                weight: 30,
                note: note.into(),
                evidence: vec!["Python".into()],
            }],
            hard_constraints: vec![HardConstraint {
                label: "工作地点".into(),
                status: "pass".into(),
                note: "符合目标城市偏好".into(),
            }],
            strengths: vec!["Python 技术栈与岗位要求直接匹配".into()],
            gaps: vec!["需要补充 Kubernetes 生产实践".into()],
            evidence: vec!["Python".into()],
            generated_at: String::new(),
            skill_version: FIT_SKILL_VERSION.into(),
            input_hash: String::new(),
            analysis_source: "llm".into(),
            fallback_reason: None,
            cache_status: "fresh".into(),
        }
    }

    #[test]
    fn market_context_mode_rejects_job_mixing_variants_and_unsupported_edits() {
        let master = ResumeTargetRef {
            kind: "master".into(),
            id: "resume".into(),
        };
        let variant = ResumeTargetRef {
            kind: "variant".into(),
            id: "variant".into(),
        };
        let market = MarketResumeContextRequest {
            keyword_keys: vec!["ai agent".into()],
            focus_skills: vec![],
        };

        assert!(
            validate_resume_chat_context_mode(&master, Some("job"), Some(&market))
                .unwrap_err()
                .contains("不能同时使用")
        );
        assert!(
            validate_resume_chat_context_mode(&variant, None, Some(&market))
                .unwrap_err()
                .contains("仅可用于主简历")
        );
        assert!(validate_market_edit_evidence(true, false, false, &[], &[])
            .unwrap_err()
            .contains("必须引用"));
        assert!(
            validate_market_edit_evidence(true, false, false, &["fact-python".into()], &[]).is_ok()
        );
        assert!(validate_market_edit_evidence(
            true,
            false,
            false,
            &[],
            &["candidate-python".into()]
        )
        .is_ok());
        assert!(
            validate_market_edit_evidence(true, true, false, &["unrelated-fact".into()], &[])
                .unwrap_err()
                .contains("首次只能核实经历")
        );
        assert!(validate_market_edit_evidence(
            true,
            true,
            false,
            &[],
            &["candidate-python".into()]
        )
        .is_ok());
        assert!(validate_market_edit_evidence(
            true,
            false,
            true,
            &[],
            &["candidate-python".into()]
        )
        .unwrap_err()
        .contains("只能依据已确认事实"));
        assert!(!resume_edit_introduces_factual_content(
            &json!(["Python", "Rust"]),
            &json!(["Rust"])
        ));
        assert!(resume_edit_introduces_factual_content(
            &json!(["Python"]),
            &json!(["Python", "Rust"])
        ));
        assert!(!resume_edit_introduces_factual_content(
            &json!("使用 Python 构建服务，负责交付"),
            &json!("使用 Python 构建服务")
        ));
    }

    #[test]
    fn market_context_is_rebuilt_from_known_keywords_and_report_skills() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::new(directory.path().join("market-context.db"));
        db.initialize().unwrap();
        let resume: ResumeProfile = serde_json::from_value(json!({
            "id":"resume-market","name":"测试用户","headline":"AI 工程师","email":"a@example.com",
            "phone":"","location":"上海","website":"","summary":"使用 Python 构建服务",
            "templateId":"ai-engineering","professionalSkills":[],"experiences":[],"education":[],
            "projects":[],"certifications":[],"facts":[],"preferences":{},"sourceFileName":"",
            "updatedAt":"2026-07-16T00:00:00+08:00","version":1
        }))
        .unwrap();
        db.save_resume(&resume).unwrap();
        let job: Job = serde_json::from_value(json!({
            "id":"job-market","source":"boss","externalId":"job-market","title":"AI 工程师",
            "company":"测试公司","salary":"20-30K","location":"上海","experience":"3-5年","degree":"本科",
            "companyScale":"100-499人","companyStage":"","industry":"人工智能","skills":["Python","RAG"],
            "welfare":[],"description":"使用 Python 构建 RAG 服务","sourceUrl":"","firstSeen":"2026-07-16T08:00:00+08:00","lastSeen":"2026-07-16T08:00:00+08:00"
        })).unwrap();
        db.upsert_scrape_list_job(job, "AI Agent").unwrap();

        let context = resolve_resume_market_context(
            &db,
            &MarketResumeContextRequest {
                keyword_keys: vec!["AI AGENT".into()],
                focus_skills: vec!["Python".into()],
            },
        )
        .unwrap();
        assert_eq!(context.keyword_keys, vec!["ai agent"]);
        assert_eq!(context.keyword_labels, vec!["AI Agent"]);
        assert_eq!(context.total_jobs, 1);
        assert_eq!(context.skills.len(), 1);
        assert_eq!(context.skills[0].label, "Python");
        assert_eq!(context.skills[0].status, "covered");

        assert!(resolve_resume_market_context(
            &db,
            &MarketResumeContextRequest {
                keyword_keys: vec!["missing".into()],
                focus_skills: vec![],
            }
        )
        .unwrap_err()
        .contains("未知"));
        assert!(resolve_resume_market_context(
            &db,
            &MarketResumeContextRequest {
                keyword_keys: vec!["ai agent".into()],
                focus_skills: vec!["Kotlin".into()],
            }
        )
        .unwrap_err()
        .contains("不在当前报告范围"));
    }

    #[test]
    fn coverage_requirements_use_stable_ids_unicode_lengths_and_description_cap() {
        let description = (1..=25)
            .map(|index| format!("第 {index} 项岗位能力要求"))
            .collect::<Vec<_>>()
            .join("。");
        let mut job: Job = serde_json::from_value(json!({
            "id":"job-coverage","source":"test","externalId":"coverage","title":"AI 工程师",
            "company":"测试公司","salary":"","location":"","experience":"","degree":"",
            "companyScale":"","companyStage":"","industry":"","skills":["Python","SQL"],
            "welfare":[],"description":description,"sourceUrl":"","firstSeen":"","lastSeen":""
        }))
        .unwrap();

        let requirements = coverage_requirements(&job);

        assert_eq!(
            requirements
                .iter()
                .filter(|item| item.kind == "requirement")
                .count(),
            MAX_DESCRIPTION_COVERAGE_REQUIREMENTS
        );
        assert_eq!(requirements.len(), 22);
        assert_eq!(requirements[0].id, "requirement-512aae45ed67cf17");

        job.skills.clear();
        job.description = "😀".repeat(140);
        let unicode_requirements = coverage_requirements(&job);
        assert_eq!(unicode_requirements.len(), 1);
        assert_eq!(unicode_requirements[0].label.chars().count(), 140);
    }

    #[test]
    fn coverage_output_discards_invalid_paths_facts_and_unsupported_claims() {
        let resume: ResumeProfile = serde_json::from_value(json!({
            "id": "variant-1", "name": "测试用户", "headline": "AI 工程师", "email": "a@example.com",
            "phone": "", "location": "上海", "website": "", "summary": "使用 Python 构建平台",
            "templateId": "ai-engineering", "professionalSkills": [], "experiences": [], "education": [],
            "projects": [], "certifications": [],
            "facts": [{"id":"fact-sql","category":"skill","value":"熟练使用 SQL","source":"用户确认","confidence":1.0,"confirmed":true}],
            "preferences": {}, "sourceFileName": "", "updatedAt": "2026-07-16T00:00:00+08:00", "version": 2
        })).unwrap();
        let requirements = vec![
            CoverageRequirement {
                id: "python".into(),
                label: "Python".into(),
                kind: "skill".into(),
            },
            CoverageRequirement {
                id: "sql".into(),
                label: "SQL".into(),
                kind: "skill".into(),
            },
            CoverageRequirement {
                id: "kotlin".into(),
                label: "Kotlin".into(),
                kind: "skill".into(),
            },
            CoverageRequirement {
                id: "go".into(),
                label: "Go".into(),
                kind: "skill".into(),
            },
        ];
        let report = validate_model_coverage_report(
            "job-1".into(),
            ResumeTargetRef {
                kind: "variant".into(),
                id: resume.id.clone(),
            },
            resume.version,
            &resume,
            &requirements,
            ModelResumeCoverageOutput {
                items: vec![
                    ModelResumeCoverageItem {
                        id: "python".into(),
                        status: "covered".into(),
                        resume_paths: vec!["/facts/0".into()],
                        evidence_fact_ids: vec![],
                        rationale: "invalid path".into(),
                    },
                    ModelResumeCoverageItem {
                        id: "sql".into(),
                        status: "strengthenable".into(),
                        resume_paths: vec!["/summary".into()],
                        evidence_fact_ids: vec!["fact-missing".into(), "fact-sql".into()],
                        rationale: "valid fact".into(),
                    },
                    ModelResumeCoverageItem {
                        id: "kotlin".into(),
                        status: "covered".into(),
                        resume_paths: vec![],
                        evidence_fact_ids: vec![],
                        rationale: "no evidence".into(),
                    },
                    ModelResumeCoverageItem {
                        id: "not-a-requirement".into(),
                        status: "gap".into(),
                        resume_paths: vec![],
                        evidence_fact_ids: vec![],
                        rationale: "ignored".into(),
                    },
                ],
            },
        );

        assert_eq!(report.items[0].status, "unknown");
        assert_eq!(report.items[1].status, "strengthenable");
        assert_eq!(report.items[1].resume_paths, vec!["/summary"]);
        assert_eq!(report.items[1].evidence_fact_ids, vec!["fact-sql"]);
        assert_eq!(report.items[2].status, "unknown");
        assert_eq!(report.items[3].status, "unknown");
        assert_eq!(report.strengthenable_count, 1);
        assert_eq!(report.unknown_count, 3);
    }

    #[test]
    fn report_competitiveness_discards_invalid_ids_paths_and_facts() {
        let resume: ResumeProfile = serde_json::from_value(json!({
            "id": "resume", "name": "测试用户", "headline": "AI 工程师", "email": "a@example.com",
            "phone": "", "location": "上海", "website": "", "summary": "使用 Python 构建服务",
            "templateId": "ai-engineering", "professionalSkills": [], "experiences": [], "education": [],
            "projects": [], "certifications": [],
            "facts": [{"id":"fact-k8s","category":"skill","value":"Kubernetes 生产实践","source":"用户确认","confidence":1.0,"confirmed":true}],
            "preferences": {}, "sourceFileName": "", "updatedAt": "2026-07-16T00:00:00+08:00", "version": 2
        })).unwrap();
        let local = ReportCompetitivenessAnalysis {
            source: "local".into(),
            resume_id: resume.id.clone(),
            resume_version: resume.version,
            generated_at: String::new(),
            items: vec![
                ReportCompetitivenessItem {
                    id: "report-skill-1".into(),
                    label: "Python".into(),
                    job_count: 8,
                    percentage: 80.0,
                    status: "covered".into(),
                    resume_paths: vec!["/summary".into()],
                    evidence_fact_ids: vec![],
                    rationale: String::new(),
                },
                ReportCompetitivenessItem {
                    id: "report-skill-2".into(),
                    label: "Kubernetes".into(),
                    job_count: 5,
                    percentage: 50.0,
                    status: "strengthenable".into(),
                    resume_paths: vec![],
                    evidence_fact_ids: vec!["fact-k8s".into()],
                    rationale: String::new(),
                },
                ReportCompetitivenessItem {
                    id: "report-skill-3".into(),
                    label: "Rust".into(),
                    job_count: 2,
                    percentage: 20.0,
                    status: "gap".into(),
                    resume_paths: vec![],
                    evidence_fact_ids: vec![],
                    rationale: String::new(),
                },
            ],
        };
        let analysis = validate_model_report_competitiveness(
            &resume,
            &local,
            ModelResumeCoverageOutput {
                items: vec![
                    ModelResumeCoverageItem {
                        id: "report-skill-1".into(),
                        status: "covered".into(),
                        resume_paths: vec!["/invalid".into()],
                        evidence_fact_ids: vec![],
                        rationale: "bad path".into(),
                    },
                    ModelResumeCoverageItem {
                        id: "report-skill-2".into(),
                        status: "strengthenable".into(),
                        resume_paths: vec![],
                        evidence_fact_ids: vec!["fact-missing".into(), "fact-k8s".into()],
                        rationale: "valid fact".into(),
                    },
                    ModelResumeCoverageItem {
                        id: "report-skill-3".into(),
                        status: "covered".into(),
                        resume_paths: vec![],
                        evidence_fact_ids: vec![],
                        rationale: "no evidence".into(),
                    },
                    ModelResumeCoverageItem {
                        id: "not-allowed".into(),
                        status: "gap".into(),
                        resume_paths: vec![],
                        evidence_fact_ids: vec![],
                        rationale: "ignored".into(),
                    },
                ],
            },
        );

        assert_eq!(analysis.items[0].status, "unknown");
        assert_eq!(analysis.items[1].status, "strengthenable");
        assert_eq!(analysis.items[1].evidence_fact_ids, vec!["fact-k8s"]);
        assert_eq!(analysis.items[2].status, "unknown");
        assert_eq!(analysis.items.len(), 3);
    }

    #[test]
    fn fit_output_requires_chinese_in_key_narrative_fields() {
        assert!(fit_report_uses_chinese(&fit_report(
            "技术基础与岗位要求匹配。",
            "建议申请并突出相关项目成果。",
            "核心技能具备直接证据。",
        )));
        assert!(!fit_report_uses_chinese(&fit_report(
            "Strong technical foundation",
            "Apply for this role",
            "Good match",
        )));
    }
}
