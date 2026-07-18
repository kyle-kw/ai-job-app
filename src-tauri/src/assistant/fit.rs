use super::*;

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
