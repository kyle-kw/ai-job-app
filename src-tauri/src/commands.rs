use crate::analytics;
use crate::llm;
use crate::models::*;
use crate::scoring;
use crate::sidecar;
use crate::skills;
use crate::time;
use crate::AppState;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResumeExtractionOutput {
    profile: ResumeProfile,
    #[serde(default)]
    raw_text: String,
}

#[derive(Debug, Deserialize)]
struct GreetingOutput {
    text: String,
}

#[derive(Debug, Deserialize)]
struct PatchesOutput {
    patches: Vec<ResumePatch>,
}

#[derive(Debug, Deserialize)]
struct MarketReportOutput {
    markdown: String,
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Result<BootstrapSnapshot, String> {
    let providers = state.db.list_providers()?;
    Ok(BootstrapSnapshot {
        readiness: Readiness {
            ai: providers
                .iter()
                .any(|provider| provider.verified && provider.is_default),
            resume: state.db.active_resume()?.is_some(),
            // BOSS is checked for every scrape; a persisted flag would become stale.
            boss: false,
        },
        jobs: state.db.list_jobs()?,
        resume: state.db.active_resume()?,
        providers,
        tasks: state.db.list_tasks()?,
        scrape_runs: state.db.list_scrape_runs()?,
        settings: state.db.settings()?,
    })
}

#[tauri::command]
pub fn get_job_data_report(state: State<'_, AppState>) -> Result<JobDataReport, String> {
    Ok(analytics::build_report(&state.db.list_jobs()?))
}

#[tauri::command]
pub fn export_job_data_report(state: State<'_, AppState>) -> Result<RenderResult, String> {
    let report = analytics::build_report(&state.db.list_jobs()?);
    if report.total_jobs == 0 {
        return Err("岗位库暂无数据，请先完成至少一轮岗位抓取。".into());
    }
    let exports = state.data_dir.join("exports");
    std::fs::create_dir_all(&exports).map_err(|error| error.to_string())?;
    let file_name = format!("岗位数据报告_{}.html", time::shanghai_file_stamp());
    let output_path = exports.join(&file_name);
    std::fs::write(&output_path, analytics::render_html(&report).as_bytes())
        .map_err(|error| format!("无法导出岗位数据报告：{error}"))?;
    Ok(RenderResult {
        path: output_path.to_string_lossy().to_string(),
        file_name,
    })
}

#[tauri::command]
pub async fn start_scrape(
    app: AppHandle,
    state: State<'_, AppState>,
    spec: SearchSpec,
) -> Result<String, String> {
    let task = new_task("scrape", &format!("抓取 {} · {}", spec.city, spec.keyword));
    state.db.save_task(&task)?;
    emit_task(&app, &task);
    let task_id = task.id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let mut task = task;
        update_task(
            &app,
            &db,
            &mut task,
            "running",
            10,
            "正在启动或连接 BOSS 专用浏览器",
            None,
        );
        let request = json!({"op":"scrape_jobs","params":spec});
        update_task(
            &app,
            &db,
            &mut task,
            "running",
            28,
            "确认登录后将自动抓取岗位列表与详情",
            None,
        );
        match sidecar::request(request).await {
            Ok(value) => match serde_json::from_value::<SidecarJobBatch>(value) {
                Ok(batch) => {
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "running",
                        82,
                        "正在去重并写入本地岗位库",
                        None,
                    );
                    let report_jobs = batch.jobs.clone();
                    match db.upsert_jobs(batch.jobs) {
                        Ok(stats) => {
                            let mut report = batch.report_markdown;
                            if let Ok(Some(provider)) = db.default_provider() {
                                let input = json!({"keyword":spec.keyword,"city":spec.city,"jobs":report_jobs});
                                if let Ok(output) = llm::run_skill::<MarketReportOutput>(
                                    &provider,
                                    skills::JOB_MARKET_ANALYSIS,
                                    &input,
                                )
                                .await
                                {
                                    report = Some(output.markdown);
                                }
                            }
                            let now = time::shanghai_rfc3339();
                            let run = ScrapeRun {
                                id: Uuid::new_v4().to_string(),
                                keyword: spec.keyword.clone(),
                                city: spec.city.clone(),
                                total_seen: stats.inserted + stats.updated,
                                inserted: stats.inserted,
                                updated: stats.updated,
                                started_at: task.created_at.clone(),
                                completed_at: Some(now),
                                report_markdown: report,
                            };
                            let _ = db.save_scrape_run(&run);
                            update_task(
                                &app,
                                &db,
                                &mut task,
                                "completed",
                                100,
                                &format!("完成：新增 {}，更新 {}", stats.inserted, stats.updated),
                                None,
                            );
                        }
                        Err(error) => update_task(
                            &app,
                            &db,
                            &mut task,
                            "failed",
                            82,
                            "写入岗位库失败",
                            Some(error),
                        ),
                    }
                }
                Err(error) => update_task(
                    &app,
                    &db,
                    &mut task,
                    "failed",
                    70,
                    "抓取结果格式无效",
                    Some(error.to_string()),
                ),
            },
            Err(error) => {
                let recoverable = if error.contains("登录") || error.contains("CDP") {
                    "请在自动打开的 BOSS 专用浏览器中完成登录或验证码，然后重新点击“开始抓取”。"
                        .to_string()
                } else {
                    error
                };
                let progress = task.progress;
                update_task(
                    &app,
                    &db,
                    &mut task,
                    "failed",
                    progress,
                    "岗位抓取未完成",
                    Some(recoverable),
                );
            }
        }
    });
    Ok(task_id)
}

