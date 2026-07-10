use crate::models::{AiProviderConfig, AppSettings, Job, ResumeProfile, ScrapeRun, TaskRun};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
}

#[derive(Debug, Default)]
pub struct UpsertStats {
    pub inserted: i64,
    pub updated: i64,
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
                INSERT OR IGNORE INTO schema_migrations(version, applied_at)
                VALUES (1, datetime('now'));
                "#,
            )
            .map_err(|error| error.to_string())?;
        self.seed_providers()?;
        Ok(())
    }

    fn seed_providers(&self) -> Result<(), String> {
        if !self.list_providers()?.is_empty() {
            return Ok(());
        }
        let providers = vec![
            AiProviderConfig {
                id: "provider-xiaomi".into(),
                kind: "xiaomi".into(),
                name: "小米 MiMo".into(),
                base_url: "https://api.xiaomimimo.com/v1".into(),
                model: "mimo-v2.5-pro".into(),
                api_key: None,
                api_key_ref: None,
                is_default: true,
                verified: false,
                last_tested_at: None,
            },
            AiProviderConfig {
                id: "provider-openrouter".into(),
                kind: "openrouter".into(),
                name: "OpenRouter 免费路由".into(),
                base_url: "https://openrouter.ai/api/v1".into(),
                model: "openrouter/free".into(),
                api_key: None,
                api_key_ref: None,
                is_default: false,
                verified: false,
                last_tested_at: None,
            },
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
                last_tested_at: None,
            },
        ];
        for provider in providers {
            self.save_provider(&provider)?;
        }
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
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let mut stats = UpsertStats::default();
        for mut job in jobs {
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
            if let Some(existing_json) = existing_json {
                let existing: Job =
                    serde_json::from_str(&existing_json).map_err(|error| error.to_string())?;
                job.id = existing.id;
                job.first_seen = existing.first_seen;
                job.fit = existing.fit;
                job.greeting = existing.greeting;
                job.patches = existing.patches;
                job.is_new = false;
                stats.updated += 1;
            } else {
                job.is_new = true;
                stats.inserted += 1;
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
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(stats)
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
        json.map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn save_resume(&self, resume: &ResumeProfile) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE resume_profiles SET is_active = 0", [])
            .map_err(|error| error.to_string())?;
        let payload = serde_json::to_string(resume).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO resume_profiles(id, payload_json, updated_at, is_active) VALUES (?1, ?2, ?3, 1) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json, updated_at=excluded.updated_at, is_active=1",
                params![resume.id, payload, resume.updated_at],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn list_providers(&self) -> Result<Vec<AiProviderConfig>, String> {
        self.list_json("SELECT payload_json FROM ai_providers ORDER BY rowid")
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
            let mut providers = self.list_providers().unwrap_or_default();
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
    use crate::models::Job;
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
        }
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
}
