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
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

const SUPPORTED_SCRAPE_CITIES: [&str; 25] = [
    "北京",
    "上海",
    "广州",
    "深圳",
    "杭州",
    "天津",
    "西安",
    "苏州",
    "武汉",
    "厦门",
    "长沙",
    "成都",
    "郑州",
    "重庆",
    "佛山",
    "合肥",
    "济南",
    "青岛",
    "南京",
    "东莞",
    "昆明",
    "南昌",
    "石家庄",
    "宁波",
    "福州",
];

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

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Result<BootstrapSnapshot, String> {
    let providers = crate::provider_policy::available_providers(state.db.list_providers()?);
    let resume = state.db.active_resume()?;
    let usable_provider = providers.iter().find(|provider| {
        provider.verified && provider.is_default && llm::secret_available(provider)
    });
    let boss_profile = state.db.boss_profile_state()?;
    let boss_running = state.db.running_task("boss-login")?.is_some();
    let boss_configuration = if boss_running {
        ConfigurationItem {
            state: "running".into(),
            message: "正在等待 BOSS 登录并验证。".into(),
            last_attempt_at: boss_profile.last_attempt_at.clone(),
        }
    } else if boss_profile.configured {
        ConfigurationItem {
            state: "ready".into(),
            message: "BOSS 专用 Chrome Profile 已配置。".into(),
            last_attempt_at: boss_profile.last_attempt_at.clone(),
        }
    } else if boss_profile.last_attempt_status == "failed" {
        ConfigurationItem {
            state: "failed".into(),
            message: boss_profile
                .last_error
                .clone()
                .unwrap_or_else(|| "BOSS 配置失败，请重试。".into()),
            last_attempt_at: boss_profile.last_attempt_at.clone(),
        }
    } else {
        ConfigurationItem {
            state: "needs_setup".into(),
            message: "需要配置 BOSS 专用浏览器。".into(),
            last_attempt_at: None,
        }
    };
    let selected_provider = providers.iter().find(|provider| provider.is_default);
    let llm_configuration = if usable_provider.is_some() {
        ConfigurationItem {
            state: "ready".into(),
            message: "默认模型已验证。".into(),
            last_attempt_at: selected_provider.and_then(|provider| provider.last_tested_at.clone()),
        }
    } else if let Some(error) =
        selected_provider.and_then(|provider| provider.last_test_error.clone())
    {
        ConfigurationItem {
            state: "failed".into(),
            message: error,
            last_attempt_at: selected_provider.and_then(|provider| provider.last_tested_at.clone()),
        }
    } else {
        ConfigurationItem {
            state: "needs_setup".into(),
            message: "填写 API Key 并测试默认模型。".into(),
            last_attempt_at: selected_provider.and_then(|provider| provider.last_tested_at.clone()),
        }
    };
    Ok(BootstrapSnapshot {
        readiness: Readiness {
            ai: usable_provider.is_some(),
            resume: resume.is_some(),
            boss: boss_profile.configured,
        },
        configuration: ConfigurationSnapshot {
            boss: boss_configuration,
            llm: llm_configuration,
        },
        resume,
        providers,
        tasks: state.db.list_tasks()?,
        scrape_runs: state.db.list_scrape_runs()?,
        settings: state.db.settings()?,
    })
}

#[tauri::command]
pub fn list_jobs_page(state: State<'_, AppState>, query: JobQuery) -> Result<JobPage, String> {
    let resume = state.db.active_resume()?;
    let provider = state.db.default_provider()?;
    let mut page = state.db.list_jobs_page(&query)?;
    crate::assistant::mark_fit_cache_status(&mut page.items, resume.as_ref(), provider.as_ref());
    Ok(page)
}

#[tauri::command]
pub fn list_job_options(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<JobOption>, String> {
    state.db.list_job_options(&query)
}

#[tauri::command]
pub fn list_job_cities(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.db.list_job_cities()
}

#[tauri::command]
pub fn get_job(state: State<'_, AppState>, job_id: String) -> Result<Job, String> {
    state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "Job does not exist.".to_string())
}