#[tauri::command]
pub async fn setup_boss(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let task = new_task("boss-login", "连接 BOSS 专用浏览器");
    state.db.save_task(&task)?;
    emit_task(&app, &task);
    let task_id = task.id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let mut task = task;
        update_task(
            &app,
            &db,
            &mut task,
            "running",
            15,
            "正在启动独立 Chrome Profile",
            None,
        );
        match sidecar::request(json!({"op":"setup_boss","params":{"loginTimeout":300}})).await {
            Ok(_) => {
                let _ = db.set_bool_flag("boss_logged_in", true);
                update_task(
                    &app,
                    &db,
                    &mut task,
                    "completed",
                    100,
                    "已确认 BOSS 登录状态",
                    None,
                );
            }
            Err(error) => update_task(
                &app,
                &db,
                &mut task,
                "failed",
                45,
                "未能确认登录状态",
                Some(error),
            ),
        }
    });
    Ok(task_id)
}

#[tauri::command]
pub async fn import_resume(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: ImportResumePayload,
) -> Result<String, String> {
    let extension = PathBuf::from(&payload.file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["pdf", "docx", "yaml", "yml"].contains(&extension.as_str()) {
        return Err("仅支持 PDF、DOCX、YAML 和 YML 文件。".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.content_base64.as_bytes())
        .map_err(|error| format!("简历文件编码无效：{error}"))?;
    let imports = state.data_dir.join("imports");
    std::fs::create_dir_all(&imports).map_err(|error| error.to_string())?;
    let input_path = imports.join(format!("{}.{}", Uuid::new_v4(), extension));
    std::fs::write(&input_path, bytes).map_err(|error| error.to_string())?;

    let task = new_task("resume-import", &format!("解析 {}", payload.file_name));
    state.db.save_task(&task)?;
    emit_task(&app, &task);
    let task_id = task.id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let mut task = task;
        update_task(
            &app,
            &db,
            &mut task,
            "running",
            18,
            "正在提取简历文本",
            None,
        );
        let request = json!({"op":"extract_resume","params":{"path":input_path,"fileName":payload.file_name}});
        match sidecar::request(request).await {
            Ok(value) => match serde_json::from_value::<ResumeExtractionOutput>(value) {
                Ok(mut output) => {
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "running",
                        52,
                        "正在识别经历与技能",
                        None,
                    );
                    if let Ok(Some(provider)) = db.default_provider() {
                        let input = json!({"fileName":payload.file_name,"rawText":output.raw_text,"fallbackProfile":output.profile});
                        if let Ok(mut ai_profile) = llm::run_skill::<ResumeProfile>(
                            &provider,
                            skills::RESUME_EXTRACTION,
                            &input,
                        )
                        .await
                        {
                            ai_profile.id = output.profile.id.clone();
                            ai_profile.source_file_name = payload.file_name.clone();
                            ai_profile.updated_at = time::shanghai_rfc3339();
                            ai_profile.version = 1;
                            ai_profile.preferences = output.profile.preferences.clone();
                            output.profile = ai_profile;
                        }
                    }
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "running",
                        84,
                        "正在建立可追溯事实清单",
                        None,
                    );
                    if output.profile.facts.is_empty() {
                        output.profile.facts = facts_from_profile(&output.profile);
                    }
                    match db.save_resume(&output.profile) {
                        Ok(_) => update_task(
                            &app,
                            &db,
                            &mut task,
                            "completed",
                            100,
                            "主简历已生成，请确认低置信度字段",
                            None,
                        ),
                        Err(error) => update_task(
                            &app,
                            &db,
                            &mut task,
                            "failed",
                            90,
                            "保存主简历失败",
                            Some(error),
                        ),
                    }
                }
                Err(error) => update_task(
                    &app,
                    &db,
                    &mut task,
                    "failed",
                    35,
                    "简历提取结果无效",
                    Some(error.to_string()),
                ),
            },
            Err(error) => update_task(
                &app,
                &db,
                &mut task,
                "failed",
                24,
                "无法读取简历",
                Some(error),
            ),
        }
    });
    Ok(task_id)
}

