use crate::analytics;
use crate::models::{
    AiProviderConfig, AppSettings, BossProfileState, InterviewPreparation, Job, JobFilterOptions,
    JobFilterSkillOption, JobOption, JobPage, JobQuery, ReportCompetitivenessAnalysis,
    ReportKeyword, ResumeCommitResult, ResumeCoverageReport, ResumeEducation, ResumeProfile,
    ResumeRebaseChange, ResumeRebasePreview, ResumeRebaseResolution, ResumeVariantCommitResult,
    ResumeVariantDetail, ResumeVariantSummary, ResumeVersionDetail, ResumeVersionSummary,
    ScrapeRun, SearchSpec, TaskRun,
};
use crate::time;
use base64::Engine;
use rusqlite::backup::Backup;
use rusqlite::functions::FunctionFlags;
use rusqlite::types::Value as SqlValue;
use rusqlite::{
    params, params_from_iter, Connection, OpenFlags, OptionalExtension, Transaction,
    TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

mod providers;

pub const CURRENT_SCHEMA_VERSION: i64 = 8;

const HISTORICAL_KEYWORD_KEY: &str = "__historical_unclassified__";
const HISTORICAL_KEYWORD_LABEL: &str = "历史未分类";

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
    gate: Arc<DatabaseGate>,
}

#[derive(Default)]
struct DatabaseGate {
    state: Mutex<DatabaseGateState>,
    idle: Condvar,
}

#[derive(Default)]
struct DatabaseGateState {
    active_connections: usize,
    maintenance: bool,
    unavailable: bool,
}

pub(crate) struct DatabaseConnection {
    connection: Connection,
    gate: Arc<DatabaseGate>,
}

pub(crate) struct DatabaseMaintenanceGuard {
    path: PathBuf,
    gate: Arc<DatabaseGate>,
    released: bool,
}

impl Deref for DatabaseConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for DatabaseConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

impl Drop for DatabaseConnection {
    fn drop(&mut self) {
        if let Ok(mut state) = self.gate.state.lock() {
            state.active_connections = state.active_connections.saturating_sub(1);
            if state.active_connections == 0 {
                self.gate.idle.notify_all();
            }
        }
    }
}

impl DatabaseMaintenanceGuard {
    pub(crate) fn has_active_tasks(&self) -> Result<bool, String> {
        let connection = Database::open_connection(&self.path)?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_runs WHERE json_extract(payload_json,'$.state') IN ('queued','running'))",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())
    }

    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        if !self.path.exists() {
            return Ok(());
        }
        let connection = Database::open_connection(&self.path)?;
        let (busy, _, _): (i64, i64, i64) = connection
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|error| format!("cannot checkpoint database: {error}"))?;
        if busy != 0 {
            return Err("cannot checkpoint database: database is busy".into());
        }
        Ok(())
    }

    pub(crate) fn backup_to(&self, destination: &Path) -> Result<(), String> {
        let source = Database::open_connection(&self.path)?;
        Database::backup_connection(&source, destination)
    }

    pub(crate) fn disable_until_restart(mut self) -> Result<(), String> {
        let mut state = self
            .gate
            .state
            .lock()
            .map_err(|_| "database gate is poisoned".to_string())?;
        state.unavailable = true;
        state.maintenance = false;
        self.released = true;
        self.gate.idle.notify_all();
        Ok(())
    }
}

