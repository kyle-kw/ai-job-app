use super::*;

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

pub(super) fn resolve_resume_market_context(
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

pub(super) fn validate_resume_chat_context_mode(
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

pub(super) fn validate_market_edit_evidence(
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

pub(super) fn resume_edit_introduces_factual_content(before: &Value, after: &Value) -> bool {
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