#[tauri::command]
pub fn save_resume(
    state: State<'_, AppState>,
    mut resume: ResumeProfile,
) -> Result<ResumeProfile, String> {
    resume.version += 1;
    resume.updated_at = time::shanghai_rfc3339();
    state.db.save_resume(&resume)?;
    Ok(resume)
}

#[tauri::command]
pub fn save_preferences(
    state: State<'_, AppState>,
    preferences: JobPreferences,
) -> Result<ResumeProfile, String> {
    let mut resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入简历。".to_string())?;
    resume.preferences = preferences;
    resume.version += 1;
    resume.updated_at = time::shanghai_rfc3339();
    state.db.save_resume(&resume)?;
    Ok(resume)
}

#[tauri::command]
pub async fn analyze_job(state: State<'_, AppState>, job_id: String) -> Result<Job, String> {
    let mut job = state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let fallback = scoring::deterministic_fit(&job, &resume);
    job.fit = if let Some(provider) = state.db.default_provider()? {
        let input = json!({"job":job,"resume":resume,"weights":{"technical":30,"experience":25,"behavior":15,"career":30}});
        Some(
            llm::run_skill::<FitReport>(&provider, skills::JOB_FIT, &input)
                .await
                .unwrap_or(fallback),
        )
    } else {
        Some(fallback)
    };
    state.db.save_job(&job)?;
    Ok(job)
}

