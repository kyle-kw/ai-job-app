use crate::models::{
    AiProviderConfig, AppSettings, BossProfileState, InterviewPreparation, Job, JobOption, JobPage,
    JobQuery, ReportKeyword, ResumeCommitResult, ResumeCoverageReport, ResumeEducation,
    ResumeProfile, ResumeRebaseChange, ResumeRebasePreview, ResumeRebaseResolution,
    ResumeVariantCommitResult, ResumeVariantDetail, ResumeVariantSummary, ResumeVersionDetail,
    ResumeVersionSummary, ScrapeRun, SearchSpec, TaskRun,
};
use crate::time;
use base64::Engine;
use rusqlite::backup::Backup;
use rusqlite::types::Value as SqlValue;
use rusqlite::{
    params, params_from_iter, Connection, OpenFlags, OptionalExtension, Transaction,
    TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

mod providers;

pub const CURRENT_SCHEMA_VERSION: i64 = 6;

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

const JOB_PAGE_SIZE: usize = 50;

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
    score: i64,
    last_seen: String,
    id: String,
}

impl Database {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            gate: Arc::new(DatabaseGate::default()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn open_connection(path: &Path) -> Result<Connection, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| error.to_string())?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        Ok(connection)
    }

    pub(crate) fn connect(&self) -> Result<DatabaseConnection, String> {
        {
            let mut state = self
                .gate
                .state
                .lock()
                .map_err(|_| "database gate is poisoned".to_string())?;
            if state.unavailable {
                return Err("database_unavailable: 数据已更改，请重启应用".into());
            }
            if state.maintenance {
                return Err("busy: 数据库正在维护，请稍后重试".into());
            }
            state.active_connections += 1;
        }

        match Self::open_connection(&self.path) {
            Ok(connection) => Ok(DatabaseConnection {
                connection,
                gate: self.gate.clone(),
            }),
            Err(error) => {
                if let Ok(mut state) = self.gate.state.lock() {
                    state.active_connections = state.active_connections.saturating_sub(1);
                    if state.active_connections == 0 {
                        self.gate.idle.notify_all();
                    }
                }
                Err(error)
            }
        }
    }

    pub(crate) fn begin_maintenance(&self) -> Result<DatabaseMaintenanceGuard, String> {
        let mut state = self
            .gate
            .state
            .lock()
            .map_err(|_| "database gate is poisoned".to_string())?;
        if state.unavailable {
            return Err("database_unavailable: 数据已更改，请重启应用".into());
        }
        if state.maintenance {
            return Err("busy: 数据库正在维护，请稍后重试".into());
        }
        state.maintenance = true;
        while state.active_connections > 0 {
            state = self
                .gate
                .idle
                .wait(state)
                .map_err(|_| "database gate is poisoned".to_string())?;
        }
        drop(state);
        Ok(DatabaseMaintenanceGuard {
            path: self.path.clone(),
            gate: self.gate.clone(),
            released: false,
        })
    }

    pub fn initialize(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS jobs (
                    id TEXT PRIMARY KEY,
                    source TEXT NOT NULL,
                    external_key TEXT NOT NULL,
                    fingerprint TEXT NOT NULL,
                    title TEXT NOT NULL,
                    company TEXT NOT NULL,
                    location TEXT NOT NULL,
                    first_seen TEXT NOT NULL,
                    last_seen TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    UNIQUE(source, external_key)
                );
                CREATE INDEX IF NOT EXISTS idx_jobs_last_seen ON jobs(last_seen DESC);
                CREATE INDEX IF NOT EXISTS idx_jobs_title ON jobs(title);
                CREATE TABLE IF NOT EXISTS scrape_runs (
                    id TEXT PRIMARY KEY,
                    payload_json TEXT NOT NULL,
                    started_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS resume_profiles (
                    id TEXT PRIMARY KEY,
                    payload_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    is_active INTEGER NOT NULL DEFAULT 1
                );
                CREATE TABLE IF NOT EXISTS ai_providers (
                    id TEXT PRIMARY KEY,
                    payload_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS task_runs (
                    id TEXT PRIMARY KEY,
                    payload_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS app_settings (
                    key TEXT PRIMARY KEY,
                    payload_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS resume_versions (
                    id TEXT PRIMARY KEY,
                    resume_id TEXT NOT NULL,
                    version INTEGER NOT NULL,
                    parent_version INTEGER,
                    created_at TEXT NOT NULL,
                    source TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    job_id TEXT,
                    proposal_id TEXT,
                    restored_from_version INTEGER,
                    profile_json TEXT NOT NULL,
                    UNIQUE(resume_id, version)
                );
                CREATE INDEX IF NOT EXISTS idx_resume_versions_resume
                    ON resume_versions(resume_id, version DESC);
                CREATE TABLE IF NOT EXISTS interview_preparation_cache (
                    cache_key TEXT PRIMARY KEY,
                    scope_key TEXT NOT NULL DEFAULT '',
                    dataset_hash TEXT NOT NULL,
                    resume_id TEXT,
                    resume_version INTEGER,
                    provider_fingerprint TEXT NOT NULL,
                    skill_version TEXT NOT NULL,
                    generated_at TEXT NOT NULL,
                    payload_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_interview_preparation_generated
                    ON interview_preparation_cache(generated_at DESC);
                INSERT OR IGNORE INTO schema_migrations(version, applied_at)
                VALUES (1, datetime('now'));
                "#,
            )
            .map_err(|error| error.to_string())?;
        Self::migrate_v2(&transaction)?;
        Self::migrate_v3(&transaction)?;
        Self::migrate_v4(&transaction)?;
        Self::migrate_v5(&transaction)?;
        Self::migrate_v6(&transaction)?;
        transaction.commit().map_err(|error| error.to_string())?;
        self.recover_interrupted_tasks()?;
        Ok(())
    }

    fn migrate_v2(transaction: &Transaction<'_>) -> Result<(), String> {
        let already_applied: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=2)",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;

        let mut providers = {
            let mut statement = transaction
                .prepare("SELECT payload_json FROM ai_providers ORDER BY rowid")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| error.to_string())?;
            rows.filter_map(|row| row.ok())
                .filter_map(|payload| serde_json::from_str::<AiProviderConfig>(&payload).ok())
                .collect::<Vec<_>>()
        };

        providers.retain(|provider| {
            provider.id != "provider-openrouter" && provider.kind != "openrouter"
        });
        let custom_is_default = providers
            .iter()
            .any(|provider| provider.kind == "custom" && provider.is_default);
        let mut xiaomi = providers
            .iter()
            .find(|provider| provider.id == "provider-xiaomi")
            .cloned()
            .unwrap_or_else(default_xiaomi_provider);
        let mut connection_changed = false;
        if !already_applied
            && (xiaomi.base_url.trim().is_empty()
                || xiaomi.base_url == "https://api.xiaomimimo.com/v1")
        {
            xiaomi.base_url = "https://token-plan-sgp.xiaomimimo.com/v1".into();
            connection_changed = true;
        }
        if xiaomi.model == "mimo-v2.5-pro" {
            xiaomi.model = "mimo-v2.5".into();
            connection_changed = true;
        }
        if connection_changed {
            xiaomi.verified = false;
            xiaomi.vision_verified = false;
            xiaomi.last_tested_at = None;
            xiaomi.last_test_error = None;
        }
        xiaomi.kind = "xiaomi".into();
        xiaomi.name = "默认模型 · 小米 MiMo".into();
        xiaomi.is_default = !custom_is_default;
        providers.retain(|provider| provider.id != "provider-xiaomi");
        providers.insert(0, xiaomi);
        if !providers
            .iter()
            .any(|provider| provider.id == "provider-custom")
        {
            providers.push(default_custom_provider());
        }

        transaction
            .execute("DELETE FROM ai_providers", [])
            .map_err(|error| error.to_string())?;
        for provider in &providers {
            let payload = serde_json::to_string(provider).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO ai_providers(id, payload_json) VALUES (?1, ?2)",
                    params![provider.id, payload],
                )
                .map_err(|error| error.to_string())?;
        }

        let legacy_boss: Option<String> = transaction
            .query_row(
                "SELECT payload_json FROM app_settings WHERE key='boss_logged_in'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let existing_boss: Option<String> = transaction
            .query_row(
                "SELECT payload_json FROM app_settings WHERE key='boss_profile_state'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if existing_boss.is_none() {
            let mut boss = BossProfileState::default();
            if legacy_boss.as_deref() == Some("true") {
                boss.configured = true;
                boss.last_attempt_status = "succeeded".into();
            }
            let payload = serde_json::to_string(&boss).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO app_settings(key,payload_json) VALUES ('boss_profile_state',?1)",
                    [payload],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute("DELETE FROM app_settings WHERE key='boss_logged_in'", [])
            .map_err(|error| error.to_string())?;

        let resumes = {
            let mut statement = transaction
                .prepare("SELECT id, payload_json, updated_at FROM resume_profiles")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|error| error.to_string())?;
            rows.filter_map(|row| row.ok()).collect::<Vec<_>>()
        };
        for (id, payload, updated_at) in resumes {
            let Ok(mut resume) = serde_json::from_str::<ResumeProfile>(&payload) else {
                continue;
            };
            ensure_resume_item_ids(&mut resume);
            let normalized = serde_json::to_string(&resume).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE resume_profiles SET payload_json=?1 WHERE id=?2",
                    params![normalized, id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,profile_json) VALUES (?1,?2,?3,NULL,?4,'legacy','迁移前当前版本',?5)",
                    params![uuid::Uuid::new_v4().to_string(), resume.id, resume.version, updated_at, normalized],
                )
                .map_err(|error| error.to_string())?;
        }

        transaction
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (2, datetime('now'))",
                [],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn migrate_v3(transaction: &Transaction<'_>) -> Result<(), String> {
        let already_applied: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=3)",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS job_keywords (
                    job_id TEXT NOT NULL,
                    keyword_key TEXT NOT NULL,
                    keyword_label TEXT NOT NULL,
                    first_seen TEXT NOT NULL,
                    last_seen TEXT NOT NULL,
                    PRIMARY KEY(job_id, keyword_key),
                    FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_job_keywords_key
                    ON job_keywords(keyword_key, last_seen DESC);
                CREATE INDEX IF NOT EXISTS idx_job_keywords_last_seen
                    ON job_keywords(last_seen DESC);
                "#,
            )
            .map_err(|error| error.to_string())?;

        let has_scope_key = {
            let mut statement = transaction
                .prepare("PRAGMA table_info(interview_preparation_cache)")
                .map_err(|error| error.to_string())?;
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|error| error.to_string())?;
            let mut found = false;
            for name in columns.flatten() {
                if name == "scope_key" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_scope_key {
            transaction
                .execute(
                    "ALTER TABLE interview_preparation_cache ADD COLUMN scope_key TEXT NOT NULL DEFAULT ''",
                    [],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_interview_preparation_scope_generated ON interview_preparation_cache(scope_key, generated_at DESC)",
                [],
            )
            .map_err(|error| error.to_string())?;
        if !already_applied {
            transaction
                .execute(
                    r#"INSERT OR IGNORE INTO job_keywords(job_id,keyword_key,keyword_label,first_seen,last_seen)
                       SELECT id,?1,?2,first_seen,last_seen FROM jobs"#,
                    params![HISTORICAL_KEYWORD_KEY, HISTORICAL_KEYWORD_LABEL],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (3, datetime('now'))",
                [],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn migrate_v4(transaction: &Transaction<'_>) -> Result<(), String> {
        let already_applied = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=4)",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        if already_applied {
            return Ok(());
        }
        let columns = {
            let mut statement = transaction
                .prepare("PRAGMA table_info(jobs)")
                .map_err(|error| error.to_string())?;
            let result = statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|error| error.to_string())?
                .collect::<Result<HashSet<_>, _>>()
                .map_err(|error| error.to_string())?;
            result
        };
        let additions = [
            (
                "search_text",
                "ALTER TABLE jobs ADD COLUMN search_text TEXT NOT NULL DEFAULT ''",
            ),
            ("salary_min", "ALTER TABLE jobs ADD COLUMN salary_min REAL"),
            ("salary_max", "ALTER TABLE jobs ADD COLUMN salary_max REAL"),
            (
                "company_scale_code",
                "ALTER TABLE jobs ADD COLUMN company_scale_code TEXT NOT NULL DEFAULT ''",
            ),
            (
                "query_is_new",
                "ALTER TABLE jobs ADD COLUMN query_is_new INTEGER NOT NULL DEFAULT 0",
            ),
            ("fit_score", "ALTER TABLE jobs ADD COLUMN fit_score INTEGER"),
            (
                "has_description",
                "ALTER TABLE jobs ADD COLUMN has_description INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "has_structured_details",
                "ALTER TABLE jobs ADD COLUMN has_structured_details INTEGER NOT NULL DEFAULT 0",
            ),
        ];
        for (name, sql) in additions {
            if !columns.contains(name) {
                transaction
                    .execute(sql, [])
                    .map_err(|error| error.to_string())?;
            }
        }
        transaction
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_jobs_page ON jobs(fit_score DESC, last_seen DESC, id ASC);
                 CREATE INDEX IF NOT EXISTS idx_jobs_pending_detail ON jobs(has_description, has_structured_details);
                 CREATE INDEX IF NOT EXISTS idx_jobs_source_detail ON jobs(source, has_description);",
            )
            .map_err(|error| error.to_string())?;
        let jobs = {
            let mut statement = transaction
                .prepare("SELECT id,payload_json FROM jobs")
                .map_err(|error| error.to_string())?;
            let result = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            result
        };
        for (id, payload) in jobs {
            let job: Job = serde_json::from_str(&payload).map_err(|error| error.to_string())?;
            let meta = JobQueryMetadata::from_job(&job);
            transaction.execute(
                "UPDATE jobs SET search_text=?1,salary_min=?2,salary_max=?3,company_scale_code=?4,query_is_new=?5,fit_score=?6,has_description=?7,has_structured_details=?8 WHERE id=?9",
                params![meta.search_text, meta.salary_min, meta.salary_max, meta.company_scale_code, meta.is_new, meta.fit_score, meta.has_description, meta.has_structured_details, id],
            ).map_err(|error| error.to_string())?;
        }
        transaction
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (4, datetime('now'))",
                [],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn migrate_v5(transaction: &Transaction<'_>) -> Result<(), String> {
        let already_applied = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=5)",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        if already_applied {
            return Ok(());
        }
        let has_city = {
            let mut statement = transaction
                .prepare("PRAGMA table_info(jobs)")
                .map_err(|error| error.to_string())?;
            let found = statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|error| error.to_string())?
                .filter_map(Result::ok)
                .any(|name| name == "city");
            found
        };
        if !has_city {
            transaction
                .execute(
                    "ALTER TABLE jobs ADD COLUMN city TEXT NOT NULL DEFAULT ''",
                    [],
                )
                .map_err(|error| error.to_string())?;
        }
        let locations = {
            let mut statement = transaction
                .prepare("SELECT id,location FROM jobs")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            rows
        };
        for (id, location) in locations {
            transaction
                .execute(
                    "UPDATE jobs SET city=?1 WHERE id=?2",
                    params![job_city(&location), id],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute("CREATE INDEX IF NOT EXISTS idx_jobs_city ON jobs(city)", [])
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (5, datetime('now'))",
                [],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn migrate_v6(transaction: &Transaction<'_>) -> Result<(), String> {
        let already_applied = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=6)",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        if already_applied {
            return Ok(());
        }
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS resume_variants (
                    id TEXT PRIMARY KEY,
                    job_id TEXT NOT NULL UNIQUE,
                    base_resume_id TEXT NOT NULL,
                    base_resume_version INTEGER NOT NULL,
                    version INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_resume_variants_updated
                    ON resume_variants(updated_at DESC);
                CREATE TABLE IF NOT EXISTS resume_coverage_cache (
                    cache_key TEXT PRIMARY KEY,
                    target_kind TEXT NOT NULL,
                    target_id TEXT NOT NULL,
                    target_version INTEGER NOT NULL,
                    job_id TEXT NOT NULL,
                    job_fingerprint TEXT NOT NULL,
                    provider_fingerprint TEXT NOT NULL,
                    skill_version TEXT NOT NULL,
                    generated_at TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_resume_coverage_target
                    ON resume_coverage_cache(target_kind,target_id,target_version);
                CREATE TRIGGER IF NOT EXISTS cleanup_resume_variant_versions
                AFTER DELETE ON resume_variants
                BEGIN
                    DELETE FROM resume_versions WHERE resume_id=OLD.id;
                    DELETE FROM resume_coverage_cache WHERE target_kind='variant' AND target_id=OLD.id;
                END;
                INSERT OR IGNORE INTO schema_migrations(version, applied_at)
                VALUES (6, datetime('now'));
                "#,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn list_jobs(&self) -> Result<Vec<Job>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT payload_json FROM jobs ORDER BY last_seen DESC, title ASC")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.map(|row| {
            let json = row.map_err(|error| error.to_string())?;
            serde_json::from_str(&json).map_err(|error| error.to_string())
        })
        .collect()
    }

    pub fn list_jobs_page(&self, query: &JobQuery) -> Result<JobPage, String> {
        let connection = self.connect()?;
        let (where_without_cursor, count_values) = job_query_where(query, false)?;
        let total = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM jobs WHERE {where_without_cursor}"),
                params_from_iter(count_values.iter()),
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let pending_detail_count = connection
            .query_row(
                "SELECT COUNT(*) FROM jobs WHERE has_description=1 AND has_structured_details=0",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let (where_clause, values) = job_query_where(query, true)?;
        let sql = format!(
            "SELECT payload_json,COALESCE(fit_score,0),last_seen,id FROM jobs WHERE {where_clause} ORDER BY COALESCE(fit_score,0) DESC,last_seen DESC,id ASC LIMIT {}",
            JOB_PAGE_SIZE + 1
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut records = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let has_more = records.len() > JOB_PAGE_SIZE;
        records.truncate(JOB_PAGE_SIZE);
        let next_cursor = if has_more {
            records
                .last()
                .map(|(_, score, last_seen, id)| {
                    encode_job_cursor(&JobCursor {
                        score: *score,
                        last_seen: last_seen.clone(),
                        id: id.clone(),
                    })
                })
                .transpose()?
        } else {
            None
        };
        let items = records
            .into_iter()
            .map(|(payload, _, _, _)| {
                serde_json::from_str(&payload).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(JobPage {
            items,
            total,
            pending_detail_count,
            next_cursor,
        })
    }

    pub fn list_job_options(&self, query: &str) -> Result<Vec<JobOption>, String> {
        let connection = self.connect()?;
        let normalized = query.trim().to_lowercase();
        let pattern = escaped_like_pattern(&normalized);
        let mut statement = connection
            .prepare(
                "SELECT id,title,company,last_seen FROM jobs WHERE ?1='' OR search_text LIKE ?2 ESCAPE '\\' ORDER BY last_seen DESC,id ASC LIMIT 50",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![normalized, pattern], |row| {
                Ok(JobOption {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    company: row.get(2)?,
                    last_seen: row.get(3)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn list_job_cities(&self) -> Result<Vec<String>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT DISTINCT city FROM jobs WHERE city<>'' ORDER BY city COLLATE NOCASE")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn job_ids_for_query(&self, query: &JobQuery) -> Result<Vec<String>, String> {
        let mut query = query.clone();
        query.cursor = None;
        let (where_clause, values) = job_query_where(&query, false)?;
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(&format!("SELECT id FROM jobs WHERE {where_clause} ORDER BY COALESCE(fit_score,0) DESC,last_seen DESC,id ASC"))
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn pending_detail_jobs(&self) -> Result<Vec<Job>, String> {
        self.list_json(
            "SELECT payload_json FROM jobs WHERE has_description=1 AND has_structured_details=0 ORDER BY last_seen DESC",
        )
    }

    pub fn delete_job(&self, job_id: &str) -> Result<i64, String> {
        let connection = self.connect()?;
        let changed = connection
            .execute("DELETE FROM jobs WHERE id=?1", [job_id])
            .map_err(|error| error.to_string())?;
        Ok(changed as i64)
    }

    pub fn delete_missing_description_jobs(&self, query: &JobQuery) -> Result<i64, String> {
        let mut query = query.clone();
        query.cursor = None;
        query.missing_description = true;
        let (where_clause, values) = job_query_where(&query, false)?;
        let connection = self.connect()?;
        let changed = connection
            .execute(
                &format!("DELETE FROM jobs WHERE {where_clause}"),
                params_from_iter(values.iter()),
            )
            .map_err(|error| error.to_string())?;
        Ok(changed as i64)
    }

    pub fn list_report_keywords(&self) -> Result<Vec<ReportKeyword>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                r#"SELECT keyword_key, MAX(keyword_label), COUNT(DISTINCT job_id), MAX(last_seen)
                   FROM job_keywords
                   GROUP BY keyword_key
                   ORDER BY MAX(last_seen) DESC, MAX(keyword_label) COLLATE NOCASE ASC"#,
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(ReportKeyword {
                    key: row.get(0)?,
                    label: row.get(1)?,
                    job_count: row.get(2)?,
                    last_seen: row.get(3)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn report_keywords_for_keys(
        &self,
        keyword_keys: &[String],
    ) -> Result<Vec<ReportKeyword>, String> {
        let requested = normalize_keyword_keys(keyword_keys);
        let requested = requested.into_iter().collect::<HashSet<_>>();
        Ok(self
            .list_report_keywords()?
            .into_iter()
            .filter(|keyword| requested.contains(&keyword.key))
            .collect())
    }

    pub fn list_jobs_by_keyword_keys(&self, keyword_keys: &[String]) -> Result<Vec<Job>, String> {
        let keyword_keys = normalize_keyword_keys(keyword_keys);
        if keyword_keys.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = (1..=keyword_keys.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            r#"SELECT DISTINCT jobs.payload_json
               FROM jobs
               INNER JOIN job_keywords ON job_keywords.job_id = jobs.id
               WHERE job_keywords.keyword_key IN ({placeholders})
               ORDER BY jobs.last_seen DESC, jobs.title ASC"#
        );
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(&query)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(keyword_keys.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| {
            let payload = row.map_err(|error| error.to_string())?;
            serde_json::from_str(&payload).map_err(|error| error.to_string())
        })
        .collect()
    }

    pub fn completed_detail_external_ids(&self, source: &str) -> Result<Vec<String>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT DISTINCT external_key FROM jobs WHERE lower(source)=lower(?1) AND has_description=1 AND external_key NOT LIKE 'fp:%' ORDER BY external_key")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([source], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn get_job(&self, id: &str) -> Result<Option<Job>, String> {
        let connection = self.connect()?;
        let json = connection
            .query_row("SELECT payload_json FROM jobs WHERE id = ?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| error.to_string())?;
        json.map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn upsert_jobs(&self, jobs: Vec<Job>) -> Result<UpsertStats, String> {
        self.upsert_jobs_internal(jobs, false, UpsertMode::Generic, None)
    }

    fn upsert_jobs_internal(
        &self,
        jobs: Vec<Job>,
        preserve_is_new_on_update: bool,
        mode: UpsertMode,
        keyword: Option<&str>,
    ) -> Result<UpsertStats, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let mut stats = UpsertStats::default();
        for job in jobs {
            let item = upsert_job_in_transaction(
                &transaction,
                job,
                preserve_is_new_on_update,
                mode,
                keyword,
            )?;
            stats.inserted += item.inserted;
            stats.updated += item.updated;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(stats)
    }

    pub fn upsert_job(&self, job: Job) -> Result<UpsertStats, String> {
        self.upsert_jobs(vec![job])
    }

    pub fn update_streamed_job(&self, job: Job) -> Result<UpsertStats, String> {
        self.upsert_jobs_internal(vec![job], true, UpsertMode::Generic, None)
    }

    pub fn upsert_scrape_list_job(&self, job: Job, keyword: &str) -> Result<UpsertStats, String> {
        self.upsert_jobs_internal(vec![job], false, UpsertMode::ScrapeList, Some(keyword))
    }

    pub fn upsert_scrape_detail_job(&self, job: Job, keyword: &str) -> Result<UpsertStats, String> {
        if job.description.trim().is_empty() {
            return Err("岗位详情为空，未写入数据库。".into());
        }
        self.upsert_jobs_internal(vec![job], true, UpsertMode::ScrapeDetail, Some(keyword))
    }

    pub fn save_job(&self, job: &Job) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(job).map_err(|error| error.to_string())?;
        let meta = JobQueryMetadata::from_job(job);
        let changed = connection
            .execute(
                "UPDATE jobs SET payload_json=?1,last_seen=?2,search_text=?3,salary_min=?4,salary_max=?5,company_scale_code=?6,city=?7,query_is_new=?8,fit_score=?9,has_description=?10,has_structured_details=?11 WHERE id=?12",
                params![payload, job.last_seen, meta.search_text, meta.salary_min, meta.salary_max, meta.company_scale_code, meta.city, meta.is_new, meta.fit_score, meta.has_description, meta.has_structured_details, job.id],
            )
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            return Err(format!(
                "Job {} does not exist; changes were not saved.",
                job.id
            ));
        }
        Ok(())
    }

    pub fn active_resume(&self) -> Result<Option<ResumeProfile>, String> {
        let connection = self.connect()?;
        let json = connection
            .query_row("SELECT payload_json FROM resume_profiles WHERE is_active = 1 ORDER BY updated_at DESC LIMIT 1", [], |row| row.get::<_, String>(0))
            .optional()
            .map_err(|error| error.to_string())?;
        json.map(|value| {
            let mut resume: ResumeProfile =
                serde_json::from_str(&value).map_err(|error| error.to_string())?;
            ensure_resume_item_ids(&mut resume);
            Ok(resume)
        })
        .transpose()
    }

    pub fn save_resume(&self, resume: &ResumeProfile) -> Result<(), String> {
        let previous_confirmed = self
            .active_resume()?
            .as_ref()
            .map(confirmed_fact_signature)
            .unwrap_or_default();
        let mut resume = resume.clone();
        ensure_resume_item_ids(&mut resume);
        validate_resume_facts(&mut resume)?;
        let confirmed_changed = previous_confirmed != confirmed_fact_signature(&resume);
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE resume_profiles SET is_active = 0", [])
            .map_err(|error| error.to_string())?;
        let payload = serde_json::to_string(&resume).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO resume_profiles(id, payload_json, updated_at, is_active) VALUES (?1, ?2, ?3, 1) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json, updated_at=excluded.updated_at, is_active=1",
                params![resume.id, payload, resume.updated_at],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,profile_json) VALUES (?1,?2,?3,NULL,?4,'legacy','保存的简历版本',?5)",
                params![uuid::Uuid::new_v4().to_string(), resume.id, resume.version, resume.updated_at, payload],
            )
            .map_err(|error| error.to_string())?;
        if confirmed_changed {
            clear_job_greetings(&transaction)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_resume(
        &self,
        mut candidate: ResumeProfile,
        expected_version: i64,
        source: &str,
        summary: &str,
        job_id: Option<String>,
        proposal_id: Option<String>,
        restored_from_version: Option<i64>,
    ) -> Result<ResumeCommitResult, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let current_json: Option<String> = transaction
            .query_row(
                "SELECT payload_json FROM resume_profiles WHERE is_active=1 ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let current = current_json
            .map(|payload| {
                serde_json::from_str::<ResumeProfile>(&payload).map_err(|error| error.to_string())
            })
            .transpose()?;
        let previous_confirmed = current
            .as_ref()
            .map(confirmed_fact_signature)
            .unwrap_or_default();
        let parent_version = current.as_ref().map(|resume| resume.version);
        match current {
            Some(current) => {
                if current.version != expected_version {
                    return Err(format!(
                        "version_conflict: 当前简历为 v{}，请刷新后重新生成建议。",
                        current.version
                    ));
                }
                candidate.id = current.id;
                candidate.version = current.version + 1;
            }
            None => {
                if expected_version != 0 {
                    return Err("version_conflict: 当前没有可提交的主简历。".into());
                }
                if candidate.id.trim().is_empty() {
                    candidate.id = uuid::Uuid::new_v4().to_string();
                }
                candidate.version = 1;
            }
        }
        candidate.updated_at = time::shanghai_rfc3339();
        ensure_resume_item_ids(&mut candidate);
        validate_resume_facts(&mut candidate)?;
        let confirmed_changed = previous_confirmed != confirmed_fact_signature(&candidate);
        let payload = serde_json::to_string(&candidate).map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE resume_profiles SET is_active=0", [])
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO resume_profiles(id,payload_json,updated_at,is_active) VALUES (?1,?2,?3,1) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json,updated_at=excluded.updated_at,is_active=1",
                params![candidate.id, payload, candidate.updated_at],
            )
            .map_err(|error| error.to_string())?;
        let version = ResumeVersionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            resume_id: candidate.id.clone(),
            version: candidate.version,
            parent_version,
            created_at: candidate.updated_at.clone(),
            source: source.into(),
            summary: summary.into(),
            job_id,
            proposal_id,
            restored_from_version,
        };
        transaction
            .execute(
                "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![version.id, version.resume_id, version.version, version.parent_version, version.created_at, version.source, version.summary, version.job_id, version.proposal_id, version.restored_from_version, payload],
            )
            .map_err(|error| error.to_string())?;
        if confirmed_changed {
            clear_job_greetings(&transaction)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(ResumeCommitResult {
            resume: candidate,
            version,
        })
    }

    pub fn list_resume_variants(&self) -> Result<Vec<ResumeVariantSummary>, String> {
        let connection = self.connect()?;
        let master_version = active_resume_version(&connection)?;
        let mut statement = connection
            .prepare(
                "SELECT v.id,v.job_id,j.title,j.company,v.name,v.base_resume_id,v.base_resume_version,v.version,v.created_at,v.updated_at
                 FROM resume_variants v JOIN jobs j ON j.id=v.job_id ORDER BY v.updated_at DESC,v.id ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let base_resume_version: i64 = row.get(6)?;
                Ok(ResumeVariantSummary {
                    id: row.get(0)?,
                    job_id: row.get(1)?,
                    job_title: row.get(2)?,
                    company: row.get(3)?,
                    name: row.get(4)?,
                    base_resume_id: row.get(5)?,
                    base_resume_version,
                    version: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    stale: master_version.is_some_and(|version| version > base_resume_version),
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn get_resume_variant(&self, id: &str) -> Result<Option<ResumeVariantDetail>, String> {
        self.resume_variant_by("v.id", id)
    }

    pub fn get_resume_variant_for_job(
        &self,
        job_id: &str,
    ) -> Result<Option<ResumeVariantDetail>, String> {
        self.resume_variant_by("v.job_id", job_id)
    }

    fn resume_variant_by(
        &self,
        column: &str,
        value: &str,
    ) -> Result<Option<ResumeVariantDetail>, String> {
        let connection = self.connect()?;
        let master_version = active_resume_version(&connection)?;
        let sql = format!(
            "SELECT v.id,v.job_id,j.title,j.company,v.name,v.base_resume_id,v.base_resume_version,v.version,v.created_at,v.updated_at,v.payload_json
             FROM resume_variants v JOIN jobs j ON j.id=v.job_id WHERE {column}=?1"
        );
        let record: Option<(ResumeVariantSummary, String)> = connection
            .query_row(&sql, [value], |row| {
                let base_resume_version: i64 = row.get(6)?;
                Ok((
                    ResumeVariantSummary {
                        id: row.get(0)?,
                        job_id: row.get(1)?,
                        job_title: row.get(2)?,
                        company: row.get(3)?,
                        name: row.get(4)?,
                        base_resume_id: row.get(5)?,
                        base_resume_version,
                        version: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                        stale: master_version.is_some_and(|version| version > base_resume_version),
                    },
                    row.get(10)?,
                ))
            })
            .optional()
            .map_err(|error| error.to_string())?;
        record
            .map(|(summary, payload)| {
                let mut profile: ResumeProfile =
                    serde_json::from_str(&payload).map_err(|error| error.to_string())?;
                ensure_resume_item_ids(&mut profile);
                Ok(ResumeVariantDetail { summary, profile })
            })
            .transpose()
    }

    pub fn create_resume_variant(
        &self,
        job_id: &str,
        expected_resume_version: i64,
    ) -> Result<ResumeVariantDetail, String> {
        // Creation is idempotent per job. Reopening an existing variant must not depend on the
        // current master resume, so the expected version only applies to the initial clone below.
        if let Some(existing) = self.get_resume_variant_for_job(job_id)? {
            return Ok(existing);
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let (job_title, company): (String, String) = transaction
            .query_row(
                "SELECT title,company FROM jobs WHERE id=?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "job_not_found: 岗位不存在。".to_string())?;
        let master_payload: String = transaction
            .query_row("SELECT payload_json FROM resume_profiles WHERE is_active=1 ORDER BY updated_at DESC LIMIT 1", [], |row| row.get(0))
            .optional().map_err(|error| error.to_string())?
            .ok_or_else(|| "resume_not_found: 请先导入主简历。".to_string())?;
        let master: ResumeProfile =
            serde_json::from_str(&master_payload).map_err(|error| error.to_string())?;
        if master.version != expected_resume_version {
            return Err(format!(
                "version_conflict: 当前主简历为 v{}，请刷新后重试。",
                master.version
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = time::shanghai_rfc3339();
        let mut profile = master.clone();
        profile.id = id.clone();
        profile.version = 1;
        profile.updated_at = now.clone();
        ensure_resume_item_ids(&mut profile);
        let payload = serde_json::to_string(&profile).map_err(|error| error.to_string())?;
        let name = format!("{company} · {job_title}");
        transaction.execute(
            "INSERT INTO resume_variants(id,job_id,base_resume_id,base_resume_version,version,name,created_at,updated_at,payload_json) VALUES (?1,?2,?3,?4,1,?5,?6,?6,?7)",
            params![id, job_id, master.id, master.version, name, now, payload],
        ).map_err(|error| error.to_string())?;
        transaction.execute(
            "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,profile_json) VALUES (?1,?2,1,NULL,?3,'variant-create','创建岗位版本',?4,?5)",
            params![uuid::Uuid::new_v4().to_string(), id, now, job_id, payload],
        ).map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        self.get_resume_variant(&id)?
            .ok_or_else(|| "storage_error: 岗位版本创建后无法读取。".into())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_resume_variant(
        &self,
        variant_id: &str,
        mut candidate: ResumeProfile,
        expected_version: i64,
        source: &str,
        summary: &str,
        restored_from_version: Option<i64>,
        new_base_resume_version: Option<i64>,
    ) -> Result<ResumeVariantCommitResult, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let current_record: Option<(String, i64, String, i64)> = transaction.query_row(
            "SELECT payload_json,version,job_id,base_resume_version FROM resume_variants WHERE id=?1",
            [variant_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional().map_err(|error| error.to_string())?;
        let (current_payload, current_version, job_id, base_resume_version) =
            current_record.ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
        if current_version != expected_version {
            return Err(format!(
                "version_conflict: 当前岗位版本为 v{current_version}，请刷新后重试。"
            ));
        }
        let current: ResumeProfile =
            serde_json::from_str(&current_payload).map_err(|error| error.to_string())?;
        candidate.id = variant_id.to_string();
        candidate.version = current_version + 1;
        candidate.updated_at = time::shanghai_rfc3339();
        if new_base_resume_version.is_none() {
            candidate.facts = current.facts;
            candidate.preferences = current.preferences;
        }
        ensure_resume_item_ids(&mut candidate);
        validate_resume_facts(&mut candidate)?;
        let payload = serde_json::to_string(&candidate).map_err(|error| error.to_string())?;
        let base_version = new_base_resume_version.unwrap_or(base_resume_version);
        let changed = transaction.execute(
            "UPDATE resume_variants SET payload_json=?1,version=?2,base_resume_version=?3,updated_at=?4 WHERE id=?5 AND version=?6",
            params![payload, candidate.version, base_version, candidate.updated_at, variant_id, expected_version],
        ).map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("version_conflict: 岗位版本已变化，请刷新后重试。".into());
        }
        let version = ResumeVersionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            resume_id: variant_id.to_string(),
            version: candidate.version,
            parent_version: Some(current_version),
            created_at: candidate.updated_at.clone(),
            source: source.into(),
            summary: summary.into(),
            job_id: Some(job_id.clone()),
            proposal_id: None,
            restored_from_version,
        };
        transaction.execute(
            "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9,?10)",
            params![version.id, version.resume_id, version.version, version.parent_version, version.created_at, version.source, version.summary, job_id, version.restored_from_version, payload],
        ).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM resume_coverage_cache WHERE target_kind='variant' AND target_id=?1",
                [variant_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        let variant = self
            .get_resume_variant(variant_id)?
            .ok_or_else(|| "storage_error: 岗位版本保存后无法读取。".to_string())?;
        Ok(ResumeVariantCommitResult { variant, version })
    }

    pub fn delete_resume_variant(&self, variant_id: &str) -> Result<i64, String> {
        let connection = self.connect()?;
        let changed = connection
            .execute("DELETE FROM resume_variants WHERE id=?1", [variant_id])
            .map_err(|error| error.to_string())?;
        Ok(changed as i64)
    }

    pub fn restore_resume_variant_version(
        &self,
        variant_id: &str,
        version_id: &str,
        expected_version: i64,
    ) -> Result<ResumeVariantCommitResult, String> {
        let detail = self
            .get_resume_version(version_id)?
            .ok_or_else(|| "简历版本不存在。".to_string())?;
        if detail.summary.resume_id != variant_id {
            return Err("不能把其他简历的历史恢复到当前岗位版本。".into());
        }
        self.commit_resume_variant(
            variant_id,
            detail.profile,
            expected_version,
            "variant-rollback",
            &format!("恢复到 v{} 的内容", detail.summary.version),
            Some(detail.summary.version),
            None,
        )
    }

    pub fn preview_resume_variant_rebase(
        &self,
        variant_id: &str,
    ) -> Result<ResumeRebasePreview, String> {
        let variant = self
            .get_resume_variant(variant_id)?
            .ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
        let master = self
            .active_resume()?
            .ok_or_else(|| "resume_not_found: 当前没有主简历。".to_string())?;
        if master.id != variant.summary.base_resume_id {
            return Err(
                "base_resume_changed: 当前主简历与岗位版本基线不一致，请重新创建岗位版本。".into(),
            );
        }
        let base = self
            .resume_version_profile(
                &variant.summary.base_resume_id,
                variant.summary.base_resume_version,
            )?
            .ok_or_else(|| "base_version_missing: 岗位版本的主简历基线已不存在。".to_string())?;
        let (auto_changes, conflicts) = compute_rebase_changes(&base, &master, &variant.profile)?;
        Ok(ResumeRebasePreview {
            variant_id: variant.summary.id,
            variant_version: variant.summary.version,
            base_resume_version: variant.summary.base_resume_version,
            master_version: master.version,
            auto_changes,
            conflicts,
        })
    }

    pub fn apply_resume_variant_rebase(
        &self,
        variant_id: &str,
        expected_variant_version: i64,
        expected_master_version: i64,
        resolutions: &[ResumeRebaseResolution],
    ) -> Result<ResumeVariantCommitResult, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;

        let variant_record: Option<(String, i64, String, String, i64)> = transaction
            .query_row(
                "SELECT payload_json,version,job_id,base_resume_id,base_resume_version FROM resume_variants WHERE id=?1",
                [variant_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let (variant_payload, variant_version, job_id, base_resume_id, base_resume_version) =
            variant_record.ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
        if variant_version != expected_variant_version {
            return Err(format!(
                "version_conflict: 当前岗位版本为 v{variant_version}，请刷新后重试。"
            ));
        }
        let variant: ResumeProfile =
            serde_json::from_str(&variant_payload).map_err(|error| error.to_string())?;

        let master_payload: String = transaction
            .query_row(
                "SELECT payload_json FROM resume_profiles WHERE is_active=1 ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "resume_not_found: 当前没有主简历。".to_string())?;
        let master: ResumeProfile =
            serde_json::from_str(&master_payload).map_err(|error| error.to_string())?;
        if master.version != expected_master_version {
            return Err(format!(
                "version_conflict: 当前主简历为 v{}，请重新检查同步差异。",
                master.version
            ));
        }
        if master.id != base_resume_id {
            return Err(
                "base_resume_changed: 当前主简历与岗位版本基线不一致，请重新创建岗位版本。".into(),
            );
        }

        let base_payload: String = transaction
            .query_row(
                "SELECT profile_json FROM resume_versions WHERE resume_id=?1 AND version=?2",
                params![base_resume_id, base_resume_version],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "base_version_missing: 岗位版本的主简历基线已不存在。".to_string())?;
        let base: ResumeProfile =
            serde_json::from_str(&base_payload).map_err(|error| error.to_string())?;
        let (auto_changes, conflicts) = compute_rebase_changes(&base, &master, &variant)?;

        let mut value = serde_json::to_value(&variant).map_err(|error| error.to_string())?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "storage_error: 岗位版本结构无效。".to_string())?;
        let resolution_map = resolutions
            .iter()
            .map(|item| (item.path.as_str(), item.choice.as_str()))
            .collect::<std::collections::HashMap<_, _>>();
        for change in &auto_changes {
            object.insert(
                change.path.trim_start_matches('/').into(),
                change.master.clone(),
            );
        }
        for conflict in &conflicts {
            match resolution_map.get(conflict.path.as_str()).copied() {
                Some("master") => {
                    object.insert(
                        conflict.path.trim_start_matches('/').into(),
                        conflict.master.clone(),
                    );
                }
                Some("variant") => {}
                _ => {
                    return Err(format!(
                        "invalid_request: 请处理字段“{}”的同步冲突。",
                        conflict.label
                    ))
                }
            }
        }
        object.insert(
            "facts".into(),
            serde_json::to_value(&master.facts).map_err(|error| error.to_string())?,
        );
        object.insert(
            "preferences".into(),
            serde_json::to_value(&master.preferences).map_err(|error| error.to_string())?,
        );
        let mut candidate: ResumeProfile =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        candidate.id = variant_id.to_string();
        candidate.version = variant_version + 1;
        candidate.updated_at = time::shanghai_rfc3339();
        ensure_resume_item_ids(&mut candidate);
        validate_resume_facts(&mut candidate)?;
        let payload = serde_json::to_string(&candidate).map_err(|error| error.to_string())?;
        let changed = transaction
            .execute(
                "UPDATE resume_variants SET payload_json=?1,version=?2,base_resume_version=?3,updated_at=?4 WHERE id=?5 AND version=?6",
                params![
                    payload,
                    candidate.version,
                    master.version,
                    candidate.updated_at,
                    variant_id,
                    expected_variant_version
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("version_conflict: 岗位版本已变化，请刷新后重试。".into());
        }
        let version = ResumeVersionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            resume_id: variant_id.to_string(),
            version: candidate.version,
            parent_version: Some(variant_version),
            created_at: candidate.updated_at.clone(),
            source: "variant-rebase".into(),
            summary: format!("同步主简历 v{}", master.version),
            job_id: Some(job_id.clone()),
            proposal_id: None,
            restored_from_version: None,
        };
        transaction
            .execute(
                "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,NULL,?9)",
                params![
                    version.id,
                    version.resume_id,
                    version.version,
                    version.parent_version,
                    version.created_at,
                    version.source,
                    version.summary,
                    job_id,
                    payload
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM resume_coverage_cache WHERE target_kind='variant' AND target_id=?1",
                [variant_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;

        let variant = self
            .get_resume_variant(variant_id)?
            .ok_or_else(|| "storage_error: 岗位版本同步后无法读取。".to_string())?;
        Ok(ResumeVariantCommitResult { variant, version })
    }

    fn resume_version_profile(
        &self,
        resume_id: &str,
        version: i64,
    ) -> Result<Option<ResumeProfile>, String> {
        let connection = self.connect()?;
        let payload: Option<String> = connection
            .query_row(
                "SELECT profile_json FROM resume_versions WHERE resume_id=?1 AND version=?2",
                params![resume_id, version],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn resume_coverage_cache(
        &self,
        cache_key: &str,
    ) -> Result<Option<ResumeCoverageReport>, String> {
        let connection = self.connect()?;
        let payload: Option<String> = connection
            .query_row(
                "SELECT payload_json FROM resume_coverage_cache WHERE cache_key=?1",
                [cache_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn save_resume_coverage_cache(
        &self,
        cache_key: &str,
        job_fingerprint: &str,
        provider_fingerprint: &str,
        skill_version: &str,
        report: &ResumeCoverageReport,
    ) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(report).map_err(|error| error.to_string())?;
        connection.execute(
            "INSERT INTO resume_coverage_cache(cache_key,target_kind,target_id,target_version,job_id,job_fingerprint,provider_fingerprint,skill_version,generated_at,payload_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(cache_key) DO UPDATE SET generated_at=excluded.generated_at,payload_json=excluded.payload_json",
            params![cache_key, report.target.kind, report.target.id, report.target_version, report.job_id, job_fingerprint, provider_fingerprint, skill_version, report.generated_at, payload],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn list_resume_versions(
        &self,
        resume_id: &str,
    ) -> Result<Vec<ResumeVersionSummary>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version FROM resume_versions WHERE resume_id=?1 ORDER BY version DESC")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([resume_id], resume_version_from_row)
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn get_resume_version(&self, id: &str) -> Result<Option<ResumeVersionDetail>, String> {
        let connection = self.connect()?;
        let record: Option<(ResumeVersionSummary, String)> = connection
            .query_row(
                "SELECT id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json FROM resume_versions WHERE id=?1",
                [id],
                |row| Ok((resume_version_from_row(row)?, row.get(10)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        record
            .map(|(summary, payload)| {
                let mut profile: ResumeProfile =
                    serde_json::from_str(&payload).map_err(|error| error.to_string())?;
                ensure_resume_item_ids(&mut profile);
                Ok(ResumeVersionDetail { summary, profile })
            })
            .transpose()
    }

    pub fn list_tasks(&self) -> Result<Vec<TaskRun>, String> {
        self.list_json("SELECT payload_json FROM task_runs ORDER BY updated_at DESC LIMIT 30")
    }

    pub fn save_task(&self, task: &TaskRun) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(task).map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO task_runs(id, payload_json, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json, updated_at=excluded.updated_at", params![task.id, payload, task.updated_at]).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn reserve_task(&self, task: &TaskRun) -> Result<bool, String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(task).map_err(|error| error.to_string())?;
        let changed = connection.execute(
            "INSERT INTO task_runs(id,payload_json,updated_at) SELECT ?1,?2,?3 WHERE NOT EXISTS (SELECT 1 FROM task_runs WHERE json_extract(payload_json,'$.kind')=?4 AND json_extract(payload_json,'$.state') IN ('queued','running'))",
            params![task.id, payload, task.updated_at, task.kind],
        ).map_err(|error| error.to_string())?;
        Ok(changed == 1)
    }

    pub fn reserve_scrape_task(
        &self,
        task: &TaskRun,
        search_spec: &SearchSpec,
    ) -> Result<bool, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let task_payload = serde_json::to_string(task).map_err(|error| error.to_string())?;
        let changed = transaction.execute(
            "INSERT INTO task_runs(id,payload_json,updated_at) SELECT ?1,?2,?3 WHERE NOT EXISTS (SELECT 1 FROM task_runs WHERE json_extract(payload_json,'$.kind')=?4 AND json_extract(payload_json,'$.state') IN ('queued','running'))",
            params![task.id, task_payload, task.updated_at, task.kind],
        ).map_err(|error| error.to_string())?;
        if changed == 1 {
            let spec_payload =
                serde_json::to_string(search_spec).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO app_settings(key,payload_json) VALUES ('last_search_spec',?1) ON CONFLICT(key) DO UPDATE SET payload_json=excluded.payload_json",
                    [spec_payload],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(changed == 1)
    }

    pub fn last_search_spec(&self) -> Result<Option<SearchSpec>, String> {
        let connection = self.connect()?;
        let json = connection
            .query_row(
                "SELECT payload_json FROM app_settings WHERE key='last_search_spec'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        json.map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn list_scrape_runs(&self) -> Result<Vec<ScrapeRun>, String> {
        self.list_json("SELECT payload_json FROM scrape_runs ORDER BY started_at DESC LIMIT 20")
    }

    pub fn save_scrape_run(&self, run: &ScrapeRun) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(run).map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO scrape_runs(id, payload_json, started_at) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json", params![run.id, payload, run.started_at]).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn settings(&self) -> Result<AppSettings, String> {
        let connection = self.connect()?;
        let json = connection
            .query_row(
                "SELECT payload_json FROM app_settings WHERE key='main'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        json.map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
            .map(|value| value.unwrap_or_default())
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(settings).map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO app_settings(key, payload_json) VALUES ('main', ?1) ON CONFLICT(key) DO UPDATE SET payload_json=excluded.payload_json", [payload]).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())
    }

    pub fn has_active_tasks(&self) -> Result<bool, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_runs WHERE json_extract(payload_json,'$.state') IN ('queued','running'))",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())
    }

    pub fn backup_to(&self, destination: &Path) -> Result<(), String> {
        let source = self.connect()?;
        Self::backup_connection(&source, destination)
    }

    pub fn copy_database(source: &Path, destination: &Path) -> Result<(), String> {
        let connection = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| error.to_string())?;
        Self::backup_connection(&connection, destination)
    }

    fn backup_connection(source: &Connection, destination: &Path) -> Result<(), String> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if destination.exists() {
            std::fs::remove_file(destination).map_err(|error| error.to_string())?;
        }
        let mut target = Connection::open(destination).map_err(|error| error.to_string())?;
        {
            let backup = Backup::new(source, &mut target).map_err(|error| error.to_string())?;
            backup
                .run_to_completion(128, std::time::Duration::from_millis(5), None)
                .map_err(|error| error.to_string())?;
        }
        drop(target);
        Self::validate_database(destination).map(|_| ())
    }

    pub fn validate_database(path: &Path) -> Result<i64, String> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| format!("cannot open backup: {error}"))?;
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|error| format!("cannot check backup integrity: {error}"))?;
        if integrity != "ok" {
            return Err(format!("backup integrity check failed: {integrity}"));
        }
        let migrations_exist: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if !migrations_exist {
            return Err("backup does not contain a supported schema".into());
        }
        connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())
    }

    pub fn clear_provider_secret_references(&self) -> Result<(), String> {
        let providers = self.list_providers()?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        for mut provider in providers {
            provider.api_key = None;
            provider.api_key_ref = None;
            provider.verified = false;
            provider.vision_verified = false;
            provider.last_tested_at = None;
            provider.last_test_error = None;
            let payload = serde_json::to_string(&provider).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE ai_providers SET payload_json=?1 WHERE id=?2",
                    params![payload, provider.id],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn boss_profile_state(&self) -> Result<BossProfileState, String> {
        let connection = self.connect()?;
        let payload = connection
            .query_row(
                "SELECT payload_json FROM app_settings WHERE key='boss_profile_state'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
            .map(|value| value.unwrap_or_default())
    }

    pub fn save_boss_profile_state(&self, state: &BossProfileState) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(state).map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO app_settings(key,payload_json) VALUES ('boss_profile_state',?1) ON CONFLICT(key) DO UPDATE SET payload_json=excluded.payload_json",
                [payload],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn running_task(&self, kind: &str) -> Result<Option<TaskRun>, String> {
        let connection = self.connect()?;
        let payload = connection
            .query_row(
                "SELECT payload_json FROM task_runs WHERE json_extract(payload_json,'$.kind')=?1 AND json_extract(payload_json,'$.state') IN ('queued','running') ORDER BY updated_at DESC LIMIT 1",
                [kind],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn recover_interrupted_tasks(&self) -> Result<(), String> {
        let tasks = self.list_tasks()?;
        for mut task in tasks
            .into_iter()
            .filter(|task| matches!(task.state.as_str(), "queued" | "running"))
        {
            task.state = "failed".into();
            task.progress = 100;
            task.message = "上次运行被应用退出中断，请重试".into();
            task.recoverable_error = Some("应用在任务完成前退出。".into());
            task.updated_at = time::shanghai_rfc3339();
            self.save_task(&task)?;
        }
        let mut boss = self.boss_profile_state()?;
        if boss.last_attempt_status == "running" {
            boss.configured = false;
            boss.last_attempt_status = "failed".into();
            boss.last_attempt_at = Some(time::shanghai_rfc3339());
            boss.last_error = Some("上次配置被应用退出中断，请重新配置。".into());
            self.save_boss_profile_state(&boss)?;
        }
        Ok(())
    }

    pub fn interview_preparation_by_key(
        &self,
        cache_key: &str,
    ) -> Result<Option<InterviewPreparationCacheRecord>, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json FROM interview_preparation_cache WHERE cache_key=?1",
                [cache_key],
                interview_cache_from_row,
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn latest_interview_preparation(
        &self,
        scope_key: &str,
    ) -> Result<Option<InterviewPreparationCacheRecord>, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json FROM interview_preparation_cache WHERE scope_key=?1 ORDER BY generated_at DESC LIMIT 1",
                [scope_key],
                interview_cache_from_row,
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn save_interview_preparation(
        &self,
        record: &InterviewPreparationCacheRecord,
    ) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let payload =
            serde_json::to_string(&record.preparation).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO interview_preparation_cache(cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(cache_key) DO UPDATE SET scope_key=excluded.scope_key,dataset_hash=excluded.dataset_hash,resume_id=excluded.resume_id,resume_version=excluded.resume_version,provider_fingerprint=excluded.provider_fingerprint,skill_version=excluded.skill_version,generated_at=excluded.generated_at,payload_json=excluded.payload_json",
                params![record.cache_key, record.scope_key, record.dataset_hash, record.resume_id, record.resume_version, record.provider_fingerprint, record.skill_version, record.generated_at, payload],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM interview_preparation_cache WHERE cache_key NOT IN (SELECT cache_key FROM interview_preparation_cache ORDER BY generated_at DESC LIMIT 10)",
                [],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn list_json<T: serde::de::DeserializeOwned>(&self, query: &str) -> Result<Vec<T>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(query)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.map(|row| {
            let json = row.map_err(|error| error.to_string())?;
            serde_json::from_str(&json).map_err(|error| error.to_string())
        })
        .collect()
    }
}

fn upsert_job_in_transaction(
    transaction: &Transaction<'_>,
    mut job: Job,
    preserve_is_new_on_update: bool,
    mode: UpsertMode,
    keyword: Option<&str>,
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
        conditions.push("query_is_new=1".into());
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
            values.push(cursor.score.into());
            let score_less = values.len();
            values.push(cursor.score.into());
            let score_equal = values.len();
            values.push(cursor.last_seen.clone().into());
            let seen_less = values.len();
            values.push(cursor.last_seen.into());
            let seen_equal = values.len();
            values.push(cursor.id.into());
            let id_after = values.len();
            conditions.push(format!(
                "(COALESCE(fit_score,0)<?{score_less} OR (COALESCE(fit_score,0)=?{score_equal} AND (last_seen<?{seen_less} OR (last_seen=?{seen_equal} AND id>?{id_after}))))"
            ));
        }
    }
    Ok((conditions.join(" AND "), values))
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
    use crate::models::{InterviewPreparation, Job, JobStructuredDetails, ResumeFact};
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

        assert_eq!(db.schema_version().unwrap(), 6);
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
        assert_eq!(db.schema_version().unwrap(), 6);

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
    fn streamed_detail_update_keeps_new_status() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.initialize().unwrap();
        db.upsert_job(job("id-1", "20-30K")).unwrap();
        db.update_streamed_job(job("id-1", "25-35K")).unwrap();

        let jobs = db.list_jobs().unwrap();
        assert_eq!(jobs[0].salary, "25-35K");
        assert!(jobs[0].is_new);
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
        let second = db
            .list_jobs_page(&JobQuery {
                cursor: first.next_cursor,
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