impl Drop for DatabaseMaintenanceGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        if let Ok(mut state) = self.gate.state.lock() {
            state.maintenance = false;
            self.gate.idle.notify_all();
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UpsertStats {
    pub inserted: i64,
    pub updated: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpsertMode {
    Generic,
    ScrapeList,
    ScrapeDetail,
}

#[derive(Debug, Clone)]
pub struct InterviewPreparationCacheRecord {
    pub cache_key: String,
    pub scope_key: String,
    pub dataset_hash: String,
    pub resume_id: Option<String>,
    pub resume_version: Option<i64>,
    pub provider_fingerprint: String,
    pub skill_version: String,
    pub generated_at: String,
    pub preparation: InterviewPreparation,
}

#[derive(Debug, Clone)]
pub struct ReportCompetitivenessCacheRecord {
    pub cache_key: String,
    pub scope_key: String,
    pub dataset_hash: String,
    pub resume_id: String,
    pub resume_version: i64,
    pub provider_fingerprint: String,
    pub skill_version: String,
    pub generated_at: String,
    pub analysis: ReportCompetitivenessAnalysis,
}

const JOB_PAGE_SIZE: usize = 50;
const JOB_SALARY_SORT_SQL: &str = "CASE WHEN salary_min IS NULL OR salary_max IS NULL THEN NULL WHEN salary_max>=1.0e308 THEN salary_min ELSE ((salary_min+salary_max)/2.0) END";

fn normalize_job_sort(value: &str) -> &'static str {
    match value.trim() {
        "recent" => "recent",
        "salary-desc" => "salary-desc",
        _ => "recommended",
    }
}

fn job_order_by(sort: &str) -> String {
    match sort {
        "recent" => "last_seen DESC,COALESCE(fit_score,0) DESC,id ASC".into(),
        "salary-desc" => format!(
            "({JOB_SALARY_SORT_SQL} IS NULL) ASC,{JOB_SALARY_SORT_SQL} DESC,COALESCE(fit_score,0) DESC,last_seen DESC,id ASC"
        ),
        _ => "COALESCE(fit_score,0) DESC,last_seen DESC,id ASC".into(),
    }
}

#[derive(Debug)]
struct JobQueryMetadata {
    search_text: String,
    salary_min: Option<f64>,
    salary_max: Option<f64>,
    company_scale_code: String,
    city: String,
    is_new: i64,
    fit_score: Option<i64>,
    has_description: i64,
    has_structured_details: i64,
}

impl JobQueryMetadata {
    fn from_job(job: &Job) -> Self {
        let (salary_min, salary_max) = parse_salary_range(&job.salary)
            .map(|(minimum, maximum)| (Some(minimum), Some(maximum)))
            .unwrap_or((None, None));
        Self {
            search_text: format!("{} {} {}", job.title, job.company, job.skills.join(" "))
                .to_lowercase(),
            salary_min,
            salary_max,
            company_scale_code: normalize_company_scale_code(&job.company_scale),
            city: job_city(&job.location),
            is_new: i64::from(job.is_new),
            fit_score: job.fit.as_ref().map(|fit| fit.overall_score),
            has_description: i64::from(!job.description.trim().is_empty()),
            has_structured_details: i64::from(job.structured_details.is_some()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JobCursor {
    sort: String,
    score: i64,
    last_seen: String,
    salary_mid: Option<f64>,
    id: String,
}

mod core;
mod jobs;
mod migrations;
mod reports;
mod resumes;
fn upsert_job_in_transaction(
    transaction: &Transaction<'_>,
    mut job: Job,
    preserve_is_new_on_update: bool,
    mode: UpsertMode,
    keyword: Option<&str>,
    scrape_run_id: Option<&str>,
) -> Result<UpsertStats, String> {
    let fingerprint = fingerprint(&job.company, &job.title, &job.location);
    let external_key = if job.external_id.trim().is_empty() {
        format!("fp:{fingerprint}")
    } else {
        job.external_id.clone()
    };
    let existing_json: Option<String> = transaction
        .query_row(
            "SELECT payload_json FROM jobs WHERE source = ?1 AND external_key = ?2",
            params![job.source, external_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let mut stats = UpsertStats::default();
    if let Some(existing_json) = existing_json {
        let existing: Job =
            serde_json::from_str(&existing_json).map_err(|error| error.to_string())?;
        job.id = existing.id;
        job.first_seen = existing.first_seen;
        job.fit = existing.fit;
        job.greeting = existing.greeting;
        job.patches = existing.patches;
        job.structured_details = existing.structured_details;
        job.is_new = preserve_is_new_on_update && existing.is_new;

        match mode {
            UpsertMode::ScrapeList => {
                if !existing.description.trim().is_empty() {
                    job.description = existing.description;
                    job.skills = existing.skills;
                    job.welfare = existing.welfare;
                }
            }
            UpsertMode::ScrapeDetail => {
                if job.skills.is_empty() {
                    job.skills = existing.skills;
                }
                if job.welfare.is_empty() {
                    job.welfare = existing.welfare;
                }
            }
            UpsertMode::Generic => {}
        }
        stats.updated = 1;
    } else {
        job.is_new = true;
        stats.inserted = 1;
    }
    let payload = serde_json::to_string(&job).map_err(|error| error.to_string())?;
    let meta = JobQueryMetadata::from_job(&job);
    transaction
        .execute(
            r#"INSERT INTO jobs(id,source,external_key,fingerprint,title,company,location,first_seen,last_seen,payload_json,search_text,salary_min,salary_max,company_scale_code,city,query_is_new,fit_score,has_description,has_structured_details)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)
               ON CONFLICT(source, external_key) DO UPDATE SET
                 fingerprint=excluded.fingerprint, title=excluded.title, company=excluded.company,
                 location=excluded.location,last_seen=excluded.last_seen,payload_json=excluded.payload_json,
                 search_text=excluded.search_text,salary_min=excluded.salary_min,salary_max=excluded.salary_max,
                 company_scale_code=excluded.company_scale_code,city=excluded.city,query_is_new=excluded.query_is_new,
                 fit_score=excluded.fit_score,has_description=excluded.has_description,
                 has_structured_details=excluded.has_structured_details"#,
            params![job.id,job.source,external_key,fingerprint,job.title,job.company,job.location,job.first_seen,job.last_seen,payload,meta.search_text,meta.salary_min,meta.salary_max,meta.company_scale_code,meta.city,meta.is_new,meta.fit_score,meta.has_description,meta.has_structured_details],
        )
        .map_err(|error| error.to_string())?;
    if let Some(run_id) = scrape_run_id {
        transaction
            .execute(
                "INSERT INTO job_scrape_runs(run_id,job_id,was_inserted) VALUES (?1,?2,?3) \
                 ON CONFLICT(run_id,job_id) DO UPDATE SET was_inserted=MAX(was_inserted,excluded.was_inserted)",
                params![run_id, job.id, stats.inserted],
            )
            .map_err(|error| error.to_string())?;
    }
    if let Some(keyword) = keyword {
        associate_job_keyword(transaction, &job, keyword)?;
    }
    Ok(stats)
}

fn associate_job_keyword(
    transaction: &Transaction<'_>,
    job: &Job,
    keyword: &str,
) -> Result<(), String> {
    let requested_label = normalize_keyword_label(keyword);
    let keyword_key = normalize_keyword_key(&requested_label);
    if keyword_key.is_empty() {
        return Err("岗位关键词不能为空。".into());
    }
    let keyword_label = transaction
        .query_row(
            "SELECT keyword_label FROM job_keywords WHERE keyword_key=?1 ORDER BY last_seen DESC LIMIT 1",
            [&keyword_key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or(requested_label);
    let seen_at = if job.last_seen.trim().is_empty() {
        time::shanghai_rfc3339()
    } else {
        job.last_seen.clone()
    };
    let first_seen = if job.first_seen.trim().is_empty() {
        seen_at.clone()
    } else {
        job.first_seen.clone()
    };
    transaction
        .execute(
            r#"INSERT INTO job_keywords(job_id,keyword_key,keyword_label,first_seen,last_seen)
               VALUES (?1,?2,?3,?4,?5)
               ON CONFLICT(job_id,keyword_key) DO UPDATE SET
                 keyword_label=excluded.keyword_label,last_seen=excluded.last_seen"#,
            params![job.id, keyword_key, keyword_label, first_seen, seen_at],
        )
        .map_err(|error| error.to_string())?;
    if keyword_key != HISTORICAL_KEYWORD_KEY {
        transaction
            .execute(
                "DELETE FROM job_keywords WHERE job_id=?1 AND keyword_key=?2",
                params![job.id, HISTORICAL_KEYWORD_KEY],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn escaped_like_pattern(value: &str) -> String {
    format!(
        "%{}%",
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

fn job_city(location: &str) -> String {
    location
        .split('·')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn push_job_condition(
    conditions: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    condition: &str,
    value: SqlValue,
) {
    values.push(value);
    conditions.push(condition.replace('?', &format!("?{}", values.len())));
}

fn mark_latest_new_jobs(connection: &Connection, jobs: &mut [Job]) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT job_id FROM job_scrape_runs \
             WHERE was_inserted=1 AND run_id=( \
                 SELECT id FROM scrape_runs \
                 WHERE json_extract(payload_json,'$.completedAt') IS NOT NULL \
                 ORDER BY started_at DESC,id DESC LIMIT 1 \
             )",
        )
        .map_err(|error| error.to_string())?;
    let latest_new_ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|error| error.to_string())?;
    for job in jobs {
        job.is_new = latest_new_ids.contains(&job.id);
    }
    Ok(())
}

fn job_query_where(
    query: &JobQuery,
    include_cursor: bool,
) -> Result<(String, Vec<SqlValue>), String> {
    let mut conditions = vec!["1=1".to_string()];
    let mut values = Vec::<SqlValue>::new();
    let text = query.query.trim().to_lowercase();
    if !text.is_empty() {
        push_job_condition(
            &mut conditions,
            &mut values,
            "search_text LIKE ? ESCAPE '\\'",
            escaped_like_pattern(&text).into(),
        );
    }
    if query.min_score > 0 {
        push_job_condition(
            &mut conditions,
            &mut values,
            "COALESCE(fit_score,0)>=?",
            query.min_score.into(),
        );
    }
    if query.only_new {
        conditions.push(
            "EXISTS ( \
                SELECT 1 FROM job_scrape_runs AS latest_new \
                WHERE latest_new.job_id=jobs.id AND latest_new.was_inserted=1 \
                  AND latest_new.run_id=( \
                      SELECT id FROM scrape_runs \
                      WHERE json_extract(payload_json,'$.completedAt') IS NOT NULL \
                      ORDER BY started_at DESC,id DESC LIMIT 1 \
                  ) \
            )"
            .into(),
        );
    }
    if !query.company_scale.trim().is_empty() {
        push_job_condition(
            &mut conditions,
            &mut values,
            "company_scale_code=?",
            query.company_scale.trim().to_string().into(),
        );
    }
    if !query.city.trim().is_empty() {
        push_job_condition(
            &mut conditions,
            &mut values,
            "city=?",
            query.city.trim().to_string().into(),
        );
    }
    if query.missing_description {
        conditions.push("has_description=0".into());
    }
    let keyword_keys = query
        .keyword_keys
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if !keyword_keys.is_empty() {
        let placeholders = keyword_keys
            .iter()
            .map(|value| {
                values.push((*value).to_string().into());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(",");
        conditions.push(format!(
            "EXISTS(SELECT 1 FROM job_keywords report_keywords WHERE report_keywords.job_id=jobs.id AND report_keywords.keyword_key IN ({placeholders}))"
        ));
    }
    for skill in query
        .skills
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
    {
        push_job_condition(
            &mut conditions,
            &mut values,
            "report_has_skill(payload_json, ?)=1",
            skill.to_string().into(),
        );
    }
    if !query.experience.trim().is_empty() {
        push_job_condition(
            &mut conditions,
            &mut values,
            "trim(json_extract(payload_json,'$.experience'))=?",
            query.experience.trim().to_string().into(),
        );
    }
    let salary_mid = "((salary_min+salary_max)/2.0)";
    match query.salary_band.trim() {
        "under-15" => conditions.push(format!("salary_min IS NOT NULL AND salary_max IS NOT NULL AND {salary_mid}<15")),
        "15-25" => conditions.push(format!("salary_min IS NOT NULL AND salary_max IS NOT NULL AND {salary_mid}>=15 AND {salary_mid}<25")),
        "25-35" => conditions.push(format!("salary_min IS NOT NULL AND salary_max IS NOT NULL AND {salary_mid}>=25 AND {salary_mid}<35")),
        "35-50" => conditions.push(format!("salary_min IS NOT NULL AND salary_max IS NOT NULL AND {salary_mid}>=35 AND {salary_mid}<50")),
        "50-plus" => conditions.push(format!("salary_min IS NOT NULL AND salary_max IS NOT NULL AND {salary_mid}>=50")),
        _ => {}
    }
    if let Some((minimum, maximum)) = salary_filter_range(query.salary.trim()) {
        if maximum.is_finite() {
            values.push(maximum.into());
            let max_index = values.len();
            values.push(minimum.into());
            let min_index = values.len();
            conditions.push(format!(
                "salary_min<=?{max_index} AND salary_max>=?{min_index}"
            ));
        } else {
            push_job_condition(
                &mut conditions,
                &mut values,
                "salary_max>=?",
                minimum.into(),
            );
        }
    }
    if include_cursor {
        if let Some(encoded) = query.cursor.as_deref() {
            let cursor = decode_job_cursor(encoded)?;
            let sort = normalize_job_sort(&query.sort);
            if cursor.sort != sort {
                return Err("Job page cursor does not match the requested sort.".into());
            }
            conditions.push(job_cursor_condition(&mut values, cursor, sort)?);
        }
    }
    Ok((conditions.join(" AND "), values))
}

fn job_cursor_condition(
    values: &mut Vec<SqlValue>,
    cursor: JobCursor,
    sort: &str,
) -> Result<String, String> {
    let mut bind = |value: SqlValue| {
        values.push(value);
        values.len()
    };
    if sort == "recent" {
        let seen_less = bind(cursor.last_seen.clone().into());
        let seen_equal = bind(cursor.last_seen.into());
        let score_less = bind(cursor.score.into());
        let score_equal = bind(cursor.score.into());
        let id_after = bind(cursor.id.into());
        return Ok(format!(
            "(last_seen<?{seen_less} OR (last_seen=?{seen_equal} AND (COALESCE(fit_score,0)<?{score_less} OR (COALESCE(fit_score,0)=?{score_equal} AND id>?{id_after}))))"
        ));
    }
    if sort == "salary-desc" {
        let score_less = bind(cursor.score.into());
        let score_equal = bind(cursor.score.into());
        let seen_less = bind(cursor.last_seen.clone().into());
        let seen_equal = bind(cursor.last_seen.into());
        let id_after = bind(cursor.id.into());
        let ties = format!(
            "COALESCE(fit_score,0)<?{score_less} OR (COALESCE(fit_score,0)=?{score_equal} AND (last_seen<?{seen_less} OR (last_seen=?{seen_equal} AND id>?{id_after})))"
        );
        return Ok(if let Some(salary_mid) = cursor.salary_mid {
            let salary_less = bind(salary_mid.into());
            let salary_equal = bind(salary_mid.into());
            format!(
                "({JOB_SALARY_SORT_SQL} IS NULL OR ({JOB_SALARY_SORT_SQL}<?{salary_less} OR ({JOB_SALARY_SORT_SQL}=?{salary_equal} AND ({ties}))))"
            )
        } else {
            format!("({JOB_SALARY_SORT_SQL} IS NULL AND ({ties}))")
        });
    }
    let score_less = bind(cursor.score.into());
    let score_equal = bind(cursor.score.into());
    let seen_less = bind(cursor.last_seen.clone().into());
    let seen_equal = bind(cursor.last_seen.into());
    let id_after = bind(cursor.id.into());
    Ok(format!(
        "(COALESCE(fit_score,0)<?{score_less} OR (COALESCE(fit_score,0)=?{score_equal} AND (last_seen<?{seen_less} OR (last_seen=?{seen_equal} AND id>?{id_after}))))"
    ))
}

fn encode_job_cursor(cursor: &JobCursor) -> Result<String, String> {
    let json = serde_json::to_vec(cursor).map_err(|error| error.to_string())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json))
}

fn decode_job_cursor(value: &str) -> Result<JobCursor, String> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| "Invalid job page cursor.".to_string())?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid job page cursor.".to_string())
}

fn salary_filter_range(code: &str) -> Option<(f64, f64)> {
    Some(match code {
        "402" => (0.0, 3.0),
        "403" => (3.0, 5.0),
        "404" => (5.0, 10.0),
        "405" => (10.0, 20.0),
        "406" => (20.0, 50.0),
        "407" => (50.0, f64::INFINITY),
        _ => return None,
    })
}

fn parse_salary_range(value: &str) -> Option<(f64, f64)> {
    static RANGE: OnceLock<regex::Regex> = OnceLock::new();
    static SINGLE: OnceLock<regex::Regex> = OnceLock::new();
    let normalized = value.replace(',', "");
    if normalized.trim().is_empty()
        || normalized.contains("面议")
        || normalized.to_lowercase().contains("negotiable")
    {
        return None;
    }
    let range = RANGE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(\d+(?:\.\d+)?)\s*(?:k|千)?\s*(?:-|~|–|—|至)\s*(\d+(?:\.\d+)?)\s*(?:k|千)",
        )
        .expect("salary range regex")
    });
    if let Some(captures) = range.captures(&normalized) {
        let left = captures.get(1)?.as_str().parse::<f64>().ok()?;
        let right = captures.get(2)?.as_str().parse::<f64>().ok()?;
        return Some((left.min(right), left.max(right)));
    }
    let single = SINGLE.get_or_init(|| {
        regex::Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(?:k|千)").expect("salary regex")
    });
    let amount = single
        .captures(&normalized)?
        .get(1)?
        .as_str()
        .parse::<f64>()
        .ok()?;
    if normalized.contains("以下") || normalized.contains("以内") {
        Some((0.0, amount))
    } else if normalized.contains("以上") || normalized.contains('+') {
        Some((amount, f64::INFINITY))
    } else {
        Some((amount, amount))
    }
}

fn normalize_company_scale_code(value: &str) -> String {
    let normalized = value
        .replace([' ', ',', '，'], "")
        .replace(['–', '—', '~', '至'], "-");
    if matches!(
        normalized.as_str(),
        "301" | "302" | "303" | "304" | "305" | "306"
    ) {
        return normalized;
    }
    for (needle, code) in [
        ("20-99", "302"),
        ("20-100", "302"),
        ("100-499", "303"),
        ("100-500", "303"),
        ("500-999", "304"),
        ("500-1000", "304"),
        ("1000-9999", "305"),
        ("1000-10000", "305"),
    ] {
        if normalized.contains(needle) {
            return code.into();
        }
    }
    if normalized.contains("10000") || normalized.contains("1万人") || normalized.contains("万人")
    {
        "306".into()
    } else if normalized.contains("0-20") || normalized.contains("20人以下") {
        "301".into()
    } else {
        String::new()
    }
}

pub fn normalize_keyword_key(value: &str) -> String {
    normalize_keyword_label(value).to_lowercase()
}

fn normalize_keyword_label(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_keyword_keys(values: &[String]) -> Vec<String> {
    let mut keys = values
        .iter()
        .map(|value| normalize_keyword_key(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys
}

fn default_xiaomi_provider() -> AiProviderConfig {
    AiProviderConfig {
        id: "provider-xiaomi".into(),
        kind: "xiaomi".into(),
        name: "默认模型 · 小米 MiMo".into(),
        base_url: "https://token-plan-sgp.xiaomimimo.com/v1".into(),
        model: "mimo-v2.5".into(),
        allow_insecure_http: false,
        api_key: None,
        api_key_ref: None,
        is_default: true,
        verified: false,
        vision_verified: false,
        last_tested_at: None,
        last_test_error: None,
    }
}

fn default_custom_provider() -> AiProviderConfig {
    AiProviderConfig {
        id: "provider-custom".into(),
        kind: "custom".into(),
        name: "自定义 OpenAI 兼容服务".into(),
        base_url: String::new(),
        model: String::new(),
        allow_insecure_http: false,
        api_key: None,
        api_key_ref: None,
        is_default: false,
        verified: false,
        vision_verified: false,
        last_tested_at: None,
        last_test_error: None,
    }
}

pub fn ensure_resume_item_ids(resume: &mut ResumeProfile) {
    if resume.template_id.trim().is_empty() {
        resume.template_id = "ai-engineering".into();
    }
    for group in &mut resume.professional_skills {
        if group.id.trim().is_empty() {
            group.id = uuid::Uuid::new_v4().to_string();
        }
    }
    for experience in &mut resume.experiences {
        if experience.id.trim().is_empty() {
            experience.id = uuid::Uuid::new_v4().to_string();
        }
        normalize_date_pair(&mut experience.start_date, &mut experience.end_date);
    }
    for education in &mut resume.education {
        if education.id.trim().is_empty() {
            education.id = uuid::Uuid::new_v4().to_string();
        }
        normalize_date_pair(&mut education.start_date, &mut education.end_date);
        normalize_education_degree(education);
    }
    for project in &mut resume.projects {
        if project.id.trim().is_empty() {
            project.id = uuid::Uuid::new_v4().to_string();
        }
        normalize_date_pair(&mut project.start_date, &mut project.end_date);
    }
    for certification in &mut resume.certifications {
        if certification.id.trim().is_empty() {
            certification.id = uuid::Uuid::new_v4().to_string();
        }
    }
    for fact in &mut resume.facts {
        if fact.id.trim().is_empty() {
            fact.id = uuid::Uuid::new_v4().to_string();
        }
    }
}

fn split_date_range(value: &str) -> Option<(String, String)> {
    let expression = regex::Regex::new(
        r"(?i)^\s*(\d{4}(?:[./\-\u{5e74}]\d{1,2}(?:\u{6708})?)?)\s*(?:-|\u{2013}|\u{2014}|\u{81f3}|\u{5230})\s*(\d{4}(?:[./\-\u{5e74}]\d{1,2}(?:\u{6708})?)?|\u{81f3}\u{4eca}|\u{73b0}\u{5728}|present)\s*$",
    )
    .ok()?;
    let captures = expression.captures(value)?;
    Some((
        captures.get(1)?.as_str().trim().to_string(),
        captures.get(2)?.as_str().trim().to_string(),
    ))
}

fn clean_date(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character: char| {
            matches!(character, '-' | '\u{2013}' | '\u{2014}') || character.is_whitespace()
        })
        .to_string()
}

pub fn normalize_date_pair(start: &mut String, end: &mut String) {
    let start_value = clean_date(start);
    let end_value = clean_date(end);
    if start_value.is_empty() {
        if let Some((range_start, range_end)) = split_date_range(&end_value) {
            *start = range_start;
            *end = range_end;
            return;
        }
    }
    if end_value.is_empty() {
        if let Some((range_start, range_end)) = split_date_range(&start_value) {
            *start = range_start;
            *end = range_end;
            return;
        }
    }
    *start = start_value;
    *end = end_value;
}

fn normalize_education_degree(education: &mut ResumeEducation) {
    let raw = education.degree.trim().to_string();
    let detail = education.degree_detail.trim().to_string();
    if raw.is_empty() {
        education.degree.clear();
        education.degree_detail = detail;
    } else if raw.contains("博士") {
        education.degree = "博士".into();
        education.degree_detail.clear();
    } else if raw.contains("硕士") {
        education.degree = "硕士".into();
        education.degree_detail.clear();
    } else if raw.contains("本科") || raw.contains("学士") {
        education.degree = "本科".into();
        education.degree_detail.clear();
    } else if raw == "其他" {
        education.degree = raw;
        education.degree_detail = detail;
    } else {
        education.degree = "其他".into();
        education.degree_detail = if detail.is_empty() { raw } else { detail };
    }
}

pub fn validate_resume_facts(resume: &mut ResumeProfile) -> Result<(), String> {
    if resume.facts.len() > 500 {
        return Err("invalid_resume_facts: 事实清单最多保留 500 条。".into());
    }
    let mut ids = HashSet::new();
    for fact in &mut resume.facts {
        fact.category = fact.category.trim().to_string();
        fact.value = fact.value.split_whitespace().collect::<Vec<_>>().join(" ");
        fact.source = fact.source.trim().to_string();
        if fact.source.is_empty() {
            fact.source = "历史数据".into();
        }
        if !matches!(
            fact.category.as_str(),
            "identity"
                | "experience"
                | "education"
                | "skill"
                | "project"
                | "certification"
                | "other"
        ) {
            return Err(format!(
                "invalid_resume_facts: 不支持的事实类别 {}。",
                fact.category
            ));
        }
        if fact.value.is_empty() {
            return Err("invalid_resume_facts: 事实内容不能为空。".into());
        }
        if fact.value.chars().count() > 1_000 || fact.source.chars().count() > 500 {
            return Err("invalid_resume_facts: 事实内容或来源过长。".into());
        }
        if !fact.confidence.is_finite() || !(0.0..=1.0).contains(&fact.confidence) {
            return Err("invalid_resume_facts: 事实可靠度必须在 0 到 1 之间。".into());
        }
        if !ids.insert(fact.id.clone()) {
            return Err("invalid_resume_facts: 事实 ID 重复。".into());
        }
    }
    Ok(())
}

fn confirmed_fact_signature(resume: &ResumeProfile) -> HashSet<String> {
    resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed)
        .map(|fact| {
            format!(
                "{}\u{0}{}\u{0}{}",
                fact.id,
                fact.category,
                fact.value.trim()
            )
        })
        .collect()
}

fn clear_job_greetings(transaction: &Transaction<'_>) -> Result<(), String> {
    let jobs = {
        let mut statement = transaction
            .prepare("SELECT id,payload_json FROM jobs")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        rows
    };
    for (id, payload) in jobs {
        let Ok(mut job) = serde_json::from_str::<Job>(&payload) else {
            continue;
        };
        if job.greeting.take().is_none() {
            continue;
        }
        let payload = serde_json::to_string(&job).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "UPDATE jobs SET payload_json=?1 WHERE id=?2",
                params![payload, id],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn active_resume_version(connection: &Connection) -> Result<Option<i64>, String> {
    let payload: Option<String> = connection
        .query_row(
            "SELECT payload_json FROM resume_profiles WHERE is_active=1 ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    payload
        .map(|value| {
            serde_json::from_str::<ResumeProfile>(&value)
                .map(|profile| profile.version)
                .map_err(|error| error.to_string())
        })
        .transpose()
}

fn compute_rebase_changes(
    base: &ResumeProfile,
    master: &ResumeProfile,
    variant: &ResumeProfile,
) -> Result<(Vec<ResumeRebaseChange>, Vec<ResumeRebaseChange>), String> {
    const PATHS: &[(&str, &str)] = &[
        ("/name", "姓名"),
        ("/headline", "职业标题"),
        ("/email", "邮箱"),
        ("/phone", "电话"),
        ("/location", "所在地"),
        ("/website", "个人主页"),
        ("/summary", "个人简介"),
        ("/templateId", "简历结构模板"),
        ("/professionalSkills", "专业技能"),
        ("/experiences", "工作经历"),
        ("/education", "教育经历"),
        ("/projects", "项目经历"),
        ("/certifications", "证书 / 专业资质"),
    ];
    let base = serde_json::to_value(base).map_err(|error| error.to_string())?;
    let master = serde_json::to_value(master).map_err(|error| error.to_string())?;
    let variant = serde_json::to_value(variant).map_err(|error| error.to_string())?;
    let mut automatic = Vec::new();
    let mut conflicts = Vec::new();
    for (path, label) in PATHS {
        let key = path.trim_start_matches('/');
        let base_value = base.get(key).cloned().unwrap_or(serde_json::Value::Null);
        let master_value = master.get(key).cloned().unwrap_or(serde_json::Value::Null);
        let variant_value = variant.get(key).cloned().unwrap_or(serde_json::Value::Null);
        let change = ResumeRebaseChange {
            path: (*path).into(),
            label: (*label).into(),
            base: base_value.clone(),
            master: master_value.clone(),
            variant: variant_value.clone(),
        };
        if variant_value == base_value && master_value != base_value {
            automatic.push(change);
        } else if variant_value != base_value
            && master_value != base_value
            && variant_value != master_value
        {
            conflicts.push(change);
        }
    }
    Ok((automatic, conflicts))
}

fn resume_version_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResumeVersionSummary> {
    Ok(ResumeVersionSummary {
        id: row.get(0)?,
        resume_id: row.get(1)?,
        version: row.get(2)?,
        parent_version: row.get(3)?,
        created_at: row.get(4)?,
        source: row.get(5)?,
        summary: row.get(6)?,
        job_id: row.get(7)?,
        proposal_id: row.get(8)?,
        restored_from_version: row.get(9)?,
    })
}

fn interview_cache_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<InterviewPreparationCacheRecord> {
    let payload: String = row.get(8)?;
    let preparation = serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            payload.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(InterviewPreparationCacheRecord {
        cache_key: row.get(0)?,
        scope_key: row.get(1)?,
        dataset_hash: row.get(2)?,
        resume_id: row.get(3)?,
        resume_version: row.get(4)?,
        provider_fingerprint: row.get(5)?,
        skill_version: row.get(6)?,
        generated_at: row.get(7)?,
        preparation,
    })
}

fn report_competitiveness_cache_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ReportCompetitivenessCacheRecord> {
    let payload: String = row.get(8)?;
    let analysis = serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            payload.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(ReportCompetitivenessCacheRecord {
        cache_key: row.get(0)?,
        scope_key: row.get(1)?,
        dataset_hash: row.get(2)?,
        resume_id: row.get(3)?,
        resume_version: row.get(4)?,
        provider_fingerprint: row.get(5)?,
        skill_version: row.get(6)?,
        generated_at: row.get(7)?,
        analysis,
    })
}

pub fn fingerprint(company: &str, title: &str, location: &str) -> String {
    let normalized = format!(
        "{}|{}|{}",
        normalize(company),
        normalize(title),
        normalize(location)
    );
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && !"-—_·（）()".contains(*character))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_normalization_splits_ranges_and_preserves_other_degrees() {
        let mut profile: ResumeProfile = serde_json::from_value(serde_json::json!({
            "id":"resume","name":"","headline":"","email":"","phone":"","location":"","website":"","summary":"",
            "templateId":"ai-engineering","professionalSkills":[],
            "experiences":[{"company":"公司","position":"工程师","location":"","startDate":"","endDate":"2024.12 - 至今","highlights":[]}],
            "education":[{"institution":"学校","area":"专业","degree":"Bachelor of Science","startDate":"2018.09–2022.06","endDate":"","highlights":[]}],
            "projects":[],"certifications":[],"facts":[],"preferences":{},"sourceFileName":"resume.pdf","updatedAt":"","version":1
        })).unwrap();

        ensure_resume_item_ids(&mut profile);

        assert_eq!(profile.experiences[0].start_date, "2024.12");
        assert_eq!(profile.experiences[0].end_date, "至今");
        assert_eq!(profile.education[0].start_date, "2018.09");
        assert_eq!(profile.education[0].end_date, "2022.06");
        assert_eq!(profile.education[0].degree, "其他");
        assert_eq!(profile.education[0].degree_detail, "Bachelor of Science");
    }
    use crate::models::{FitReport, InterviewPreparation, Job, JobStructuredDetails, ResumeFact};
    use tempfile::tempdir;

    fn job(external_id: &str, salary: &str) -> Job {
        Job {
            id: uuid::Uuid::new_v4().to_string(),
            source: "boss".into(),
            external_id: external_id.into(),
            title: "AI Agent 工程师".into(),
            company: "示例公司".into(),
            salary: salary.into(),
            location: "上海".into(),
            experience: "3-5年".into(),
            degree: "本科".into(),
            company_scale: String::new(),
            company_stage: String::new(),
            industry: String::new(),
            skills: vec!["Python".into()],
            welfare: vec![],
            description: String::new(),
            source_url: String::new(),
            boss_name: None,
            boss_title: None,
            first_seen: "2026-01-01".into(),
            last_seen: "2026-01-01".into(),
            is_new: true,
            fit: None,
            greeting: None,
            patches: vec![],
            structured_details: None,
        }
    }

    fn fit(score: i64) -> FitReport {
        FitReport {
            overall_score: score,
            confidence: 100,
            verdict: String::new(),
            recommendation: String::new(),
            summary: String::new(),
            dimensions: vec![],
            hard_constraints: vec![],
            strengths: vec![],
            gaps: vec![],
            evidence: vec![],
            generated_at: String::new(),
            skill_version: String::new(),
            input_hash: String::new(),
            analysis_source: "local".into(),
            fallback_reason: None,
            cache_status: "fresh".into(),
        }
    }

    fn resume(facts: Vec<ResumeFact>) -> ResumeProfile {
        ResumeProfile {
            id: "resume".into(),
            name: String::new(),
            headline: String::new(),
            email: String::new(),
            phone: String::new(),
            location: String::new(),
            website: String::new(),
            summary: String::new(),
            template_id: "data-analysis".into(),
            professional_skills: vec![],
            experiences: vec![],
            education: vec![],
            projects: vec![],
            certifications: vec![],
            facts,
            preferences: Default::default(),
            source_file_name: "test".into(),
            updated_at: String::new(),
            version: 0,
        }
    }

    #[test]
    fn fact_validation_assigns_missing_ids_and_rejects_invalid_data() {
        let mut valid = resume(vec![ResumeFact {
            id: String::new(),
            category: "skill".into(),
            value: " SQL ".into(),
            source: String::new(),
            confidence: 1.0,
            confirmed: false,
        }]);
        ensure_resume_item_ids(&mut valid);
        validate_resume_facts(&mut valid).unwrap();
        assert!(!valid.facts[0].id.is_empty());
        assert_eq!(valid.facts[0].value, "SQL");
        assert_eq!(valid.facts[0].source, "历史数据");

        let mut duplicate = resume(vec![
            ResumeFact {
                id: "same".into(),
                category: "skill".into(),
                value: "SQL".into(),
                source: "手工".into(),
                confidence: 1.0,
                confirmed: true,
            },
            ResumeFact {
                id: "same".into(),
                category: "other".into(),
                value: "事实".into(),
                source: "手工".into(),
                confidence: 1.0,
                confirmed: false,
            },
        ]);
        assert!(validate_resume_facts(&mut duplicate)
            .unwrap_err()
            .contains("ID 重复"));

        let mut invalid_category = resume(vec![ResumeFact {
            id: "fact".into(),
            category: "accounting".into(),
            value: "月结".into(),
            source: "手工".into(),
            confidence: 1.0,
            confirmed: true,
        }]);
        assert!(validate_resume_facts(&mut invalid_category)
            .unwrap_err()
            .contains("事实类别"));
    }

    #[test]
    fn changing_confirmed_facts_clears_saved_greetings() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        let fact = ResumeFact {
            id: "fact".into(),
            category: "skill".into(),
            value: "Excel".into(),
            source: "手工".into(),
            confidence: 1.0,
            confirmed: false,
        };
        let committed = db
            .commit_resume(resume(vec![fact]), 0, "test", "initial", None, None, None)
            .unwrap();
        let mut stored_job = job("job", "20-30K");
        stored_job.greeting = Some("旧招呼语".into());
        db.upsert_job(stored_job).unwrap();

        let mut changed = committed.resume;
        changed.facts[0].confirmed = true;
        db.commit_resume(changed, 1, "manual", "confirm fact", None, None, None)
            .unwrap();

        assert_eq!(db.list_jobs().unwrap()[0].greeting, None);
    }

    #[test]
    fn v5_to_v6_migration_only_adds_empty_variant_tables() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("resume-v5.db"));
        db.initialize().unwrap();
        let master = db
            .commit_resume(resume(vec![]), 0, "test", "initial", None, None, None)
            .unwrap();
        {
            let connection = db.connect().unwrap();
            connection
                .execute_batch(
                    "DROP TRIGGER IF EXISTS cleanup_resume_variant_versions;
                     DROP TABLE resume_coverage_cache;
                     DROP TABLE resume_variants;
                     DELETE FROM schema_migrations WHERE version=6;",
                )
                .unwrap();
        }

        db.initialize().unwrap();

        assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        assert_eq!(db.active_resume().unwrap().unwrap().id, master.resume.id);
        assert_eq!(
            db.active_resume().unwrap().unwrap().version,
            master.resume.version
        );
        assert!(db.list_resume_variants().unwrap().is_empty());
        let connection = db.connect().unwrap();
        let cache_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM resume_coverage_cache", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(cache_count, 0);
    }

    #[test]
    fn resume_variants_are_unique_versioned_and_rebased_without_changing_master() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("variants.db"));
        db.initialize().unwrap();
        assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

        let mut master = resume(vec![]);
        master.name = "林知远".into();
        master.headline = "AI 工程师".into();
        master.summary = "主简历简介".into();
        let committed = db
            .commit_resume(master, 0, "test", "initial", None, None, None)
            .unwrap();
        let stored_job = job("variant-job", "20-30K");
        let job_id = stored_job.id.clone();
        db.upsert_job(stored_job).unwrap();

        let created = db
            .create_resume_variant(&job_id, committed.resume.version)
            .unwrap();
        let duplicate = db
            .create_resume_variant(&job_id, committed.resume.version)
            .unwrap();
        assert_eq!(created.summary.id, duplicate.summary.id);
        assert_eq!(db.list_resume_variants().unwrap().len(), 1);

        let mut tailored = created.profile.clone();
        tailored.summary = "岗位定制简介".into();
        let saved = db
            .commit_resume_variant(
                &created.summary.id,
                tailored,
                1,
                "variant-manual",
                "manual",
                None,
                None,
            )
            .unwrap();
        assert_eq!(saved.variant.summary.version, 2);
        assert_eq!(db.active_resume().unwrap().unwrap().summary, "主简历简介");

        let mut changed_master = db.active_resume().unwrap().unwrap();
        changed_master.headline = "高级 AI 工程师".into();
        changed_master.summary = "更新后的主简历简介".into();
        let changed_master = db
            .commit_resume(
                changed_master,
                1,
                "manual",
                "master update",
                None,
                None,
                None,
            )
            .unwrap();
        let reopened_with_stale_create_version = db
            .create_resume_variant(&job_id, committed.resume.version)
            .unwrap();
        assert_eq!(
            created.summary.id,
            reopened_with_stale_create_version.summary.id
        );
        assert!(reopened_with_stale_create_version.summary.stale);
        let preview = db
            .preview_resume_variant_rebase(&created.summary.id)
            .unwrap();
        assert!(preview
            .auto_changes
            .iter()
            .any(|item| item.path == "/headline"));
        assert!(preview.conflicts.iter().any(|item| item.path == "/summary"));

        let stale_master_error = db
            .apply_resume_variant_rebase(
                &created.summary.id,
                2,
                changed_master.resume.version - 1,
                &[ResumeRebaseResolution {
                    path: "/summary".into(),
                    choice: "variant".into(),
                }],
            )
            .unwrap_err();
        assert!(stale_master_error.starts_with("version_conflict:"));
        assert_eq!(
            db.get_resume_variant(&created.summary.id)
                .unwrap()
                .unwrap()
                .summary
                .version,
            2
        );

        let rebased = db
            .apply_resume_variant_rebase(
                &created.summary.id,
                2,
                changed_master.resume.version,
                &[ResumeRebaseResolution {
                    path: "/summary".into(),
                    choice: "variant".into(),
                }],
            )
            .unwrap();
        assert_eq!(rebased.variant.profile.headline, "高级 AI 工程师");
        assert_eq!(rebased.variant.profile.summary, "岗位定制简介");
        assert!(!rebased.variant.summary.stale);
        assert_eq!(rebased.version.source, "variant-rebase");
    }

    #[test]
    fn deleting_a_job_cascades_its_resume_variant_and_history() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("variant-cascade.db"));
        db.initialize().unwrap();
        let master = db
            .commit_resume(resume(vec![]), 0, "test", "initial", None, None, None)
            .unwrap();
        let stored_job = job("variant-cascade", "20-30K");
        let job_id = stored_job.id.clone();
        db.upsert_job(stored_job).unwrap();
        let variant = db
            .create_resume_variant(&job_id, master.resume.version)
            .unwrap();
        assert_eq!(
            db.list_resume_versions(&variant.summary.id).unwrap().len(),
            1
        );
        let report = ResumeCoverageReport {
            job_id: job_id.clone(),
            target: crate::models::ResumeTargetRef {
                kind: "variant".into(),
                id: variant.summary.id.clone(),
            },
            target_version: variant.summary.version,
            source: "ai".into(),
            generated_at: time::shanghai_rfc3339(),
            items: vec![],
            covered_count: 0,
            strengthenable_count: 0,
            gap_count: 0,
            unknown_count: 0,
        };
        db.save_resume_coverage_cache("cache", "job", "provider", "skill", &report)
            .unwrap();

        db.delete_job(&job_id).unwrap();
        assert!(db
            .get_resume_variant(&variant.summary.id)
            .unwrap()
            .is_none());
        assert!(db
            .list_resume_versions(&variant.summary.id)
            .unwrap()
            .is_empty());
        assert!(db.resume_coverage_cache("cache").unwrap().is_none());
    }

    #[test]
    fn maintenance_waits_for_existing_connections_and_blocks_new_ones() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("maintenance.db"));
        db.initialize().unwrap();
        let connection = db.connect().unwrap();
        let maintenance_db = db.clone();
        let (acquired_tx, acquired_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let guard = maintenance_db.begin_maintenance().unwrap();
            acquired_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            drop(guard);
        });