#[tauri::command]
pub async fn generate_greeting(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<String, String> {
    let mut job = state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let fallback = scoring::fallback_greeting(&job, &resume);
    let mut greeting = if let Some(provider) = state.db.default_provider()? {
        let input = json!({"job":job,"resumeFacts":resume.facts,"maxChineseCharacters":60});
        llm::run_skill::<GreetingOutput>(&provider, skills::GREETING_MESSAGE, &input)
            .await
            .map(|output| output.text)
            .unwrap_or(fallback)
    } else {
        fallback
    };
    if greeting.chars().count() > 60 {
        greeting = greeting.chars().take(60).collect();
    }
    job.greeting = Some(greeting.clone());
    state.db.save_job(&job)?;
    Ok(greeting)
}

#[tauri::command]
pub async fn propose_tailoring(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<Vec<ResumePatch>, String> {
    let mut job = state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let fallback = scoring::deterministic_patches(&job, &resume);
    let patches = if let Some(provider) = state.db.default_provider()? {
        let input = json!({"job":job,"resume":resume,"confirmedFacts":resume.facts.iter().filter(|fact| fact.confirmed).collect::<Vec<_>>()});
        llm::run_skill::<PatchesOutput>(&provider, skills::RESUME_TAILOR, &input)
            .await
            .map(|output| output.patches)
            .unwrap_or(fallback)
    } else {
        fallback
    };
    job.patches = patches.clone();
    state.db.save_job(&job)?;
    Ok(patches)
}

#[tauri::command]
pub fn update_resume_patch(
    state: State<'_, AppState>,
    job_id: String,
    patch_id: String,
    status: String,
    after: Option<String>,
) -> Result<Vec<ResumePatch>, String> {
    if !["pending", "accepted", "rejected"].contains(&status.as_str()) {
        return Err("无效的修改状态。".into());
    }
    let mut job = state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let patch = job
        .patches
        .iter_mut()
        .find(|patch| patch.id == patch_id)
        .ok_or_else(|| "修改建议不存在。".to_string())?;
    patch.status = status;
    if let Some(after) = after {
        patch.after = after;
    }
    state.db.save_job(&job)?;
    Ok(job.patches)
}

#[tauri::command]
pub async fn render_resume(
    state: State<'_, AppState>,
    job_id: Option<String>,
) -> Result<RenderResult, String> {
    let mut resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let mut file_stem = format!("{}_主简历", safe_file_name(&resume.name));
    if let Some(job_id) = job_id {
        let job = state
            .db
            .get_job(&job_id)?
            .ok_or_else(|| "岗位不存在。".to_string())?;
        file_stem = format!(
            "{}_{}_专岗简历",
            safe_file_name(&resume.name),
            safe_file_name(&job.company)
        );
        apply_accepted_patches(&mut resume, &job.patches);
    }
    let exports = state.data_dir.join("exports");
    std::fs::create_dir_all(&exports).map_err(|error| error.to_string())?;
    let output_path = exports.join(format!("{file_stem}.pdf"));
    let value = sidecar::request(
        json!({"op":"render_resume","params":{"profile":resume,"outputPath":output_path}}),
    )
    .await?;
    let rendered_path = value
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_else(|| output_path.to_str().unwrap_or_default())
        .to_string();
    Ok(RenderResult {
        path: rendered_path,
        file_name: format!("{file_stem}.pdf"),
    })
}

#[tauri::command]
pub fn save_provider(
    state: State<'_, AppState>,
    mut provider: AiProviderConfig,
) -> Result<Vec<AiProviderConfig>, String> {
    if let Some(key) = provider.api_key.take().filter(|key| !key.trim().is_empty()) {
        provider.api_key_ref = Some(llm::store_secret(&provider.id, &key)?);
        provider.verified = false;
    }
    state.db.save_provider(&provider)?;
    state.db.list_providers()
}

#[tauri::command]
pub async fn test_provider(
    state: State<'_, AppState>,
    mut provider: AiProviderConfig,
) -> Result<ProviderTestResult, String> {
    let result = llm::test(&provider).await?;
    if result.ok {
        if let Some(key) = provider.api_key.take().filter(|key| !key.trim().is_empty()) {
            provider.api_key_ref = Some(llm::store_secret(&provider.id, &key)?);
        }
        provider.verified = true;
        provider.last_tested_at = Some(time::shanghai_rfc3339());
        state.db.save_provider(&provider)?;
    }
    Ok(result)
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    mut settings: AppSettings,
) -> Result<AppSettings, String> {
    settings.telemetry = false;
    state.db.save_settings(&settings)?;
    Ok(settings)
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
    for skill in &profile.skills {
        facts.push(ResumeFact {
            id: Uuid::new_v4().to_string(),
            category: "skill".into(),
            value: skill.clone(),
            source: format!("{} · 技能", profile.source_file_name),
            confidence: 0.95,
            confirmed: true,
        });
    }
    for (experience_index, experience) in profile.experiences.iter().enumerate() {
        for highlight in &experience.highlights {
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
    facts
}

fn apply_accepted_patches(resume: &mut ResumeProfile, patches: &[ResumePatch]) {
    for patch in patches.iter().filter(|patch| patch.status == "accepted") {
        if patch.section.contains("简介") {
            resume.summary = patch.after.clone();
            continue;
        }
        for experience in &mut resume.experiences {
            for highlight in &mut experience.highlights {
                if *highlight == patch.before {
                    *highlight = patch.after.clone();
                }
            }
        }
    }
}

fn safe_file_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if "\\/:*?\"<>|".contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect()
}

fn redact(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| {
            if token.starts_with("sk-") || token.starts_with("tp-") {
                "[REDACTED]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