#[tauri::command]
pub fn delete_job(state: State<'_, AppState>, job_id: String) -> Result<DeleteJobsResult, String> {
    let deleted_count = state.db.delete_job(&job_id)?;
    if deleted_count == 0 {
        return Err("岗位不存在或已被删除。".into());
    }
    Ok(DeleteJobsResult { deleted_count })
}

#[tauri::command]
pub fn delete_missing_description_jobs(
    state: State<'_, AppState>,
    query: JobQuery,
) -> Result<DeleteJobsResult, String> {
    Ok(DeleteJobsResult {
        deleted_count: state.db.delete_missing_description_jobs(&query)?,
    })
}

#[tauri::command]
pub fn list_report_keywords(state: State<'_, AppState>) -> Result<Vec<ReportKeyword>, String> {
    state.db.list_report_keywords()
}

#[tauri::command]
pub fn get_job_data_report(
    state: State<'_, AppState>,
    keyword_keys: Vec<String>,
) -> Result<JobDataReport, String> {
    selected_job_data_report(&state.db, &keyword_keys)
}

#[tauri::command]
pub fn export_jobs_json(
    state: State<'_, AppState>,
    output_path: String,
) -> Result<RenderResult, String> {
    let jobs = state.db.list_jobs()?;
    if jobs.is_empty() {
        return Err("暂无岗位可导出。".into());
    }
    let output_path = validate_export_path(output_path, "json", "岗位 JSON")?;
    let payload = serialize_jobs_json(&jobs)?;
    std::fs::write(&output_path, payload).map_err(|error| format!("无法导出岗位 JSON：{error}"))?;
    render_result(output_path)
}

fn serialize_jobs_json(jobs: &[Job]) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(jobs).map_err(|error| format!("无法生成岗位 JSON：{error}"))
}

#[tauri::command]
pub fn export_job_data_report(
    state: State<'_, AppState>,
    keyword_keys: Vec<String>,
    output_path: String,
) -> Result<RenderResult, String> {
    let report = selected_job_data_report(&state.db, &keyword_keys)?;
    if report.total_jobs == 0 {
        return Err("所选关键词暂无岗位，请调整筛选或先完成抓取。".into());
    }
    let output_path = validate_export_path(output_path, "html", "岗位数据报告")?;
    let mut html = analytics::render_html(&report);
    if let Some(preparation) =
        crate::assistant::fresh_interview_preparation(&state.db, &keyword_keys)?
    {
        html = analytics::append_interview_preparation(html, &preparation);
    }
    std::fs::write(&output_path, html.as_bytes())
        .map_err(|error| format!("无法导出岗位数据报告：{error}"))?;
    render_result(output_path)
}

fn validate_export_path(
    output_path: String,
    expected_extension: &str,
    label: &str,
) -> Result<PathBuf, String> {
    if output_path.trim().is_empty() {
        return Err(format!("请选择{label}的保存位置。"));
    }
    let output_path = PathBuf::from(output_path);
    let extension_matches = output_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected_extension));
    if !extension_matches {
        return Err(format!("{label}必须保存为 .{expected_extension} 文件。"));
    }
    if let Some(parent) = output_path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    Ok(output_path)
}

fn render_result(output_path: PathBuf) -> Result<RenderResult, String> {
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "导出文件名无效。".to_string())?
        .to_string();
    Ok(RenderResult {
        path: output_path.to_string_lossy().to_string(),
        file_name,
    })
}

fn selected_job_data_report(
    db: &crate::db::Database,
    keyword_keys: &[String],
) -> Result<JobDataReport, String> {
    if keyword_keys.is_empty() {
        return Err("请先选择至少一个关键词，再生成数据报告。".into());
    }
    let selected_keywords = db.report_keywords_for_keys(keyword_keys)?;
    if selected_keywords.is_empty() {
        return Err("所选关键词已不存在，请刷新后重新选择。".into());
    }
    let jobs = db.list_jobs_by_keyword_keys(keyword_keys)?;
    Ok(analytics::build_report_for_keywords(
        &jobs,
        selected_keywords,
    ))
}

