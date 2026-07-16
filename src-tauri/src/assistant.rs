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

#[tauri::command]
pub async fn analyze_job(
    state: State<'_, AppState>,
    job_id: String,
    force: Option<bool>,
) -> Result<FitAnalysisResult, String> {
    distribution::require_privacy(&state)?;
    analyze_job_internal(&state.db, &job_id, force.unwrap_or(false)).await
}

async fn analyze_job_internal(
    db: &Database,
    job_id: &str,
    force: bool,
) -> Result<FitAnalysisResult, String> {
    let mut job = db
        .get_job(job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let resume = db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let provider = db.default_provider()?;
    let input_hash = fit_input_hash(&job, &resume, provider.as_ref());
    if !force {
        if let Some(fit) = &job.fit {
            if fit.input_hash == input_hash && fit.cache_status != "legacy" {
                return Ok(FitAnalysisResult {
                    source: if fit.analysis_source == "llm" {
                        "llm"
                    } else {
                        "local"
                    }
                    .into(),
                    job,
                    cache_hit: true,
                    warning: None,
                });
            }
        }
    }

    let mut fallback = scoring::deterministic_fit(&job, &resume);
    fallback.input_hash = input_hash.clone();
    fallback.analysis_source = "local".into();
    fallback.cache_status = "fresh".into();
    fallback.fallback_reason = if provider.is_none() {
        Some("provider_missing".into())
    } else {
        None
    };
    let mut warning = None;
    let fit = if let Some(provider) = provider.as_ref() {
        let input = json!({
            "job": sanitized_job_for_ai(&job),
            "resume": sanitized_resume_for_fit(&resume),
            "weights": {"technical":30,"experience":25,"behavior":15,"career":30}
        });
        match llm::run_skill::<FitReport>(provider, skills::JOB_FIT, &input).await {
            Ok(mut report) if fit_report_uses_chinese(&report) => {
                report.input_hash = input_hash;
                report.analysis_source = "llm".into();
                report.fallback_reason = None;
                report.cache_status = "fresh".into();
                report.generated_at = time::shanghai_rfc3339();
                report.skill_version = FIT_SKILL_VERSION.into();
                report
            }
            Ok(_) => {
                fallback.fallback_reason = Some("invalid_output".into());
                warning = Some("模型结果未按要求使用简体中文，已使用中文本地基础匹配。".into());
                fallback
            }
            Err(error) => {
                fallback.fallback_reason = Some("llm_failed".into());
                warning = Some(format!(
                    "模型暂不可用，已使用本地基础匹配：{}",
                    redact(&error)
                ));
                fallback
            }
        }
    } else {
        warning = Some("尚未配置模型，已使用本地基础匹配。".into());
        fallback
    };
    let source = if fit.analysis_source == "llm" {
        "llm"
    } else {
        "local"
    }
    .to_string();
    job.fit = Some(fit);
    db.save_job(&job)?;
    Ok(FitAnalysisResult {
        job,
        cache_hit: false,
        source,
        warning,
    })
}

#[tauri::command]
pub async fn start_fit_batch_for_query(
    app: AppHandle,
    state: State<'_, AppState>,
    query: JobQuery,
) -> Result<String, String> {
    let ids = state.db.job_ids_for_query(&query)?;
    start_fit_batch(app, state, ids).await
}

#[tauri::command]
pub async fn start_fit_batch(
    app: AppHandle,
    state: State<'_, AppState>,
    job_ids: Vec<String>,
) -> Result<String, String> {
    distribution::require_privacy(&state)?;
    if let Some(task) = state.db.running_task("fit")? {
        return Ok(task.id);
    }
    let mut seen = HashSet::new();
    let ids = job_ids
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Err("当前筛选结果中没有可分析岗位。".into());
    }
    state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let task = new_task("fit", &format!("批量分析 {} 个岗位", ids.len()));
    if !state.db.reserve_task(&task)? {
        return Err("已有批量匹配任务正在排队或运行。".into());
    }
    emit_task(&app, &task);
    let task_id = task.id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let mut task = task;
        let total = ids.len();
        let mut ai = 0;
        let mut local = 0;
        let mut cached = 0;
        let mut failed = 0;
        for (index, id) in ids.iter().enumerate() {
            update_task(
                &app,
                &db,
                &mut task,
                "running",
                5 + ((index as i64) * 90 / total as i64),
                &format!("正在分析 {}/{}", index + 1, total),
                None,
            );
            match analyze_job_internal(&db, id, false).await {
                Ok(result) if result.cache_hit => cached += 1,
                Ok(result) if result.source == "llm" => ai += 1,
                Ok(_) => local += 1,
                Err(_) => failed += 1,
            }
        }
        let message = format!("完成：AI {ai}，本地基础 {local}，缓存跳过 {cached}，失败 {failed}");
        update_task(
            &app,
            &db,
            &mut task,
            if failed == total {
                "failed"
            } else {
                "completed"
            },
            100,
            &message,
            if failed > 0 {
                Some(format!("{failed} 个岗位未能保存分析结果"))
            } else {
                None
            },
        );
    });
    Ok(task_id)
}

