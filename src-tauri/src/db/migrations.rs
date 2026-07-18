use super::*;

impl Database {
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
        Self::migrate_v7(&transaction)?;
        Self::migrate_v8(&transaction)?;
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

    fn migrate_v7(transaction: &Transaction<'_>) -> Result<(), String> {
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS report_competitiveness_cache (
                    cache_key TEXT PRIMARY KEY,
                    scope_key TEXT NOT NULL,
                    dataset_hash TEXT NOT NULL,
                    resume_id TEXT NOT NULL,
                    resume_version INTEGER NOT NULL,
                    provider_fingerprint TEXT NOT NULL,
                    skill_version TEXT NOT NULL,
                    generated_at TEXT NOT NULL,
                    payload_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_report_competitiveness_scope_generated
                    ON report_competitiveness_cache(scope_key, generated_at DESC);
                INSERT OR IGNORE INTO schema_migrations(version, applied_at)
                VALUES (7, datetime('now'));
                "#,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn migrate_v8(transaction: &Transaction<'_>) -> Result<(), String> {
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS job_scrape_runs (
                    run_id TEXT NOT NULL,
                    job_id TEXT NOT NULL,
                    was_inserted INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY(run_id, job_id),
                    FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_job_scrape_runs_latest_new
                    ON job_scrape_runs(run_id, was_inserted, job_id);
                INSERT OR IGNORE INTO job_scrape_runs(run_id, job_id, was_inserted)
                SELECT latest.id, jobs.id, 1
                FROM jobs
                JOIN (
                    SELECT id, payload_json
                    FROM scrape_runs
                    WHERE json_extract(payload_json,'$.completedAt') IS NOT NULL
                    ORDER BY started_at DESC, id DESC
                    LIMIT 1
                ) AS latest
                WHERE COALESCE(json_extract(jobs.payload_json,'$.isNew'),0)=1
                  AND jobs.first_seen>=json_extract(latest.payload_json,'$.startedAt')
                  AND jobs.first_seen<=json_extract(latest.payload_json,'$.completedAt');
                INSERT OR IGNORE INTO schema_migrations(version, applied_at)
                VALUES (8, datetime('now'));
                "#,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}
