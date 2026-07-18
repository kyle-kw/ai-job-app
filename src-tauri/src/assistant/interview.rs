use super::*;

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