        assert!(acquired_rx
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err());
        drop(connection);
        acquired_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        assert!(db
            .connect()
            .err()
            .is_some_and(|error| error.starts_with("busy:")));
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(db.connect().is_ok());
    }

    #[test]
    fn disabling_database_requires_a_restart_before_new_connections() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("disabled.db"));
        db.initialize().unwrap();
        db.begin_maintenance()
            .unwrap()
            .disable_until_restart()
            .unwrap();
        assert!(db
            .connect()
            .err()
            .is_some_and(|error| error.starts_with("database_unavailable:")));
    }

    #[test]
    fn upsert_deduplicates_and_preserves_first_seen() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        let first = db.upsert_jobs(vec![job("id-1", "20-30K")]).unwrap();
        let second = db.upsert_jobs(vec![job("id-1", "25-35K")]).unwrap();
        assert_eq!(first.inserted, 1);
        assert_eq!(second.updated, 1);
        let jobs = db.list_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].salary, "25-35K");
        assert_eq!(jobs[0].first_seen, "2026-01-01");
    }

    #[test]
    fn fallback_fingerprint_is_stable() {
        assert_eq!(
            fingerprint("示例 公司", "AI-Agent工程师", "上海·浦东"),
            fingerprint("示例公司", "AI Agent 工程师", "上海浦东")
        );
    }

    #[test]
    fn streamed_detail_update_does_not_invent_a_completed_scrape_batch() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        db.upsert_job(job("id-1", "20-30K")).unwrap();
        db.update_streamed_job(job("id-1", "25-35K")).unwrap();

        let jobs = db.list_jobs().unwrap();
        assert_eq!(jobs[0].salary, "25-35K");
        assert!(!jobs[0].is_new);
    }

    #[test]
    fn scrape_upsert_preserves_structured_details() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        let mut enriched = job("id-1", "20-30K");
        enriched.structured_details = Some(JobStructuredDetails {
            job_description: "清理后的职位描述".into(),
            ..JobStructuredDetails::default()
        });
        db.upsert_job(enriched).unwrap();
        db.upsert_job(job("id-1", "25-35K")).unwrap();

        let jobs = db.list_jobs().unwrap();
        assert_eq!(
            jobs[0]
                .structured_details
                .as_ref()
                .map(|details| details.job_description.as_str()),
            Some("清理后的职位描述")
        );
    }

    #[test]
    fn keyword_groups_normalize_and_multi_select_uses_a_job_union() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        let first = job("id-1", "20-30K");
        let second = job("id-2", "30-40K");
        db.upsert_scrape_list_job(first, " AI   Agent ").unwrap();
        db.upsert_scrape_list_job(second.clone(), "ai agent")
            .unwrap();
        db.upsert_scrape_list_job(second, "数据分析").unwrap();

        let keywords = db.list_report_keywords().unwrap();
        assert_eq!(keywords.len(), 2);
        let ai = keywords
            .iter()
            .find(|keyword| keyword.key == "ai agent")
            .unwrap();
        assert_eq!(ai.label, "AI Agent");
        assert_eq!(ai.job_count, 2);
        let selected = db
            .list_jobs_by_keyword_keys(&["AI AGENT".into(), "数据分析".into()])
            .unwrap();
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn historical_jobs_are_migrated_then_reclassified_on_a_real_scrape() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        db.upsert_job(job("legacy-1", "20-30K")).unwrap();
        {
            let connection = db.connect().unwrap();
            connection
                .execute("DELETE FROM schema_migrations WHERE version=3", [])
                .unwrap();
            connection.execute("DROP TABLE job_keywords", []).unwrap();
        }
        db.initialize().unwrap();
        let keywords = db.list_report_keywords().unwrap();
        assert_eq!(keywords.len(), 1);
        assert_eq!(keywords[0].key, HISTORICAL_KEYWORD_KEY);

        db.upsert_scrape_list_job(job("legacy-1", "25-35K"), "AI Agent")
            .unwrap();
        let keywords = db.list_report_keywords().unwrap();
        assert_eq!(keywords.len(), 1);
        assert_eq!(keywords[0].key, "ai agent");
    }

    #[test]
    fn list_refresh_preserves_detail_content_and_detail_updates_commit_individually() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        let mut detail = job("id-1", "20-30K");
        detail.description = "第一版岗位详情".into();
        detail.skills = vec!["Rust".into()];
        detail.structured_details = Some(JobStructuredDetails {
            job_description: "结构化详情".into(),
            ..JobStructuredDetails::default()
        });
        db.upsert_scrape_detail_job(detail, "AI Agent").unwrap();

        let mut listing = job("id-1", "25-35K");
        listing.skills = vec!["Python".into()];
        db.upsert_scrape_list_job(listing, "AI Agent").unwrap();
        let preserved = db.get_job(&db.list_jobs().unwrap()[0].id).unwrap().unwrap();
        assert_eq!(preserved.description, "第一版岗位详情");
        assert_eq!(preserved.skills, vec!["Rust"]);
        assert!(preserved.structured_details.is_some());

        let mut refreshed_detail = job("id-1", "25-35K");
        refreshed_detail.description = "第二版岗位详情".into();
        refreshed_detail.skills = vec!["Go".into()];
        db.upsert_scrape_detail_job(refreshed_detail, "AI Agent")
            .unwrap();
        let saved = db.list_jobs().unwrap();
        assert_eq!(saved[0].description, "第二版岗位详情");
        assert_eq!(saved[0].skills, vec!["Go"]);
        assert_eq!(
            db.completed_detail_external_ids("boss").unwrap(),
            vec!["id-1"]
        );
    }

    #[test]
    fn interview_cache_latest_results_are_isolated_by_keyword_scope() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        for (cache_key, scope_key, generated_at, summary) in [
            ("cache-ai", "scope-ai", "2026-01-01T10:00:00+08:00", "AI"),
            (
                "cache-finance",
                "scope-finance",
                "2026-01-02T10:00:00+08:00",
                "财务",
            ),
        ] {
            db.save_interview_preparation(&InterviewPreparationCacheRecord {
                cache_key: cache_key.into(),
                scope_key: scope_key.into(),
                dataset_hash: "dataset".into(),
                resume_id: None,
                resume_version: None,
                provider_fingerprint: "provider".into(),
                skill_version: "interview-preparation@1.0.0".into(),
                generated_at: generated_at.into(),
                preparation: InterviewPreparation {
                    summary: summary.into(),
                    skills: vec![],
                    project_ideas: vec![],
                    practice_questions: vec![],
                },
            })
            .unwrap();
        }

        assert_eq!(
            db.latest_interview_preparation("scope-ai")
                .unwrap()
                .unwrap()
                .preparation
                .summary,
            "AI"
        );
        assert_eq!(
            db.latest_interview_preparation("scope-finance")
                .unwrap()
                .unwrap()
                .preparation
                .summary,
            "财务"
        );
    }

    #[test]
    fn xiaomi_provider_defaults_and_migrates_to_mimo_v2_5() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();

        let mut provider = db.provider_by_id("provider-xiaomi").unwrap().unwrap();
        assert_eq!(provider.model, "mimo-v2.5");

        provider.model = "mimo-v2.5-pro".into();
        provider.verified = true;
        provider.vision_verified = true;
        db.save_provider(&provider).unwrap();
        db.initialize().unwrap();

        let migrated = db.provider_by_id("provider-xiaomi").unwrap().unwrap();
        assert_eq!(migrated.model, "mimo-v2.5");
        assert!(!migrated.verified);
        assert!(!migrated.vision_verified);
    }

    #[test]
    fn only_new_tracks_insertions_from_the_latest_completed_scrape_run() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("latest-new.db"));
        db.initialize().unwrap();

        db.upsert_scrape_list_job_for_run(job("old", "20-30K"), "AI Agent", "run-old")
            .unwrap();
        db.save_scrape_run(&ScrapeRun {
            id: "run-old".into(),
            keyword: "AI Agent".into(),
            city: "上海".into(),
            total_seen: 1,
            inserted: 1,
            updated: 0,
            started_at: "2026-07-17T10:00:00+08:00".into(),
            completed_at: Some("2026-07-17T10:05:00+08:00".into()),
            report_markdown: None,
            search_spec: None,
            resolved_city: Some("上海".into()),
            detail_summary: None,
            sample: None,
        })
        .unwrap();

        db.upsert_scrape_list_job_for_run(job("old", "25-35K"), "AI Agent", "run-latest")
            .unwrap();
        db.upsert_scrape_list_job_for_run(job("new", "30-40K"), "AI Agent", "run-latest")
            .unwrap();
        db.save_scrape_run(&ScrapeRun {
            id: "run-latest".into(),
            keyword: "AI Agent".into(),
            city: "上海".into(),
            total_seen: 2,
            inserted: 1,
            updated: 1,
            started_at: "2026-07-18T10:00:00+08:00".into(),
            completed_at: Some("2026-07-18T10:05:00+08:00".into()),
            report_markdown: None,
            search_spec: None,
            resolved_city: Some("上海".into()),
            detail_summary: None,
            sample: None,
        })
        .unwrap();

        db.upsert_scrape_list_job_for_run(job("failed-run", "40-50K"), "AI Agent", "run-failed")
            .unwrap();

        let latest = db
            .list_jobs_page(&JobQuery {
                only_new: true,
                ..JobQuery::default()
            })
            .unwrap();
        assert_eq!(latest.total, 1);
        assert_eq!(latest.items[0].external_id, "new");
        assert!(latest.items[0].is_new);

        let all = db.list_jobs().unwrap();
        assert!(
            all.iter()
                .find(|job| job.external_id == "new")
                .unwrap()
                .is_new
        );
        assert!(
            !all.iter()
                .find(|job| job.external_id == "old")
                .unwrap()
                .is_new
        );
        assert!(
            !all.iter()
                .find(|job| job.external_id == "failed-run")
                .unwrap()
                .is_new
        );
    }

    #[test]
    fn pagination_and_atomic_task_reservation_are_stable() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        for index in 0..61 {
            let mut value = job(&format!("id-{index:03}"), "20-30K");
            value.title = format!("Rust Engineer {index:03}");
            value.last_seen = format!("2026-01-{:02}T10:00:00+08:00", index % 28 + 1);
            db.upsert_job(value).unwrap();
        }
        let query = JobQuery {
            query: "rust".into(),
            ..JobQuery::default()
        };
        let first = db.list_jobs_page(&query).unwrap();
        assert_eq!(first.total, 61);
        assert_eq!(first.items.len(), JOB_PAGE_SIZE);
        let recommended_cursor = first.next_cursor.clone();
        let second = db
            .list_jobs_page(&JobQuery {
                cursor: recommended_cursor.clone(),
                ..query
            })
            .unwrap();
        assert_eq!(second.items.len(), 11);
        let first_ids = first
            .items
            .into_iter()
            .map(|job| job.id)
            .collect::<HashSet<_>>();
        assert!(second.items.iter().all(|job| !first_ids.contains(&job.id)));

        for sort in ["recent", "salary-desc"] {
            let first = db
                .list_jobs_page(&JobQuery {
                    query: "rust".into(),
                    sort: sort.into(),
                    ..JobQuery::default()
                })
                .unwrap();
            let second = db
                .list_jobs_page(&JobQuery {
                    query: "rust".into(),
                    sort: sort.into(),
                    cursor: first.next_cursor.clone(),
                    ..JobQuery::default()
                })
                .unwrap();
            let first_ids = first
                .items
                .iter()
                .map(|job| job.id.clone())
                .collect::<HashSet<_>>();
            assert_eq!(first.items.len() + second.items.len(), 61);
            assert!(second.items.iter().all(|job| !first_ids.contains(&job.id)));
        }
        assert!(db
            .list_jobs_page(&JobQuery {
                query: "rust".into(),
                sort: "recent".into(),
                cursor: recommended_cursor,
                ..JobQuery::default()
            })
            .is_err());

        let now = time::shanghai_rfc3339();
        let task = TaskRun {
            id: "task-1".into(),
            kind: "scrape".into(),
            title: "one".into(),
            state: "queued".into(),
            progress: 0,
            message: String::new(),
            recoverable_error: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            logs: vec![],
        };
        let competing = TaskRun {
            id: "task-2".into(),
            ..task.clone()
        };
        assert!(db.reserve_task(&task).unwrap());
        assert!(!db.reserve_task(&competing).unwrap());
    }

    #[test]
    fn job_filter_options_and_sort_orders_are_consistent() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();

        let mut high_score = job("high-score", "20-30K");
        high_score.last_seen = "2026-07-17".into();
        high_score.skills = vec!["Python".into()];
        high_score.fit = Some(fit(90));
        db.upsert_job(high_score).unwrap();

        let mut high_salary = job("high-salary", "50-70K");
        high_salary.last_seen = "2026-07-18".into();
        high_salary.experience = "5-10年".into();
        high_salary.skills = vec!["Python".into(), "RAG".into()];
        high_salary.fit = Some(fit(70));
        db.upsert_job(high_salary).unwrap();

        let mut unknown_salary = job("unknown-salary", "面议");
        unknown_salary.last_seen = "2026-07-19".into();
        unknown_salary.skills = vec!["Rust".into()];
        unknown_salary.fit = Some(fit(95));
        db.upsert_job(unknown_salary).unwrap();

        let ids_for = |sort: &str| {
            db.list_jobs_page(&JobQuery {
                sort: sort.into(),
                ..JobQuery::default()
            })
            .unwrap()
            .items
            .into_iter()
            .map(|job| job.external_id)
            .collect::<Vec<_>>()
        };
        assert_eq!(
            ids_for("recommended"),
            vec!["unknown-salary", "high-score", "high-salary"]
        );
        assert_eq!(
            ids_for("recent"),
            vec!["unknown-salary", "high-salary", "high-score"]
        );
        assert_eq!(
            ids_for("salary-desc"),
            vec!["high-salary", "high-score", "unknown-salary"]
        );
        assert_eq!(
            db.jobs_for_query(&JobQuery {
                skills: vec!["Python".into()],
                sort: "salary-desc".into(),
                ..JobQuery::default()
            })
            .unwrap()
            .into_iter()
            .map(|job| job.external_id)
            .collect::<Vec<_>>(),
            vec!["high-salary", "high-score"]
        );

        let options = db.list_job_filter_options().unwrap();
        assert_eq!(options.cities, vec!["上海"]);
        assert_eq!(options.experiences, vec!["3-5年", "5-10年"]);
        assert_eq!(options.skills[0].label, "Python");
        assert_eq!(options.skills[0].count, 2);
    }

    #[test]
    fn report_drilldown_combines_keyword_or_skill_and_and_exact_dimensions() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();

        let mut agent = job("agent", "20-30K");
        agent.skills = vec!["Python".into(), "RAG".into()];
        agent.experience = "3-5年".into();
        let agent_id = agent.id.clone();
        db.upsert_scrape_list_job(agent, "AI Agent").unwrap();

        let mut data = job("data", "10-20K");
        data.skills = vec!["Python".into(), "Java".into()];
        data.experience = "5-10年".into();
        let data_id = data.id.clone();
        db.upsert_scrape_list_job(data, "数据分析").unwrap();

        let keywords = db.list_report_keywords().unwrap();
        let agent_key = keywords
            .iter()
            .find(|item| item.label == "AI Agent")
            .unwrap()
            .key
            .clone();
        let data_key = keywords
            .iter()
            .find(|item| item.label == "数据分析")
            .unwrap()
            .key
            .clone();

        let keyword_page = db
            .list_jobs_page(&JobQuery {
                keyword_keys: vec![agent_key.clone(), data_key.clone()],
                ..JobQuery::default()
            })
            .unwrap();
        assert_eq!(keyword_page.total, 2);

        let skill_page = db
            .list_jobs_page(&JobQuery {
                keyword_keys: vec![agent_key, data_key],
                skills: vec!["Python".into(), "RAG".into()],
                experience: "3-5年".into(),
                salary_band: "25-35".into(),
                ..JobQuery::default()
            })
            .unwrap();
        assert_eq!(skill_page.total, 1);
        assert_eq!(skill_page.items[0].id, agent_id);

        let lower_band = db
            .list_jobs_page(&JobQuery {
                salary_band: "15-25".into(),
                ..JobQuery::default()
            })
            .unwrap();
        assert_eq!(lower_band.total, 1);
        assert_eq!(lower_band.items[0].id, data_id);
    }

    #[test]
    fn report_competitiveness_cache_is_scope_isolated_and_retains_ten_entries() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        for index in 0..11 {
            db.save_report_competitiveness(&ReportCompetitivenessCacheRecord {
                cache_key: format!("cache-{index:02}"),
                scope_key: format!("scope-{}", index % 2),
                dataset_hash: format!("dataset-{index}"),
                resume_id: "resume".into(),
                resume_version: index,
                provider_fingerprint: "provider".into(),
                skill_version: "v1".into(),
                generated_at: format!("2026-07-16T12:{index:02}:00+08:00"),
                analysis: ReportCompetitivenessAnalysis {
                    source: "ai".into(),
                    resume_id: "resume".into(),
                    resume_version: index,
                    generated_at: format!("2026-07-16T12:{index:02}:00+08:00"),
                    items: vec![],
                },
            })
            .unwrap();
        }

        assert!(db
            .report_competitiveness_by_key("cache-00")
            .unwrap()
            .is_none());
        assert!(db
            .report_competitiveness_by_key("cache-10")
            .unwrap()
            .is_some());
        assert_eq!(
            db.latest_report_competitiveness("scope-0")
                .unwrap()
                .unwrap()
                .cache_key,
            "cache-10"
        );
        assert_eq!(
            db.latest_report_competitiveness("scope-1")
                .unwrap()
                .unwrap()
                .cache_key,
            "cache-09"
        );
    }

    #[test]
    fn scrape_reservation_persists_the_full_spec_without_competing_overwrites() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        let now = time::shanghai_rfc3339();
        let task = TaskRun {
            id: "scrape-1".into(),
            kind: "scrape".into(),
            title: "first".into(),
            state: "queued".into(),
            progress: 0,
            message: String::new(),
            recoverable_error: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            logs: vec![],
        };
        let competing = TaskRun {
            id: "scrape-2".into(),
            ..task.clone()
        };
        let first = SearchSpec {
            keyword: "AI Agent".into(),
            city: "杭州".into(),
            pages: 4,
            salary: Some("405".into()),
            experience: Some("105".into()),
            degree: Some("203".into()),
            company_scale: Some("303".into()),
        };
        let second = SearchSpec {
            keyword: "不应保存".into(),
            city: "北京".into(),
            pages: 1,
            salary: None,
            experience: None,
            degree: None,
            company_scale: None,
        };

        assert!(db.reserve_scrape_task(&task, &first).unwrap());
        assert!(!db.reserve_scrape_task(&competing, &second).unwrap());

        let saved = db.last_search_spec().unwrap().unwrap();
        assert_eq!(saved.keyword, first.keyword);
        assert_eq!(saved.city, first.city);
        assert_eq!(saved.pages, first.pages);
        assert_eq!(saved.salary, first.salary);
        assert_eq!(saved.experience, first.experience);
        assert_eq!(saved.degree, first.degree);
        assert_eq!(saved.company_scale, first.company_scale);
    }

    #[test]
    fn city_filter_migration_and_missing_description_deletion_are_safe() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();

        let mut missing_shanghai = job("missing-shanghai", "20-30K");
        missing_shanghai.location = "上海·浦东新区".into();
        let missing_shanghai_id = missing_shanghai.id.clone();
        db.upsert_scrape_list_job(missing_shanghai, "AI Agent")
            .unwrap();

        let mut detailed_shanghai = job("detailed-shanghai", "20-30K");
        detailed_shanghai.location = "上海·徐汇区".into();
        detailed_shanghai.description = "负责 AI 平台研发".into();
        let detailed_shanghai_id = detailed_shanghai.id.clone();
        db.upsert_scrape_list_job(detailed_shanghai, "AI Agent")
            .unwrap();

        let mut missing_hangzhou = job("missing-hangzhou", "20-30K");
        missing_hangzhou.location = "杭州·余杭区".into();
        let missing_hangzhou_id = missing_hangzhou.id.clone();
        db.upsert_scrape_list_job(missing_hangzhou, "AI Agent")
            .unwrap();

        let connection = db.connect().unwrap();
        connection.execute("UPDATE jobs SET city=''", []).unwrap();
        connection
            .execute("DELETE FROM schema_migrations WHERE version=5", [])
            .unwrap();
        drop(connection);
        db.initialize().unwrap();

        let cities = db
            .list_job_cities()
            .unwrap()
            .into_iter()
            .collect::<HashSet<_>>();
        assert_eq!(
            cities,
            HashSet::from(["上海".to_string(), "杭州".to_string()])
        );

        let query = JobQuery {
            city: "上海".into(),
            missing_description: true,
            ..JobQuery::default()
        };
        let page = db.list_jobs_page(&query).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, missing_shanghai_id);

        let deleted = db
            .delete_missing_description_jobs(&JobQuery {
                city: "上海".into(),
                ..JobQuery::default()
            })
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(db.get_job(&detailed_shanghai_id).unwrap().is_some());
        assert!(db.get_job(&missing_hangzhou_id).unwrap().is_some());
        assert_eq!(db.list_report_keywords().unwrap()[0].job_count, 2);

        assert_eq!(db.delete_job(&missing_hangzhou_id).unwrap(), 1);
        assert_eq!(db.list_report_keywords().unwrap()[0].job_count, 1);
        assert_eq!(db.delete_job(&detailed_shanghai_id).unwrap(), 1);
        assert!(db.list_report_keywords().unwrap().is_empty());
    }

    #[test]
    fn missing_jobs_and_stale_resume_versions_fail_loudly() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        assert!(db.save_job(&job("missing", "20K")).is_err());

        let initial = resume(vec![]);
        db.commit_resume(initial.clone(), 0, "test", "initial", None, None, None)
            .unwrap();
        let error = db
            .commit_resume(initial, 0, "test", "stale", None, None, None)
            .unwrap_err();
        assert!(error.contains("version_conflict"));
    }
}
