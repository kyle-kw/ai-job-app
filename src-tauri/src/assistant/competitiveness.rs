use super::*;

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

pub(super) fn validate_model_report_competitiveness(
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