#[tauri::command]
pub fn open_job_source(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    distribution::require_privacy(&state)?;
    let job = state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let url =
        reqwest::Url::parse(job.source_url.trim()).map_err(|_| "原岗位链接不可用。".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("仅允许打开 http(s) 岗位链接。".into());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host != "zhipin.com" && !host.ends_with(".zhipin.com") {
        return Err("岗位链接不是受信任的 BOSS 域名。".into());
    }
    let _ = app;
    open_system_url(url.as_str())
}

#[tauri::command]
pub fn open_github_issues() -> Result<(), String> {
    open_system_url("https://github.com/kyle-kw/ai-job-app/issues")
}

#[cfg(target_os = "windows")]
fn open_system_url(url: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开系统默认浏览器：{error}"))
}

#[cfg(target_os = "macos")]
fn open_system_url(url: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开系统默认浏览器：{error}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_system_url(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开系统默认浏览器：{error}"))
}

#[tauri::command]
pub fn get_interview_preparation_state(
    state: State<'_, AppState>,
    keyword_keys: Vec<String>,
) -> Result<InterviewPreparationState, String> {
    interview_preparation_state(&state.db, &keyword_keys)
}

#[tauri::command]
pub async fn generate_interview_preparation(
    state: State<'_, AppState>,
    keyword_keys: Vec<String>,
    force: Option<bool>,
) -> Result<InterviewPreparationState, String> {
    distribution::require_privacy(&state)?;
    if keyword_keys.is_empty() {
        return Err("请先选择至少一个关键词，再生成 AI 面试准备。".into());
    }
    let selected_keywords = state.db.report_keywords_for_keys(&keyword_keys)?;
    if selected_keywords.is_empty() {
        return Err("所选关键词已不存在，请刷新后重新选择。".into());
    }
    let jobs = state.db.list_jobs_by_keyword_keys(&keyword_keys)?;
    if jobs.is_empty() {
        return Err("所选关键词暂无岗位，请调整筛选或先完成抓取。".into());
    }
    let provider = state
        .db
        .default_provider()?
        .ok_or_else(|| "请先配置并验证默认模型。".to_string())?;
    let resume = state.db.active_resume()?;
    let report = analytics::build_report_for_keywords(&jobs, selected_keywords.clone());
    let dataset_hash = dataset_hash(&jobs);
    let scope_key = keyword_scope_key(&selected_keywords);
    let provider_fingerprint = provider_fingerprint(&provider);
    let cache_key = interview_cache_key(
        &scope_key,
        &dataset_hash,
        resume.as_ref(),
        &provider_fingerprint,
    );
    if !force.unwrap_or(false) && state.db.interview_preparation_by_key(&cache_key)?.is_some() {
        return interview_preparation_state(&state.db, &keyword_keys);
    }
    let input = json!({
        "report": {
            "selectedKeywords": report.selected_keywords,
            "totalJobs": report.total_jobs,
            "roles": report.roles,
            "experience": report.experience,
            "degree": report.degree,
            "industries": report.industries,
            "companyScales": report.company_scales,
            "topSkills": report.top_skills.iter().take(20).collect::<Vec<_>>(),
            "skillPairs": report.skill_pairs.iter().take(15).collect::<Vec<_>>()
        },
        "resume": resume.as_ref().map(sanitized_resume_for_interview)
    });
    let output = llm::run_skill::<ModelInterviewPreparation>(
        &provider,
        skills::INTERVIEW_PREPARATION,
        &input,
    )
    .await?;
    let counts = report
        .top_skills
        .iter()
        .map(|item| (item.label.to_lowercase(), item.count))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let skills = output
        .skills
        .into_iter()
        .filter(|item| counts.contains_key(&item.name.to_lowercase()))
        .filter(|item| seen.insert(item.name.to_lowercase()))
        .take(8)
        .map(|item| InterviewPreparationSkill {
            job_count: counts.get(&item.name.to_lowercase()).copied(),
            name: item.name,
            gap: item.gap,
            action: item.action,
        })
        .collect();
    let preparation = InterviewPreparation {
        summary: output.summary,
        skills,
        project_ideas: output.project_ideas.into_iter().take(4).collect(),
        practice_questions: output.practice_questions.into_iter().take(8).collect(),
    };
    state
        .db
        .save_interview_preparation(&InterviewPreparationCacheRecord {
            cache_key,
            scope_key,
            dataset_hash,
            resume_id: resume.as_ref().map(|value| value.id.clone()),
            resume_version: resume.as_ref().map(|value| value.version),
            provider_fingerprint,
            skill_version: INTERVIEW_SKILL_VERSION.into(),
            generated_at: time::shanghai_rfc3339(),
            preparation,
        })?;
    interview_preparation_state(&state.db, &keyword_keys)
}

fn interview_preparation_state(
    db: &Database,
    keyword_keys: &[String],
) -> Result<InterviewPreparationState, String> {
    let provider = db.default_provider()?;
    let resume = db.active_resume()?;
    let has_provider = provider.is_some();
    let has_resume = resume.is_some();
    if keyword_keys.is_empty() {
        return Ok(InterviewPreparationState {
            status: "missing".into(),
            reason: Some("no_keywords".into()),
            has_provider,
            has_resume,
            generated_at: None,
            preparation: None,
        });
    }
    let selected_keywords = db.report_keywords_for_keys(keyword_keys)?;
    let scope_key = keyword_scope_key(&selected_keywords);
    let jobs = db.list_jobs_by_keyword_keys(keyword_keys)?;
    if jobs.is_empty() {
        return Ok(InterviewPreparationState {
            status: "missing".into(),
            reason: Some("no_jobs".into()),
            has_provider,
            has_resume,
            generated_at: None,
            preparation: None,
        });
    }
    let latest = db.latest_interview_preparation(&scope_key)?;
    let Some(provider) = provider else {
        return Ok(InterviewPreparationState {
            status: if latest.is_some() { "stale" } else { "missing" }.into(),
            reason: Some("no_provider".into()),
            has_provider: false,
            has_resume,
            generated_at: latest.as_ref().map(|item| item.generated_at.clone()),
            preparation: latest.map(|item| item.preparation),
        });
    };
    let key = interview_cache_key(
        &scope_key,
        &dataset_hash(&jobs),
        resume.as_ref(),
        &provider_fingerprint(&provider),
    );
    if let Some(record) = db.interview_preparation_by_key(&key)? {
        return Ok(InterviewPreparationState {
            status: "fresh".into(),
            reason: if has_resume {
                None
            } else {
                Some("no_resume".into())
            },
            has_provider: true,
            has_resume,
            generated_at: Some(record.generated_at),
            preparation: Some(record.preparation),
        });
    }
    Ok(InterviewPreparationState {
        status: if latest.is_some() { "stale" } else { "missing" }.into(),
        reason: if has_resume {
            Some("data_changed".into())
        } else {
            Some("no_resume".into())
        },
        has_provider: true,
        has_resume,
        generated_at: latest.as_ref().map(|item| item.generated_at.clone()),
        preparation: latest.map(|item| item.preparation),
    })
}

pub(crate) fn fresh_interview_preparation(
    db: &Database,
    keyword_keys: &[String],
) -> Result<Option<InterviewPreparation>, String> {
    let state = interview_preparation_state(db, keyword_keys)?;
    Ok(if state.status == "fresh" {
        state.preparation
    } else {
        None
    })
}

#[tauri::command]
pub fn get_report_competitiveness_state(
    state: State<'_, AppState>,
    keyword_keys: Vec<String>,
) -> Result<ReportCompetitivenessState, String> {
    report_competitiveness_state(&state.db, &keyword_keys)
}

#[tauri::command]
pub async fn generate_report_competitiveness(
    state: State<'_, AppState>,
    keyword_keys: Vec<String>,
    force: Option<bool>,
) -> Result<ReportCompetitivenessState, String> {
    distribution::require_privacy(&state)?;
    if keyword_keys.is_empty() {
        return Err("请先选择至少一个关键词，再运行 AI 竞争力分析。".into());
    }
    let selected_keywords = state.db.report_keywords_for_keys(&keyword_keys)?;
    if selected_keywords.is_empty() {
        return Err("所选关键词已不存在，请刷新后重新选择。".into());
    }
    let jobs = state.db.list_jobs_by_keyword_keys(&keyword_keys)?;
    if jobs.is_empty() {
        return Err("所选关键词暂无岗位，请调整筛选或先完成抓取。".into());
    }
    let resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let provider = state
        .db
        .default_provider()?
        .ok_or_else(|| "请先配置并验证默认模型。".to_string())?;
    let report = analytics::build_report_for_keywords(&jobs, selected_keywords.clone());
    let local = build_local_report_competitiveness(&report, &resume);
    let scope_key = keyword_scope_key(&selected_keywords);
    let dataset_hash = report_competitiveness_dataset_hash(&report);
    let provider_key = provider_fingerprint(&provider);
    let cache_key =
        report_competitiveness_cache_key(&scope_key, &dataset_hash, &resume, &provider_key);
    if !force.unwrap_or(false)
        && state
            .db
            .report_competitiveness_by_key(&cache_key)?
            .is_some()
    {
        return report_competitiveness_state(&state.db, &keyword_keys);
    }

    let allowed_paths = competitiveness_resume_fields(&resume)
        .into_iter()
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    let confirmed_facts = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .collect::<Vec<_>>();
    let input = json!({
        "skills": local.items.iter().map(|item| json!({
            "id":item.id,"label":item.label,"jobCount":item.job_count,"percentage":item.percentage
        })).collect::<Vec<_>>(),
        "resume": sanitized_resume_for_competitiveness(&resume),
        "confirmedFacts": confirmed_facts,
        "allowedResumePaths": allowed_paths,
    });
    let output = llm::run_skill::<ModelResumeCoverageOutput>(
        &provider,
        skills::REPORT_COMPETITIVENESS,
        &input,
    )
    .await
    .map_err(|error| format!("model_unavailable: {}", redact(&error)))?;
    let analysis = validate_model_report_competitiveness(&resume, &local, output);
    state
        .db
        .save_report_competitiveness(&ReportCompetitivenessCacheRecord {
            cache_key,
            scope_key,
            dataset_hash,
            resume_id: resume.id.clone(),
            resume_version: resume.version,
            provider_fingerprint: provider_key,
            skill_version: REPORT_COMPETITIVENESS_SKILL_VERSION.into(),
            generated_at: analysis.generated_at.clone(),
            analysis,
        })?;
    report_competitiveness_state(&state.db, &keyword_keys)
}

pub(crate) fn report_competitiveness_state(
    db: &Database,
    keyword_keys: &[String],
) -> Result<ReportCompetitivenessState, String> {
    let provider = db.default_provider()?;
    let resume = db.active_resume()?;
    let has_provider = provider.is_some();
    let has_resume = resume.is_some();
    if keyword_keys.is_empty() {
        return Ok(ReportCompetitivenessState {
            status: "missing".into(),
            reason: Some("no_keywords".into()),
            has_resume,
            has_provider,
            generated_at: None,
            local: None,
            ai: None,
            effective_source: None,
        });
    }
    let selected_keywords = db.report_keywords_for_keys(keyword_keys)?;
    let jobs = db.list_jobs_by_keyword_keys(keyword_keys)?;
    if jobs.is_empty() {
        return Ok(ReportCompetitivenessState {
            status: "missing".into(),
            reason: Some("no_jobs".into()),
            has_resume,
            has_provider,
            generated_at: None,
            local: None,
            ai: None,
            effective_source: None,
        });
    }
    let scope_key = keyword_scope_key(&selected_keywords);
    let latest = db.latest_report_competitiveness(&scope_key)?;
    let Some(resume) = resume else {
        return Ok(ReportCompetitivenessState {
            status: if latest.is_some() { "stale" } else { "missing" }.into(),
            reason: Some("no_resume".into()),
            has_resume: false,
            has_provider,
            generated_at: latest.as_ref().map(|record| record.generated_at.clone()),
            local: None,
            ai: latest.map(|record| record.analysis),
            effective_source: None,
        });
    };
    let report = analytics::build_report_for_keywords(&jobs, selected_keywords);
    let local = build_local_report_competitiveness(&report, &resume);
    let Some(provider) = provider else {
        return Ok(ReportCompetitivenessState {
            status: if latest.is_some() { "stale" } else { "missing" }.into(),
            reason: Some("no_provider".into()),
            has_resume: true,
            has_provider: false,
            generated_at: latest.as_ref().map(|record| record.generated_at.clone()),
            local: Some(local),
            ai: latest.map(|record| record.analysis),
            effective_source: Some("local".into()),
        });
    };
    let cache_key = report_competitiveness_cache_key(
        &scope_key,
        &report_competitiveness_dataset_hash(&report),
        &resume,
        &provider_fingerprint(&provider),
    );
    if let Some(record) = db.report_competitiveness_by_key(&cache_key)? {
        return Ok(ReportCompetitivenessState {
            status: "fresh".into(),
            reason: None,
            has_resume: true,
            has_provider: true,
            generated_at: Some(record.generated_at),
            local: Some(local),
            ai: Some(record.analysis),
            effective_source: Some("ai".into()),
        });
    }
    Ok(ReportCompetitivenessState {
        status: if latest.is_some() { "stale" } else { "missing" }.into(),
        reason: if latest.is_some() {
            Some("data_changed".into())
        } else {
            None
        },
        has_resume: true,
        has_provider: true,
        generated_at: latest.as_ref().map(|record| record.generated_at.clone()),
        local: Some(local),
        ai: latest.map(|record| record.analysis),
        effective_source: Some("local".into()),
    })
}

pub(crate) fn effective_report_competitiveness(
    db: &Database,
    keyword_keys: &[String],
) -> Result<Option<ReportCompetitivenessAnalysis>, String> {
    let state = report_competitiveness_state(db, keyword_keys)?;
    Ok(if state.status == "fresh" {
        state.ai.or(state.local)
    } else {
        state.local
    })
}

fn build_local_report_competitiveness(
    report: &JobDataReport,
    resume: &ResumeProfile,
) -> ReportCompetitivenessAnalysis {
    let fields = competitiveness_resume_fields(resume);
    let facts = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .collect::<Vec<_>>();
    let items = report
        .top_skills
        .iter()
        .take(12)
        .enumerate()
        .map(|(index, skill)| {
            let resume_paths = fields
                .iter()
                .filter(|(_, text)| coverage_text_matches(&skill.label, text))
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>();
            let evidence_fact_ids = facts
                .iter()
                .filter(|fact| coverage_text_matches(&skill.label, &fact.value))
                .map(|fact| fact.id.clone())
                .collect::<Vec<_>>();
            let (status, rationale) = if !resume_paths.is_empty() {
                ("covered", "主简历正文中已有明确表达。")
            } else if !evidence_fact_ids.is_empty() {
                (
                    "strengthenable",
                    "已确认事实中存在证据，但主简历正文尚未明确表达。",
                )
            } else {
                ("gap", "主简历正文和已确认事实中均未找到可靠证据。")
            };
            ReportCompetitivenessItem {
                id: format!("report-skill-{}", index + 1),
                label: skill.label.clone(),
                job_count: skill.count,
                percentage: skill.percentage,
                status: status.into(),
                resume_paths,
                evidence_fact_ids,
                rationale: rationale.into(),
            }
        })
        .collect();
    ReportCompetitivenessAnalysis {
        source: "local".into(),
        resume_id: resume.id.clone(),
        resume_version: resume.version,
        generated_at: time::shanghai_rfc3339(),
        items,
    }
}

fn validate_model_report_competitiveness(
    resume: &ResumeProfile,
    local: &ReportCompetitivenessAnalysis,
    output: ModelResumeCoverageOutput,
) -> ReportCompetitivenessAnalysis {
    let allowed_paths = competitiveness_resume_fields(resume)
        .into_iter()
        .map(|(path, _)| path)
        .collect::<HashSet<_>>();
    let confirmed_ids = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();
    let allowed_ids = local
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let mut model_items = HashMap::new();
    for item in output.items {
        if allowed_ids.contains(item.id.as_str()) && !model_items.contains_key(&item.id) {
            model_items.insert(item.id.clone(), item);
        }
    }
    let items = local
        .items
        .iter()
        .map(|baseline| {
            let model = model_items.remove(&baseline.id);
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
            let requested = model
                .as_ref()
                .map(|item| item.status.as_str())
                .unwrap_or("unknown");
            let status = match requested {
                "covered" if !resume_paths.is_empty() => "covered",
                "strengthenable" if !evidence_fact_ids.is_empty() => "strengthenable",
                "gap" if resume_paths.is_empty() && evidence_fact_ids.is_empty() => "gap",
                "unknown" => "unknown",
                _ => "unknown",
            };
            if matches!(status, "gap" | "unknown") {
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
            ReportCompetitivenessItem {
                id: baseline.id.clone(),
                label: baseline.label.clone(),
                job_count: baseline.job_count,
                percentage: baseline.percentage,
                status: status.into(),
                resume_paths,
                evidence_fact_ids,
                rationale,
            }
        })
        .collect();
    ReportCompetitivenessAnalysis {
        source: "ai".into(),
        resume_id: resume.id.clone(),
        resume_version: resume.version,
        generated_at: time::shanghai_rfc3339(),
        items,
    }
}

fn competitiveness_resume_fields(resume: &ResumeProfile) -> Vec<(String, String)> {
    let mut fields = vec![
        ("/headline".into(), resume.headline.clone()),
        ("/summary".into(), resume.summary.clone()),
    ];
    fields.extend(
        resume
            .professional_skills
            .iter()
            .enumerate()
            .map(|(index, item)| {
                (
                    format!("/professionalSkills/{index}"),
                    format!("{} {}", item.label, item.items.join(" ")),
                )
            }),
    );
    fields.extend(resume.experiences.iter().enumerate().map(|(index, item)| {
        (
            format!("/experiences/{index}"),
            format!(
                "{} {} {}",
                item.company,
                item.position,
                item.highlights.join(" ")
            ),
        )
    }));
    fields.extend(resume.projects.iter().enumerate().map(|(index, item)| {
        (
            format!("/projects/{index}"),
            format!(
                "{} {} {}",
                item.name,
                item.summary,
                item.highlights.join(" ")
            ),
        )
    }));
    fields.extend(resume.education.iter().enumerate().map(|(index, item)| {
        (
            format!("/education/{index}"),
            format!(
                "{} {} {} {} {}",
                item.institution,
                item.area,
                item.degree,
                item.degree_detail,
                item.highlights.join(" ")
            ),
        )
    }));
    fields.extend(
        resume
            .certifications
            .iter()
            .enumerate()
            .map(|(index, item)| {
                (
                    format!("/certifications/{index}"),
                    format!("{} {}", item.name, item.issuer),
                )
            }),
    );
    fields
}

fn coverage_text_matches(label: &str, text: &str) -> bool {
    let label = label.trim();
    if label.is_empty() {
        return false;
    }
    if label.is_ascii() && label.chars().any(|value| value.is_ascii_alphanumeric()) {
        let pattern = format!(r"(?i)(^|[^a-z0-9]){}([^a-z0-9]|$)", regex::escape(label));
        return regex::Regex::new(&pattern).is_ok_and(|pattern| pattern.is_match(text));
    }
    normalize_coverage_text(text).contains(&normalize_coverage_text(label))
}

fn report_competitiveness_dataset_hash(report: &JobDataReport) -> String {
    hash_json(&json!({
        "totalJobs": report.total_jobs,
        "skills": report.top_skills.iter().take(12).map(|item| json!({
            "label":item.label,"count":item.count,"percentage":item.percentage
        })).collect::<Vec<_>>()
    }))
}

fn report_competitiveness_cache_key(
    scope_key: &str,
    dataset_hash: &str,
    resume: &ResumeProfile,
    provider_fingerprint: &str,
) -> String {
    hash_json(&json!({
        "skillVersion": REPORT_COMPETITIVENESS_SKILL_VERSION,
        "scopeKey": scope_key,
        "datasetHash": dataset_hash,
        "resume": {"id":resume.id,"version":resume.version},
        "provider": provider_fingerprint
    }))
}

fn sanitized_resume_for_competitiveness(resume: &ResumeProfile) -> Value {
    json!({
        "headline":resume.headline,
        "summary":resume.summary,
        "professionalSkills":resume.professional_skills,
        "experiences":resume.experiences,
        "education":resume.education,
        "projects":resume.projects,
        "certifications":resume.certifications
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelResumeChatOutput {
    assistant_message: String,
    #[serde(default)]
    edits: Vec<ModelResumeEdit>,
    #[serde(default)]
    fact_candidates: Vec<ResumeFactCandidate>,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelResumeEdit {
    path: String,
    after: Value,
    rationale: String,
    #[serde(default)]
    evidence_fact_ids: Vec<String>,
    #[serde(default)]
    required_fact_candidate_ids: Vec<String>,
}

fn resolve_resume_market_context(
    db: &Database,
    request: &MarketResumeContextRequest,
) -> Result<ResumeChatMarketContext, String> {
    let mut keyword_keys = request
        .keyword_keys
        .iter()
        .map(|value| crate::db::normalize_keyword_key(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    keyword_keys.sort();
    keyword_keys.dedup();
    if keyword_keys.is_empty() || keyword_keys.len() > 8 {
        return Err("invalid_market_context: 请选择 1 至 8 个有效报告关键词。".into());
    }
    if request.focus_skills.len() > 12 {
        return Err("invalid_market_context: 最多关注 12 个当前报告技能。".into());
    }

    let selected_keywords = db.report_keywords_for_keys(&keyword_keys)?;
    if selected_keywords.len() != keyword_keys.len() {
        return Err("invalid_market_context: 包含未知或已失效的报告关键词。".into());
    }
    let jobs = db.list_jobs_by_keyword_keys(&keyword_keys)?;
    if jobs.is_empty() {
        return Err("invalid_market_context: 当前关键词范围没有本地岗位样本。".into());
    }
    let analysis = effective_report_competitiveness(db, &keyword_keys)?
        .ok_or_else(|| "invalid_market_context: 当前范围无法生成竞争力矩阵。".to_string())?;

    let mut requested_skills = request
        .focus_skills
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    requested_skills.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    if requested_skills.len() > 12 {
        return Err("invalid_market_context: 最多关注 12 个当前报告技能。".into());
    }
    let selected_items = if requested_skills.is_empty() {
        analysis.items.iter().take(12).collect::<Vec<_>>()
    } else {
        let mut items = Vec::with_capacity(requested_skills.len());
        for requested in requested_skills {
            let item = analysis
                .items
                .iter()
                .find(|item| item.label.eq_ignore_ascii_case(requested))
                .ok_or_else(|| {
                    format!("invalid_market_context: 技能“{requested}”不在当前报告范围内。")
                })?;
            if !items
                .iter()
                .any(|existing: &&ReportCompetitivenessItem| existing.id == item.id)
            {
                items.push(item);
            }
        }
        items
    };

    Ok(ResumeChatMarketContext {
        keyword_keys,
        keyword_labels: selected_keywords
            .iter()
            .map(|keyword| keyword.label.clone())
            .collect(),
        total_jobs: jobs.len() as i64,
        skills: selected_items
            .into_iter()
            .map(|item| ResumeChatMarketSkill {
                label: item.label.clone(),
                job_count: item.job_count,
                percentage: item.percentage,
                status: item.status.clone(),
                rationale: item.rationale.clone(),
            })
            .collect(),
    })
}

fn validate_resume_chat_context_mode(
    target: &ResumeTargetRef,
    job_id: Option<&str>,
    market_context: Option<&MarketResumeContextRequest>,
) -> Result<(), String> {
    if job_id.is_some() && market_context.is_some() {
        return Err("invalid_request: 关联岗位与市场样本上下文不能同时使用。".into());
    }
    if market_context.is_some() && target.kind != "master" {
        return Err("invalid_request: 市场样本上下文仅可用于主简历。".into());
    }
    Ok(())
}

fn validate_market_edit_evidence(
    market_factual_edit: bool,
    gap_only_context: bool,
    strengthenable_only_context: bool,
    evidence_fact_ids: &[String],
    required_fact_candidate_ids: &[String],
) -> Result<(), String> {
    if market_factual_edit && evidence_fact_ids.is_empty() && required_fact_candidate_ids.is_empty()
    {
        return Err("unsafe_proposal: 市场样本只能指导排序和措辞，事实性修改必须引用已确认事实或待确认事实。".into());
    }
    if market_factual_edit && gap_only_context && required_fact_candidate_ids.is_empty() {
        return Err(
            "unsafe_proposal: 市场缺口首次只能核实经历；用户明确补充事实并形成待确认候选后才能修改。"
                .into(),
        );
    }
    if market_factual_edit && strengthenable_only_context && evidence_fact_ids.is_empty() {
        return Err(
            "unsafe_proposal: 可强化项只能依据已确认事实生成修改，不能仅依赖新事实候选。".into(),
        );
    }
    Ok(())
}

fn resume_edit_introduces_factual_content(before: &Value, after: &Value) -> bool {
    if before == after {
        return false;
    }
    match (before, after) {
        (_, Value::String(value)) if value.trim().is_empty() => false,
        (Value::String(before), Value::String(after)) if before.contains(after) => false,
        (Value::Array(before), Value::Array(after)) => {
            !after.iter().all(|item| before.contains(item))
        }
        _ => true,
    }
}

#[tauri::command]
pub async fn propose_resume_chat_edits(
    state: State<'_, AppState>,
    request: ResumeChatRequest,
) -> Result<ResumeChatProposal, String> {
    distribution::require_privacy(&state)?;
    validate_chat_messages(&request.messages)?;
    let target = request.target.clone();
    validate_resume_chat_context_mode(
        &target,
        request.job_id.as_deref(),
        request.market_context.as_ref(),
    )?;
    let (resume, fixed_job_id) = match target.kind.as_str() {
        "variant" => {
            let detail = state
                .db
                .get_resume_variant(&target.id)?
                .ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
            let job_id = detail.summary.job_id.clone();
            (detail.profile, Some(job_id))
        }
        "master" => {
            let resume = state
                .db
                .active_resume()?
                .ok_or_else(|| "resume_not_found: 请先导入主简历。".to_string())?;
            (resume, None)
        }
        _ => return Err("invalid_request: 不支持的简历目标。".into()),
    };
    if resume.id != target.id || resume.version != request.expected_version {
        return Err("version_conflict: 简历已变化，请刷新后重新对话。".into());
    }
    let provider = state
        .db
        .default_provider()?
        .ok_or_else(|| "ai_not_ready: 请先配置并验证默认模型。".to_string())?;
    if fixed_job_id.is_some()
        && request
            .job_id
            .as_deref()
            .is_some_and(|id| Some(id) != fixed_job_id.as_deref())
    {
        return Err("job_mismatch: 岗位版本只能关联创建时选择的岗位。".into());
    }
    let effective_job_id = fixed_job_id.as_deref().or(request.job_id.as_deref());
    let job = effective_job_id
        .map(|id| state.db.get_job(id))
        .transpose()?
        .flatten();
    if effective_job_id.is_some() && job.is_none() {
        return Err("job_not_found: 关联岗位已不存在。".into());
    }
    let market_context = request
        .market_context
        .as_ref()
        .map(|context| resolve_resume_market_context(&state.db, context))
        .transpose()?;
    let input = json!({
        "resume": &resume,
        "confirmedFacts": resume.facts.iter().filter(|fact| fact.confirmed).collect::<Vec<_>>(),
        "job": job.as_ref().map(sanitized_job_for_ai),
        "marketContext": market_context,
        "messages": request.messages,
        "allowedPaths": allowed_resume_paths()
    });
    let output = llm::run_skill::<ModelResumeChatOutput>(&provider, skills::RESUME_CHAT, &input)
        .await
        .map_err(|error| format!("model_unavailable: {}", redact(&error)))?;
    if target.kind == "variant" && !output.fact_candidates.is_empty() {
        return Err("fact_requires_master: 岗位版本不能新增事实，请先回到主简历事实清单确认，再同步岗位版本。".into());
    }
    if output.edits.len() > 12 {
        return Err("invalid_model_output: 单次建议超过 12 项，请缩小修改范围。".into());
    }
    let message_ids = request
        .messages
        .iter()
        .map(|message| message.id.as_str())
        .collect::<HashSet<_>>();
    let candidate_ids = output
        .fact_candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect::<HashSet<_>>();
    if candidate_ids.len() != output.fact_candidates.len() {
        return Err("unsafe_proposal: 新事实候选存在重复 ID。".into());
    }
    for candidate in &output.fact_candidates {
        if candidate.value.trim().is_empty()
            || !allowed_fact_category(&candidate.category)
            || candidate
                .source_message_id
                .as_deref()
                .is_some_and(|id| !message_ids.contains(id))
        {
            return Err("unsafe_proposal: 新事实候选缺少有效的用户消息依据。".into());
        }
    }
    let confirmed = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();
    let resume_value = serde_json::to_value(&resume).map_err(|error| error.to_string())?;
    let mut paths = HashSet::new();
    let candidate_text = output
        .fact_candidates
        .iter()
        .map(|candidate| candidate.value.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let confirmed_text = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .map(|fact| fact.value.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut edits = Vec::new();
    for edit in output.edits {
        if !paths.insert(edit.path.clone()) {
            return Err("unsafe_proposal: 同一字段不能在一次建议中重复修改。".into());
        }
        let label = resume_path_label(&edit.path)
            .ok_or_else(|| format!("unsafe_proposal: 不允许修改字段 {}", edit.path))?;
        validate_resume_after(&edit.path, &edit.after)?;
        if edit
            .evidence_fact_ids
            .iter()
            .any(|id| !confirmed.contains(id.as_str()))
        {
            return Err("unsafe_proposal: 修改引用了未确认或不存在的事实。".into());
        }
        if edit
            .required_fact_candidate_ids
            .iter()
            .any(|id| !candidate_ids.contains(id.as_str()))
        {
            return Err("unsafe_proposal: 修改引用了不存在的新事实候选。".into());
        }
        let before = resume_value
            .get(edit.path.trim_start_matches('/'))
            .cloned()
            .ok_or_else(|| "unsafe_proposal: 无法读取修改前字段。".to_string())?;
        validate_market_edit_evidence(
            market_context.is_some()
                && resume_edit_introduces_factual_content(&before, &edit.after),
            market_context.as_ref().is_some_and(|context| {
                !context.skills.is_empty()
                    && context.skills.iter().all(|skill| skill.status == "gap")
            }),
            market_context.as_ref().is_some_and(|context| {
                !context.skills.is_empty()
                    && context
                        .skills
                        .iter()
                        .all(|skill| skill.status == "strengthenable")
            }),
            &edit.evidence_fact_ids,
            &edit.required_fact_candidate_ids,
        )?;
        validate_numeric_claims(&before, &edit.after, &confirmed_text, &candidate_text)?;
        validate_new_skills(
            &edit.path,
            &before,
            &edit.after,
            &resume,
            &output.fact_candidates,
        )?;
        edits.push(ResumeFieldEdit {
            id: Uuid::new_v4().to_string(),
            path: edit.path,
            label: label.into(),
            operation: "replace".into(),
            before,
            after: edit.after,
            rationale: edit.rationale.chars().take(500).collect(),
            evidence_fact_ids: edit.evidence_fact_ids,
            required_fact_candidate_ids: edit.required_fact_candidate_ids,
        });
    }
    Ok(ResumeChatProposal {
        proposal_id: Uuid::new_v4().to_string(),
        target,
        base_version: resume.version,
        job: job.map(|job| ResumeChatJob {
            id: job.id,
            title: job.title,
            company: job.company,
        }),
        market_context,
        assistant_message: output.assistant_message.chars().take(2_000).collect(),
        edits,
        fact_candidates: output.fact_candidates,
        warnings: output
            .warnings
            .into_iter()
            .take(8)
            .map(|warning| warning.chars().take(300).collect())
            .collect(),
    })
}

#[tauri::command]
pub fn apply_resume_chat_edits(
    state: State<'_, AppState>,
    request: ApplyResumeEditsRequest,
) -> Result<ResumeEditCommitResult, String> {
    if request.selected_edit_ids.is_empty() {
        return Err("invalid_request: 请至少选择一项修改。".into());
    }
    let target = request.proposal.target.clone();
    let current = match target.kind.as_str() {
        "variant" => state
            .db
            .get_resume_variant(&target.id)?
            .map(|value| value.profile)
            .ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?,
        "master" => state
            .db
            .active_resume()?
            .ok_or_else(|| "resume_not_found: 请先导入主简历。".to_string())?,
        _ => return Err("invalid_request: 不支持的简历目标。".into()),
    };
    if current.id != target.id
        || current.version != request.expected_version
        || current.version != request.proposal.base_version
    {
        return Err("version_conflict: 简历已变化，请刷新后重新生成建议。".into());
    }
    let selected = request
        .selected_edit_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let confirmed_candidates = request
        .confirmed_fact_candidate_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let known_candidates = request
        .proposal
        .fact_candidates
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<HashMap<_, _>>();
    if target.kind == "variant"
        && (!known_candidates.is_empty() || !confirmed_candidates.is_empty())
    {
        return Err("fact_requires_master: 岗位版本不能新增事实，请先在主简历中确认。".into());
    }
    let mut profile_value = serde_json::to_value(&current).map_err(|error| error.to_string())?;
    let object = profile_value
        .as_object_mut()
        .ok_or_else(|| "storage_error: 简历结构无效。".to_string())?;
    let mut applied = 0;
    let mut used_candidates = HashSet::new();
    for edit in &request.proposal.edits {
        if !selected.contains(edit.id.as_str()) {
            continue;
        }
        resume_path_label(&edit.path)
            .ok_or_else(|| "unsafe_proposal: 修改路径已失效。".to_string())?;
        validate_resume_after(&edit.path, &edit.after)?;
        let key = edit.path.trim_start_matches('/');
        let current_value = object
            .get(key)
            .ok_or_else(|| "unsafe_proposal: 修改字段已不存在。".to_string())?;
        if current_value != &edit.before {
            return Err("version_conflict: 修改前内容已变化，请重新生成建议。".into());
        }
        for candidate_id in &edit.required_fact_candidate_ids {
            if !confirmed_candidates.contains(candidate_id.as_str())
                || !known_candidates.contains_key(candidate_id.as_str())
            {
                return Err("unsafe_proposal: 请先确认修改所依赖的新事实。".into());
            }
            used_candidates.insert(candidate_id.clone());
        }
        object.insert(key.into(), edit.after.clone());
        applied += 1;
    }
    if applied == 0 {
        return Err("invalid_request: 选择的修改已不存在。".into());
    }
    let mut candidate: ResumeProfile = serde_json::from_value(profile_value)
        .map_err(|error| format!("unsafe_proposal: {error}"))?;
    candidate.id = current.id.clone();
    candidate.version = current.version;
    candidate.updated_at = current.updated_at.clone();
    candidate.preferences = current.preferences.clone();
    candidate.facts = current.facts.clone();
    for candidate_id in used_candidates {
        if target.kind == "variant" {
            return Err("fact_requires_master: 岗位版本不能新增事实，请先在主简历中确认。".into());
        }
        let fact = known_candidates
            .get(candidate_id.as_str())
            .ok_or_else(|| "unsafe_proposal: 新事实候选已失效。".to_string())?;
        candidate.facts.push(ResumeFact {
            id: Uuid::new_v4().to_string(),
            category: fact.category.clone(),
            value: fact.value.clone(),
            source: "AI 对话 · 用户确认".into(),
            confidence: 1.0,
            confirmed: true,
        });
    }
    ensure_resume_item_ids(&mut candidate);
    // The earlier read improves the error message only. commit_resume repeats
    // expected_version inside its write transaction and is the authoritative
    // guard against a concurrent resume update in this TOCTOU window.
    if target.kind == "variant" {
        state
            .db
            .commit_resume_variant(
                &target.id,
                candidate,
                request.expected_version,
                "variant-ai",
                &format!("岗位版本 AI 应用 {applied} 项修改"),
                None,
                None,
            )
            .map(|result| ResumeEditCommitResult::Variant(Box::new(result)))
    } else {
        let (source, summary) = if request.proposal.market_context.is_some() {
            (
                "market-ai-chat",
                format!("市场样本 AI 修改 · 应用 {applied} 项修改"),
            )
        } else {
            ("ai-chat", format!("AI 对话应用 {applied} 项修改"))
        };
        state
            .db
            .commit_resume(
                candidate,
                request.expected_version,
                source,
                &summary,
                request.proposal.job.as_ref().map(|job| job.id.clone()),
                Some(request.proposal.proposal_id),
                None,
            )
            .map(|result| ResumeEditCommitResult::Master(Box::new(result)))
    }
}

#[tauri::command]
pub async fn analyze_resume_coverage(
    state: State<'_, AppState>,
    target: ResumeTargetRef,
    force: bool,
) -> Result<ResumeCoverageReport, String> {
    distribution::require_privacy(&state)?;
    if target.kind != "variant" {
        return Err("invalid_request: 首期岗位覆盖分析仅支持岗位版本。".into());
    }
    let variant = state
        .db
        .get_resume_variant(&target.id)?
        .ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
    if variant.profile.version != variant.summary.version {
        return Err("storage_error: 岗位版本号不一致。".into());
    }
    let job = state
        .db
        .get_job(&variant.summary.job_id)?
        .ok_or_else(|| "job_not_found: 关联岗位已不存在。".to_string())?;
    let provider = state
        .db
        .default_provider()?
        .ok_or_else(|| "ai_not_ready: 请先配置并验证默认模型。".to_string())?;
    let requirements = coverage_requirements(&job);
    let job_fingerprint = coverage_job_fingerprint(&job)?;
    let provider_key = provider_fingerprint(&provider);
    let cache_key = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}|{}|{}|{}|{}",
                target.id,
                variant.profile.version,
                job_fingerprint,
                provider_key,
                RESUME_COVERAGE_SKILL_VERSION
            )
            .as_bytes()
        )
    );
    if !force {
        if let Some(cached) = state.db.resume_coverage_cache(&cache_key)? {
            return Ok(cached);
        }
    }
    let confirmed_facts = variant
        .profile
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .collect::<Vec<_>>();
    let allowed_paths = coverage_resume_paths(&variant.profile);
    let input = json!({
        "job": sanitized_job_for_ai(&job),
        "resume": &variant.profile,
        "confirmedFacts": confirmed_facts,
        "requirements": requirements,
        "allowedResumePaths": allowed_paths,
    });
    let output =
        llm::run_skill::<ModelResumeCoverageOutput>(&provider, skills::RESUME_COVERAGE, &input)
            .await
            .map_err(|error| format!("model_unavailable: {}", redact(&error)))?;
    let report = validate_model_coverage_report(
        job.id.clone(),
        target,
        variant.profile.version,
        &variant.profile,
        &requirements,
        output,
    );
    state.db.save_resume_coverage_cache(
        &cache_key,
        &job_fingerprint,
        &provider_key,
        RESUME_COVERAGE_SKILL_VERSION,
        &report,
    )?;
    Ok(report)
}

#[tauri::command]
pub fn list_resume_versions(
    state: State<'_, AppState>,
    resume_id: String,
) -> Result<Vec<ResumeVersionSummary>, String> {
    state.db.list_resume_versions(&resume_id)
}

#[tauri::command]
pub fn get_resume_version(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<ResumeVersionDetail, String> {
    state
        .db
        .get_resume_version(&version_id)?
        .ok_or_else(|| "简历版本不存在。".into())
}

#[tauri::command]
pub fn restore_resume_version(
    state: State<'_, AppState>,
    version_id: String,
    expected_version: i64,
) -> Result<ResumeCommitResult, String> {
    let detail = state
        .db
        .get_resume_version(&version_id)?
        .ok_or_else(|| "简历版本不存在。".to_string())?;
    let current = state
        .db
        .active_resume()?
        .ok_or_else(|| "当前没有主简历。".to_string())?;
    if current.id != detail.profile.id {
        return Err("不能把其他简历的历史恢复为当前版本。".into());
    }
    let mut candidate = detail.profile;
    candidate.preferences = current.preferences;
    state.db.commit_resume(
        candidate,
        expected_version,
        "rollback",
        &format!("恢复到 v{} 的内容", detail.summary.version),
        None,
        None,
        Some(detail.summary.version),
    )
}

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
