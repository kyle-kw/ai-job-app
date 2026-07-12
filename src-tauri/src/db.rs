use crate::models::{
    AiProviderConfig, AppSettings, BossProfileState, InterviewPreparation, Job, ReportKeyword,
    ResumeCommitResult, ResumeEducation, ResumeProfile, ResumeVersionDetail, ResumeVersionSummary, ScrapeRun,
    TaskRun,
};
use crate::time;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const HISTORICAL_KEYWORD_KEY: &str = "__historical_unclassified__";
const HISTORICAL_KEYWORD_LABEL: &str = "历史未分类";

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
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

impl Database {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path).map_err(|error| error.to_string())?;
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

    pub fn initialize(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let connection = self.connect()?;
        connection
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
        self.migrate_v2()?;
        self.migrate_v3()?;
        self.recover_interrupted_tasks()?;
        Ok(())
    }

    fn migrate_v2(&self) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
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
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn migrate_v3(&self) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
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
            let found = columns
                .filter_map(Result::ok)
                .any(|name| name == "scope_key");
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
        transaction.commit().map_err(|error| error.to_string())?;
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
        let mut ids = self
            .list_jobs()?
            .into_iter()
            .filter(|job| {
                job.source.eq_ignore_ascii_case(source)
                    && !job.external_id.trim().is_empty()
                    && !job.description.trim().is_empty()
            })
            .map(|job| job.external_id)
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        Ok(ids)
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
        connection
            .execute(
                "UPDATE jobs SET payload_json = ?1, last_seen = ?2 WHERE id = ?3",
                params![payload, job.last_seen, job.id],
            )
            .map_err(|error| error.to_string())?;
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

    pub fn list_providers(&self) -> Result<Vec<AiProviderConfig>, String> {
        self.list_json("SELECT payload_json FROM ai_providers ORDER BY rowid")
    }

    pub fn provider_by_id(&self, id: &str) -> Result<Option<AiProviderConfig>, String> {
        let connection = self.connect()?;
        let payload = connection
            .query_row(
                "SELECT payload_json FROM ai_providers WHERE id=?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn default_provider(&self) -> Result<Option<AiProviderConfig>, String> {
        Ok(self
            .list_providers()?
            .into_iter()
            .find(|provider| provider.is_default && provider.verified))
    }

    pub fn save_provider(&self, provider: &AiProviderConfig) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        if provider.is_default {
            let mut providers = {
                let mut statement = transaction
                    .prepare("SELECT payload_json FROM ai_providers")
                    .map_err(|error| error.to_string())?;
                let rows = statement
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(|error| error.to_string())?;
                rows.filter_map(|row| row.ok())
                    .filter_map(|payload| serde_json::from_str::<AiProviderConfig>(&payload).ok())
                    .collect::<Vec<_>>()
            };
            for item in &mut providers {
                if item.id != provider.id && item.is_default {
                    item.is_default = false;
                    let payload = serde_json::to_string(item).map_err(|error| error.to_string())?;
                    transaction
                        .execute(
                            "UPDATE ai_providers SET payload_json=?1 WHERE id=?2",
                            params![payload, item.id],
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        let payload = serde_json::to_string(provider).map_err(|error| error.to_string())?;
        transaction.execute("INSERT INTO ai_providers(id, payload_json) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json", params![provider.id, payload]).map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
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
        Ok(self
            .list_tasks()?
            .into_iter()
            .find(|task| task.kind == kind && matches!(task.state.as_str(), "queued" | "running")))
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

    pub fn bool_flag(&self, key: &str) -> Result<bool, String> {
        let connection = self.connect()?;
        let value = connection
            .query_row(
                "SELECT payload_json FROM app_settings WHERE key=?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        Ok(value.as_deref() == Some("true"))
    }

    pub fn set_bool_flag(&self, key: &str, value: bool) -> Result<(), String> {
        let connection = self.connect()?;
        connection.execute("INSERT INTO app_settings(key, payload_json) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET payload_json=excluded.payload_json", params![key, if value { "true" } else { "false" }]).map_err(|error| error.to_string())?;
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
    transaction
        .execute(
            r#"INSERT INTO jobs(id, source, external_key, fingerprint, title, company, location, first_seen, last_seen, payload_json)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               ON CONFLICT(source, external_key) DO UPDATE SET
                 fingerprint=excluded.fingerprint, title=excluded.title, company=excluded.company,
                 location=excluded.location, last_seen=excluded.last_seen, payload_json=excluded.payload_json"#,
            params![job.id, job.source, external_key, fingerprint, job.title, job.company, job.location, job.first_seen, job.last_seen, payload],
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
    Some((captures.get(1)?.as_str().trim().to_string(), captures.get(2)?.as_str().trim().to_string()))
}

fn clean_date(value: &str) -> String {
    value.trim().trim_matches(|character: char| matches!(character, '-' | '\u{2013}' | '\u{2014}') || character.is_whitespace()).to_string()
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
}
