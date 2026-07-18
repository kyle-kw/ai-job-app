use super::*;

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
        last_search_spec: state.db.last_search_spec()?,
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
pub fn list_job_filter_options(state: State<'_, AppState>) -> Result<JobFilterOptions, String> {
    state.db.list_job_filter_options()
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
    query: Option<JobQuery>,
) -> Result<RenderResult, String> {
    let jobs = match query {
        Some(query) => state.db.jobs_for_query(&query)?,
        None => state.db.list_jobs()?,
    };
    if jobs.is_empty() {
        return Err("当前范围暂无岗位可导出。".into());
    }
    let output_path = validate_export_path(output_path, "json", "岗位 JSON")?;
    let payload = serialize_jobs_json(&jobs)?;
    std::fs::write(&output_path, payload).map_err(|error| format!("无法导出岗位 JSON：{error}"))?;
    render_result(output_path)
}

pub(super) fn serialize_jobs_json(jobs: &[Job]) -> Result<Vec<u8>, String> {
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
    let competitiveness =
        crate::assistant::effective_report_competitiveness(&state.db, &keyword_keys)?;
    let mut html = analytics::append_decision_sections(
        analytics::render_html(&report),
        &report,
        competitiveness.as_ref(),
    );
    if let Some(preparation) =
        crate::assistant::fresh_interview_preparation(&state.db, &keyword_keys)?
    {
        html = analytics::append_interview_preparation(html, &preparation);
    }
    std::fs::write(&output_path, html.as_bytes())
        .map_err(|error| format!("无法导出岗位数据报告：{error}"))?;
    render_result(output_path)
}

pub(super) fn validate_export_path(
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
    let scrape_runs = db.list_report_scrape_runs()?;
    Ok(analytics::build_report_for_keywords_with_runs(
        &jobs,
        selected_keywords,
        &scrape_runs,
    ))
}

pub(super) fn streamed_job_key(job: &Job) -> String {
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
