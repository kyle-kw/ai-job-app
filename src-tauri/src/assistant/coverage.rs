use super::*;

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
