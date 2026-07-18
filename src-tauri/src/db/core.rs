use super::*;

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

    pub(super) fn open_connection(path: &Path) -> Result<Connection, String> {
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
        connection
            .create_scalar_function(
                "report_has_skill",
                2,
                FunctionFlags::SQLITE_DETERMINISTIC,
                |context| {
                    let payload = context.get::<String>(0)?;
                    let skill = context.get::<String>(1)?;
                    Ok(serde_json::from_str::<Job>(&payload)
                        .is_ok_and(|job| analytics::job_has_skill(&job, &skill)))
                },
            )
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

    pub fn list_report_scrape_runs(&self) -> Result<Vec<ScrapeRun>, String> {
        self.list_json("SELECT payload_json FROM scrape_runs WHERE json_extract(payload_json,'$.completedAt') IS NOT NULL ORDER BY started_at DESC")
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

    pub(super) fn backup_connection(source: &Connection, destination: &Path) -> Result<(), String> {
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

    pub(super) fn list_json<T: serde::de::DeserializeOwned>(
        &self,
        query: &str,
    ) -> Result<Vec<T>, String> {
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
