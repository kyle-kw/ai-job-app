use super::jobs::streamed_job_key;
use super::*;

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
    if !is_supported_scrape_city(&spec.city) {
        return Err("请选择城市列表中的有效城市。".into());
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
    if !state.db.reserve_scrape_task(&task, &spec)? {
        return Err("已有同类抓取任务正在排队或运行。".into());
    }
    emit_task(&app, &task);
    let task_id = task.id.clone();
    let scrape_run_id = Uuid::new_v4().to_string();
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
        let scrape_run_id_for_events = scrape_run_id.clone();
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
                    db_for_events.upsert_scrape_detail_job_for_run(
                        job,
                        &keyword_for_events,
                        &scrape_run_id_for_events,
                    )?;
                }
            } else {
                let job_key = streamed_job_key(&job);
                let already_seen = streamed_for_events
                    .lock()
                    .map_err(|_| "岗位抓取统计状态不可用".to_string())?
                    .0
                    .contains(&job_key);
                let stats = db_for_events.upsert_scrape_list_job_for_run(
                    job,
                    &keyword_for_events,
                    &scrape_run_id_for_events,
                )?;
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
                    let report_markdown = batch.report_markdown;
                    let resolved_city = batch.resolved_city;
                    let mut detail_summary = batch.detail_summary;
                    let reconcile_result = report_jobs.iter().try_for_each(|job| {
                        let job_key = streamed_job_key(job);
                        let already_streamed = streamed
                            .lock()
                            .map_err(|_| "岗位抓取统计状态不可用".to_string())?
                            .0
                            .contains(&job_key);
                        if !already_streamed {
                            let stats = db.upsert_scrape_list_job_for_run(
                                job.clone(),
                                &spec.keyword,
                                &scrape_run_id,
                            )?;
                            let mut streamed = streamed
                                .lock()
                                .map_err(|_| "岗位抓取统计状态不可用".to_string())?;
                            if streamed.0.insert(job_key) {
                                streamed.1 += stats.inserted;
                                streamed.2 += stats.updated;
                            }
                        }
                        if !job.description.trim().is_empty() {
                            db.upsert_scrape_detail_job_for_run(
                                job.clone(),
                                &spec.keyword,
                                &scrape_run_id,
                            )?;
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
                            let mut persisted_ids = HashSet::new();
                            let persisted_jobs = report_jobs
                                .iter()
                                .filter(|job| persisted_ids.insert(job.id.clone()))
                                .filter_map(|job| db.get_job(&job.id).ok().flatten())
                                .collect::<Vec<_>>();
                            let sample = analytics::build_scrape_sample(&persisted_jobs);
                            if let Some(summary) = detail_summary.as_mut() {
                                if summary.total == 0 {
                                    summary.total = sample.total_jobs;
                                }
                            }
                            let run = ScrapeRun {
                                id: scrape_run_id.clone(),
                                keyword: spec.keyword.clone(),
                                city: spec.city.clone(),
                                total_seen: sample.total_jobs,
                                inserted,
                                updated,
                                started_at: task.created_at.clone(),
                                completed_at: Some(now),
                                report_markdown,
                                search_spec: Some(spec.clone()),
                                resolved_city,
                                detail_summary,
                                sample: Some(sample),
                            };
                            match db.save_scrape_run(&run) {
                                Ok(()) => update_task(
                                    &app,
                                    &db,
                                    &mut task,
                                    "completed",
                                    100,
                                    &format!("完成：新增 {inserted}，更新 {updated}"),
                                    None,
                                ),
                                Err(error) => update_task(
                                    &app,
                                    &db,
                                    &mut task,
                                    "failed",
                                    96,
                                    "岗位已写入，但抓取样本摘要保存失败",
                                    Some(error),
                                ),
                            }
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