fn streamed_job_key(job: &Job) -> String {
    if job.external_id.trim().is_empty() {
        format!(
            "{}|fp:{}",
            job.source.to_lowercase(),
            crate::db::fingerprint(&job.company, &job.title, &job.location)
        )
    } else {
        format!("{}|{}", job.source.to_lowercase(), job.external_id.trim())
    }
}

#[tauri::command]
pub async fn start_scrape(
    app: AppHandle,
    state: State<'_, AppState>,
    mut spec: SearchSpec,
) -> Result<String, String> {
    distribution::require_privacy(&state)?;
    ensure_chrome_available().await?;
    spec.keyword = spec.keyword.trim().to_string();
    spec.city = spec.city.trim().to_string();
    if spec.keyword.is_empty() {
        return Err("岗位关键词不能为空。".into());
    }
    if !SUPPORTED_SCRAPE_CITIES.contains(&spec.city.as_str()) {
        return Err("城市不在当前支持的热门城市列表中。".into());
    }
    if !(1..=5).contains(&spec.pages) {
        return Err("抓取页数只能选择 1 至 5 页。".into());
    }
    if state.db.running_task("scrape")?.is_some() {
        return Err("已有岗位抓取任务正在运行，请等待其完成后再开始新的抓取。".into());
    }
    let completed_detail_external_ids = state.db.completed_detail_external_ids("boss")?;
    let mut request_params = serde_json::to_value(&spec).map_err(|error| error.to_string())?;
    request_params
        .as_object_mut()
        .ok_or_else(|| "抓取参数格式无效。".to_string())?
        .insert(
            "completedDetailExternalIds".into(),
            json!(completed_detail_external_ids),
        );
    let estimated_minutes = i64::from(spec.pages) * 20;
    let task = new_task("scrape", &format!("抓取 {} · {}", spec.city, spec.keyword));
    if !state.db.reserve_task(&task)? {
        return Err("已有同类抓取任务正在排队或运行。".into());
    }
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
            &format!(
                "正在检查 BOSS 登录状态；若出现登录界面，请在 5 分钟内完成登录。预计抓取约 {estimated_minutes} 分钟"
            ),
            None,
        );
        let request = json!({"op":"scrape_jobs","params":request_params});
        update_task(
            &app,
            &db,
            &mut task,
            "running",
            28,
            "正在等待登录验证；验证成功后将自动抓取岗位列表与详情",
            None,
        );
        let task_state = Arc::new(Mutex::new(task));
        let streamed = Arc::new(Mutex::new((HashSet::<String>::new(), 0_i64, 0_i64)));
        let streamed_for_events = Arc::clone(&streamed);
        let task_for_events = Arc::clone(&task_state);
        let db_for_events = db.clone();
        let app_for_events = app.clone();
        let keyword_for_events = spec.keyword.clone();
        match sidecar::request_with_events(request, move |event| {
            match event.get("type").and_then(Value::as_str) {
                Some("progress") => {
                    let progress = event
                        .get("progress")
                        .and_then(Value::as_i64)
                        .unwrap_or(30)
                        .clamp(1, 95);
                    let raw_message = event
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("岗位抓取进行中");
                    let message = if raw_message.contains("请勿关闭应用") {
                        raw_message.to_string()
                    } else {
                        format!("{raw_message}；抓取期间请勿关闭应用，切换页面不会中断")
                    };
                    let mut task = task_for_events
                        .lock()
                        .map_err(|_| "岗位抓取任务状态不可用".to_string())?;
                    update_task(
                        &app_for_events,
                        &db_for_events,
                        &mut task,
                        "running",
                        progress,
                        &message,
                        None,
                    );
                    return Ok(());
                }
                Some("job") => {}
                _ => return Ok(()),
            }
            let job: Job = serde_json::from_value(
                event
                    .get("job")
                    .cloned()
                    .ok_or("sidecar 岗位事件缺少 job 字段")?,
            )
            .map_err(|error| format!("sidecar 岗位事件格式无效：{error}"))?;
            let phase = event
                .get("phase")
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    if job.description.trim().is_empty() {
                        "list"
                    } else {
                        "detail"
                    }
                });
            if phase == "detail" {
                if !job.description.trim().is_empty() {
                    db_for_events.upsert_scrape_detail_job(job, &keyword_for_events)?;
                }
            } else {
                let job_key = streamed_job_key(&job);
                let already_seen = streamed_for_events
                    .lock()
                    .map_err(|_| "岗位抓取统计状态不可用".to_string())?
                    .0
                    .contains(&job_key);
                let stats = db_for_events.upsert_scrape_list_job(job, &keyword_for_events)?;
                if !already_seen {
                    let mut streamed = streamed_for_events
                        .lock()
                        .map_err(|_| "岗位抓取统计状态不可用".to_string())?;
                    if streamed.0.insert(job_key) {
                        streamed.1 += stats.inserted;
                        streamed.2 += stats.updated;
                    }
                }
            }
            Ok(())
        })
        .await
        {
            Ok(value) => match serde_json::from_value::<SidecarJobBatch>(value) {
                Ok(batch) => {
                    let mut task = task_state
                        .lock()
                        .map(|task| task.clone())
                        .unwrap_or_else(|_| new_task("scrape", "岗位抓取"));
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "running",
                        82,
                        "正在完成抓取并核对本地数据；抓取期间请勿关闭应用，切换页面不会中断",
                        None,
                    );
                    let report_jobs = batch.jobs;
                    let reconcile_result = report_jobs.iter().try_for_each(|job| {
                        let job_key = streamed_job_key(job);
                        let already_streamed = streamed
                            .lock()
                            .map_err(|_| "岗位抓取统计状态不可用".to_string())?
                            .0
                            .contains(&job_key);
                        if !already_streamed {
                            let stats = db.upsert_scrape_list_job(job.clone(), &spec.keyword)?;
                            let mut streamed = streamed
                                .lock()
                                .map_err(|_| "岗位抓取统计状态不可用".to_string())?;
                            if streamed.0.insert(job_key) {
                                streamed.1 += stats.inserted;
                                streamed.2 += stats.updated;
                            }
                        }
                        if !job.description.trim().is_empty() {
                            db.upsert_scrape_detail_job(job.clone(), &spec.keyword)?;
                        }
                        Ok(())
                    });
                    match reconcile_result {
                        Ok(()) => {
                            let (inserted, updated) = streamed
                                .lock()
                                .map(|state| (state.1, state.2))
                                .unwrap_or_default();
                            let now = time::shanghai_rfc3339();
                            let run = ScrapeRun {
                                id: Uuid::new_v4().to_string(),
                                keyword: spec.keyword.clone(),
                                city: spec.city.clone(),
                                total_seen: inserted + updated,
                                inserted,
                                updated,
                                started_at: task.created_at.clone(),
                                completed_at: Some(now),
                                report_markdown: None,
                            };
                            let _ = db.save_scrape_run(&run);
                            update_task(
                                &app,
                                &db,
                                &mut task,
                                "completed",
                                100,
                                &format!("完成：新增 {inserted}，更新 {updated}"),
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
                Err(error) => {
                    let mut task = task_state
                        .lock()
                        .map(|task| task.clone())
                        .unwrap_or_else(|_| new_task("scrape", "岗位抓取"));
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "failed",
                        70,
                        "抓取结果格式无效",
                        Some(error.to_string()),
                    );
                }
            },
            Err(error) => {
                let mut task = task_state
                    .lock()
                    .map(|task| task.clone())
                    .unwrap_or_else(|_| new_task("scrape", "岗位抓取"));
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
pub async fn start_job_detail_extraction(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    distribution::require_privacy(&state)?;
    let provider = state
        .db
        .default_provider()?
        .ok_or_else(|| "请先在设置中配置并验证默认 AI 模型。".to_string())?;
    let jobs = state.db.pending_detail_jobs()?;
    if jobs.is_empty() {
        return Err("没有待提取的岗位详情。".into());
    }

    let task = new_task(
        "job-detail-extraction",
        &format!("批量提取 {} 条岗位详情", jobs.len()),
    );
    if !state.db.reserve_task(&task)? {
        return Err("已有岗位详情提取任务正在排队或运行。".into());
    }
    emit_task(&app, &task);
    let task_id = task.id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let mut task = task;
        let total = jobs.len();
        let mut succeeded = 0_usize;
        let mut failures = Vec::new();

        for (index, mut job) in jobs.into_iter().enumerate() {
            let progress = 8 + ((index as i64) * 86 / total as i64);
            update_task(
                &app,
                &db,
                &mut task,
                "running",
                progress,
                &format!("正在提取 {}/{}：{}", index + 1, total, job.title),
                None,
            );
            let input = json!({
                "jobId": &job.id,
                "knownMetadata": {
                    "title": &job.title,
                    "company": &job.company,
                    "location": &job.location,
                    "industry": &job.industry
                },
                "rawDetailText": &job.description
            });
            match llm::run_skill::<JobStructuredDetails>(
                &provider,
                skills::JOB_DETAIL_EXTRACTION,
                &input,
            )
            .await
            {
                Ok(mut details) => {
                    details.extracted_at = time::shanghai_rfc3339();
                    details.extractor_version = "job-detail-extraction@1.0.0".into();
                    job.structured_details = Some(details);
                    match db.save_job(&job) {
                        Ok(()) => succeeded += 1,
                        Err(error) => failures.push(format!("{}：{error}", job.title)),
                    }
                }
                Err(error) => failures.push(format!("{}：{error}", job.title)),
            }
        }

        let failed = total - succeeded;
        if succeeded == 0 {
            update_task(
                &app,
                &db,
                &mut task,
                "failed",
                95,
                "岗位详情批量提取失败",
                failures.first().cloned(),
            );
        } else {
            let error = if failures.is_empty() {
                None
            } else {
                Some(failures.into_iter().take(2).collect::<Vec<_>>().join("；"))
            };
            update_task(
                &app,
                &db,
                &mut task,
                "completed",
                100,
                &format!("提取完成：成功 {succeeded}，失败 {failed}"),
                error,
            );
        }
    });
    Ok(task_id)
}

#[tauri::command]
pub async fn setup_boss(
    app: AppHandle,
    state: State<'_, AppState>,
    reset_profile: Option<bool>,
) -> Result<String, String> {
    distribution::require_privacy(&state)?;
    ensure_chrome_available().await?;
    if let Some(task) = state.db.running_task("boss-login")? {
        return Ok(task.id);
    }
    let reset_profile = reset_profile.unwrap_or(false);
    let task = new_task(
        "boss-login",
        if reset_profile {
            "重新配置 BOSS 专用浏览器"
        } else {
            "配置 BOSS 专用浏览器"
        },
    );
    if !state.db.reserve_task(&task)? {
        return Err("已有 BOSS 登录任务正在排队或运行。".into());
    }
    let mut profile = state.db.boss_profile_state()?;
    profile.configured = false;
    profile.last_attempt_status = "running".into();
    profile.last_attempt_at = Some(time::shanghai_rfc3339());
    profile.last_error = None;
    if let Err(error) = state.db.save_boss_profile_state(&profile) {
        let mut failed_task = task.clone();
        failed_task.state = "failed".into();
        failed_task.progress = 100;
        failed_task.message = "无法保存 BOSS 配置状态".into();
        failed_task.recoverable_error = Some(redact(&error));
        failed_task.updated_at = time::shanghai_rfc3339();
        let _ = state.db.save_task(&failed_task);
        return Err(error);
    }
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
            20,
            if reset_profile {
                "正在重建专用 Chrome Profile，请在 5 分钟内重新登录 BOSS"
            } else {
                "正在启动专用 Chrome，请在 5 分钟内登录 BOSS"
            },
            None,
        );
        match sidecar::request(
            json!({"op":"setup_boss","params":{"loginTimeout":300,"resetProfile":reset_profile}}),
        )
        .await
        {
            Ok(value) => match serde_json::from_value::<SidecarBossOutcome>(value) {
                Ok(outcome) if outcome.login_succeeded => {
                    let now = time::shanghai_rfc3339();
                    let profile = BossProfileState {
                        configured: true,
                        configured_at: Some(now.clone()),
                        last_attempt_status: "succeeded".into(),
                        last_attempt_at: Some(now),
                        last_error: outcome.error.clone().map(|error| redact(&error)),
                    };
                    let _ = db.save_boss_profile_state(&profile);
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "completed",
                        100,
                        if outcome.cleanup_succeeded {
                            "BOSS 登录配置已完成，专用 Chrome 已自动关闭"
                        } else {
                            "配置成功，但专用 Chrome 未能自动关闭，请手动关闭"
                        },
                        if outcome.cleanup_succeeded {
                            None
                        } else {
                            outcome.error
                        },
                    );
                }
                Ok(outcome) => {
                    let error = outcome
                        .error
                        .unwrap_or_else(|| "未检测到有效的 BOSS 登录状态。".into());
                    let profile = BossProfileState {
                        configured: false,
                        configured_at: None,
                        last_attempt_status: "failed".into(),
                        last_attempt_at: Some(time::shanghai_rfc3339()),
                        last_error: Some(redact(&error)),
                    };
                    let _ = db.save_boss_profile_state(&profile);
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "failed",
                        100,
                        "BOSS 配置失败",
                        Some(error),
                    );
                }
                Err(error) => {
                    let message = format!("BOSS 配置结果格式无效：{error}");
                    let mut profile = db.boss_profile_state().unwrap_or_default();
                    profile.configured = false;
                    profile.last_attempt_status = "failed".into();
                    profile.last_attempt_at = Some(time::shanghai_rfc3339());
                    profile.last_error = Some(message.clone());
                    let _ = db.save_boss_profile_state(&profile);
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "failed",
                        100,
                        "BOSS 配置失败",
                        Some(message),
                    );
                }
            },
            Err(error) => {
                let cleanup = sidecar::request(json!({"op":"close_boss","params":{}}))
                    .await
                    .err();
                let message = cleanup
                    .map(|cleanup| format!("{error}；清理失败：{cleanup}"))
                    .unwrap_or(error);
                let mut profile = db.boss_profile_state().unwrap_or_default();
                profile.configured = false;
                profile.last_attempt_status = "failed".into();
                profile.last_attempt_at = Some(time::shanghai_rfc3339());
                profile.last_error = Some(redact(&message));
                let _ = db.save_boss_profile_state(&profile);
                update_task(
                    &app,
                    &db,
                    &mut task,
                    "failed",
                    100,
                    "BOSS 配置失败",
                    Some(message),
                );
            }
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
    distribution::require_privacy(&state)?;
    let extension = PathBuf::from(&payload.file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["pdf", "docx", "yaml", "yml"].contains(&extension.as_str()) {
        return Err("仅支持 PDF、DOCX、YAML 和 YML 文件。".into());
    }
    validate_resume_import_size(payload.content_base64.len(), None)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.content_base64.as_bytes())
        .map_err(|error| format!("简历文件编码无效：{error}"))?;
    validate_resume_import_size(payload.content_base64.len(), Some(bytes.len()))?;
    let imports = state.data_dir.join("imports");
    std::fs::create_dir_all(&imports).map_err(|error| error.to_string())?;
    let input_path = imports.join(format!("{}.{}", Uuid::new_v4(), extension));
    std::fs::write(&input_path, bytes).map_err(|error| error.to_string())?;
    let import_artifacts = ImportArtifacts::new(input_path.clone());

    let task = new_task("resume-import", &format!("解析 {}", payload.file_name));
    if !state.db.reserve_task(&task)? {
        return Err("已有简历导入任务正在排队或运行。".into());
    }
    emit_task(&app, &task);
    let task_id = task.id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let _import_artifacts = import_artifacts;
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
                    let scan_required = output.pages.iter().any(|page| page.image_path.is_some());
                    let provider = db.default_provider().ok().flatten();
                    if scan_required {
                        let Some(vision_provider) = provider.as_ref() else {
                            update_task(
                                &app,
                                &db,
                                &mut task,
                                "failed",
                                38,
                                "扫描件需要图片识别模型",
                                Some("请先在设置中配置并验证默认 AI 模型。".into()),
                            );
                            return;
                        };
                        if !vision_provider.vision_verified {
                            update_task(
                                &app,
                                &db,
                                &mut task,
                                "failed",
                                38,
                                "默认模型未通过图片能力测试",
                                Some("请在设置中重新测试支持图片的多模态模型。".into()),
                            );
                            return;
                        }
                        for page in &mut output.pages {
                            let Some(image_path) = page.image_path.as_ref() else {
                                continue;
                            };
                            update_task(
                                &app,
                                &db,
                                &mut task,
                                "running",
                                30 + (page.page_number as i64).min(10) * 2,
                                &format!("正在识别扫描页 {}", page.page_number),
                                None,
                            );
                            let image_bytes = match std::fs::read(image_path) {
                                Ok(value) => value,
                                Err(error) => {
                                    update_task(
                                        &app,
                                        &db,
                                        &mut task,
                                        "failed",
                                        40,
                                        "无法读取扫描页",
                                        Some(error.to_string()),
                                    );
                                    return;
                                }
                            };
                            let image_data_url = format!(
                                "data:image/png;base64,{}",
                                base64::engine::general_purpose::STANDARD.encode(image_bytes)
                            );
                            match llm::transcribe_resume_page(
                                vision_provider,
                                &image_data_url,
                                page.page_number,
                            )
                            .await
                            {
                                Ok(text) if !text.trim().is_empty() => page.text = text,
                                Ok(_) => {
                                    update_task(
                                        &app,
                                        &db,
                                        &mut task,
                                        "failed",
                                        44,
                                        "扫描页识别结果为空",
                                        Some(format!("第 {} 页未识别到文字。", page.page_number)),
                                    );
                                    return;
                                }
                                Err(error) => {
                                    update_task(
                                        &app,
                                        &db,
                                        &mut task,
                                        "failed",
                                        44,
                                        "扫描页识别失败",
                                        Some(error),
                                    );
                                    return;
                                }
                            }
                        }
                        output.raw_text = output
                            .pages
                            .iter()
                            .map(|page| format!("--- Page {} ---\n{}", page.page_number, page.text))
                            .collect::<Vec<_>>()
                            .join("\n\n");
                    }
                    let mut ai_extracted = false;
                    if let Some(provider) = provider {
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
                            ai_extracted = true;
                        }
                    }
                    if scan_required && !ai_extracted {
                        update_task(
                            &app,
                            &db,
                            &mut task,
                            "failed",
                            58,
                            "扫描简历结构化失败",
                            Some("图片已识别，但模型返回的简历结构无效，请重试或更换模型。".into()),
                        );
                        return;
                    }
                    for fact in &mut output.profile.facts {
                        fact.confirmed = false;
                    }
                    crate::db::ensure_resume_item_ids(&mut output.profile);
                    update_task(
                        &app,
                        &db,
                        &mut task,
                        "running",
                        84,
                        "正在建立可追溯事实清单",
                        None,
                    );
                    merge_missing_profile_facts(&mut output.profile);
                    let current = db.active_resume().ok().flatten();
                    let expected_version =
                        current.as_ref().map(|resume| resume.version).unwrap_or(0);
                    if let Some(current) = current {
                        output.profile.id = current.id;
                        output.profile.preferences = current.preferences;
                    }
                    match db.commit_resume(
                        output.profile,
                        expected_version,
                        "import",
                        &format!("导入 {}", payload.file_name),
                        None,
                        None,
                        None,
                    ) {
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
pub fn create_resume_from_template(
    state: State<'_, AppState>,
    template_id: String,
) -> Result<ResumeProfile, String> {
    if state.db.active_resume()?.is_some() {
        return Err("已有主简历，请在简历编辑器中切换结构模板。".into());
    }
    let template_id = template_id.trim();
    let group_labels: &[&str] = match template_id {
        "general" => &["专业能力", "工具与系统"],
        "ai-engineering" => &[
            "核心方向",
            "后端与数据",
            "模型与文档",
            "工程运维",
            "扩展实践",
        ],
        "data-analysis" => &[
            "数据工具",
            "数据处理",
            "分析方法",
            "可视化与报表",
            "业务分析",
        ],
        "finance-accounting" => &[
            "会计核算",
            "税务与合规",
            "预算与财务分析",
            "财务系统与办公工具",
        ],
        _ => return Err("不支持的简历模板。".into()),
    };
    let profile = ResumeProfile {
        id: Uuid::new_v4().to_string(),
        name: String::new(),
        headline: String::new(),
        email: String::new(),
        phone: String::new(),
        location: String::new(),
        website: String::new(),
        summary: String::new(),
        template_id: template_id.to_string(),
        professional_skills: group_labels
            .iter()
            .map(|label| ProfessionalSkillGroup {
                id: Uuid::new_v4().to_string(),
                label: (*label).to_string(),
                items: vec![],
            })
            .collect(),
        experiences: vec![],
        education: vec![],
        projects: vec![],
        certifications: vec![],
        facts: vec![],
        preferences: JobPreferences::default(),
        source_file_name: "空白模板".into(),
        updated_at: time::shanghai_rfc3339(),
        version: 0,
    };
    Ok(state
        .db
        .commit_resume(
            profile,
            0,
            "template",
            &format!("创建 {template_id} 空白简历"),
            None,
            None,
            None,
        )?
        .resume)
}

#[tauri::command]
pub fn save_resume(
    state: State<'_, AppState>,
    resume: ResumeProfile,
) -> Result<ResumeProfile, String> {
    let expected_version = resume.version;
    Ok(state
        .db
        .commit_resume(
            resume,
            expected_version,
            "manual",
            "手工保存主简历",
            None,
            None,
            None,
        )?
        .resume)
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
    resume.updated_at = time::shanghai_rfc3339();
    state.db.save_resume(&resume)?;
    Ok(resume)
}

#[tauri::command]
pub async fn generate_greeting(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<String, String> {
    distribution::require_privacy(&state)?;
    let mut job = state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let fallback = scoring::fallback_greeting(&job, &resume);
    let confirmed_facts = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .collect::<Vec<_>>();
    let mut greeting = if let Some(provider) = state.db.default_provider()? {
        let input = json!({
            "job": {"id":job.id,"title":job.title,"company":job.company,"skills":job.skills},
            "resumeFacts":confirmed_facts,
            "maxChineseCharacters":60
        });
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
pub async fn render_resume(
    state: State<'_, AppState>,
    output_path: String,
    color_theme: ResumeColorTheme,
) -> Result<RenderResult, String> {
    let resume = state
        .db
        .active_resume()?
        .ok_or_else(|| "请先导入主简历。".to_string())?;
    let output_path = PathBuf::from(output_path);
    if output_path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("pdf"))
    {
        return Err("导出路径必须使用 .pdf 扩展名。".into());
    }
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("无法创建导出目录：{error}"))?;
    }
    let value = sidecar::request(
        json!({"op":"render_resume","params":{"profile":resume,"outputPath":output_path,"colorTheme":color_theme}}),
    )
    .await?;
    let rendered_path = value
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_else(|| output_path.to_str().unwrap_or_default())
        .to_string();
    Ok(RenderResult {
        path: rendered_path,
        file_name: output_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("简历.pdf")
            .to_string(),
    })
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
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
