use super::*;

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
pub fn list_resume_variants(
    state: State<'_, AppState>,
) -> Result<Vec<ResumeVariantSummary>, String> {
    state.db.list_resume_variants()
}

#[tauri::command]
pub fn get_resume_variant(
    state: State<'_, AppState>,
    variant_id: String,
) -> Result<ResumeVariantDetail, String> {
    state
        .db
        .get_resume_variant(&variant_id)?
        .ok_or_else(|| "variant_not_found: 岗位版本不存在。".into())
}

#[tauri::command]
pub fn create_resume_variant(
    state: State<'_, AppState>,
    job_id: String,
    expected_resume_version: i64,
) -> Result<ResumeVariantDetail, String> {
    state
        .db
        .create_resume_variant(&job_id, expected_resume_version)
}

#[tauri::command]
pub fn save_resume_variant(
    state: State<'_, AppState>,
    variant_id: String,
    resume: ResumeProfile,
    expected_version: i64,
) -> Result<ResumeVariantCommitResult, String> {
    state.db.commit_resume_variant(
        &variant_id,
        resume,
        expected_version,
        "variant-manual",
        "手工保存岗位版本",
        None,
        None,
    )
}

#[tauri::command]
pub fn delete_resume_variant(
    state: State<'_, AppState>,
    variant_id: String,
) -> Result<i64, String> {
    state.db.delete_resume_variant(&variant_id)
}

#[tauri::command]
pub fn preview_resume_variant_rebase(
    state: State<'_, AppState>,
    variant_id: String,
) -> Result<ResumeRebasePreview, String> {
    state.db.preview_resume_variant_rebase(&variant_id)
}

#[tauri::command]
pub fn apply_resume_variant_rebase(
    state: State<'_, AppState>,
    variant_id: String,
    expected_variant_version: i64,
    expected_master_version: i64,
    resolutions: Vec<ResumeRebaseResolution>,
) -> Result<ResumeVariantCommitResult, String> {
    state.db.apply_resume_variant_rebase(
        &variant_id,
        expected_variant_version,
        expected_master_version,
        &resolutions,
    )
}

#[tauri::command]
pub fn restore_resume_variant_version(
    state: State<'_, AppState>,
    variant_id: String,
    version_id: String,
    expected_version: i64,
) -> Result<ResumeVariantCommitResult, String> {
    state
        .db
        .restore_resume_variant_version(&variant_id, &version_id, expected_version)
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
    target: Option<ResumeTargetRef>,
) -> Result<RenderResult, String> {
    let resume = match target.as_ref() {
        Some(target) if target.kind == "variant" => state
            .db
            .get_resume_variant(&target.id)?
            .map(|value| value.profile)
            .ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?,
        Some(target) if target.kind == "master" => {
            let resume = state
                .db
                .active_resume()?
                .ok_or_else(|| "请先导入主简历。".to_string())?;
            if target.id != resume.id {
                return Err("version_conflict: 当前主简历已变化，请刷新后重试。".into());
            }
            resume
        }
        None => state
            .db
            .active_resume()?
            .ok_or_else(|| "请先导入主简历。".to_string())?,
        Some(_) => return Err("invalid_request: 不支持的简历目标。".into()),
    };
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
