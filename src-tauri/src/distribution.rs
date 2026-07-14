use crate::db::{Database, CURRENT_SCHEMA_VERSION};
use crate::models::{
    AppInfo, AppSettings, AppUpdateInfo, BackupInfo, BossProfileState, ChromeStatus,
    ClearDataItemResult, ClearDataResult, UpdateEvent,
};
use crate::{llm, secrets, sidecar, time, AppState};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_updater::UpdaterExt;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

pub const PRIVACY_ACKNOWLEDGED_VERSION: &str = "2026-07-14";
const IDENTIFIER: &str = "io.github.kylekw.aijobapp";
const LEGACY_IDENTIFIER: &str = "com.localfirst.aijobapp";
const DATABASE_NAME: &str = "ai-job-app.db";
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct LegacyMigration {
    pub legacy_dir: Option<PathBuf>,
    pub migrated: bool,
}

pub fn migrate_legacy_identity(data_dir: &Path) -> Result<LegacyMigration, String> {
    migrate_legacy_identity_with(data_dir, |path, contents| {
        std::fs::write(path, contents).map_err(|error| error.to_string())
    })
}

fn migrate_legacy_identity_with<F>(
    data_dir: &Path,
    write_marker: F,
) -> Result<LegacyMigration, String>
where
    F: FnOnce(&Path, &str) -> Result<(), String>,
{
    let legacy_dir = data_dir
        .parent()
        .map(|parent| parent.join(LEGACY_IDENTIFIER))
        .filter(|path| path.exists());
    let current_database = data_dir.join(DATABASE_NAME);
    let Some(legacy_dir_path) = legacy_dir.as_ref() else {
        return Ok(LegacyMigration {
            legacy_dir: None,
            migrated: false,
        });
    };
    let legacy_database = legacy_dir_path.join(DATABASE_NAME);
    if current_database.exists() || !legacy_database.exists() {
        return Ok(LegacyMigration {
            legacy_dir,
            migrated: false,
        });
    }

    std::fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
    let temporary = data_dir.join(format!(".{DATABASE_NAME}.identity-migration"));
    let marker = data_dir.join(".legacy-migrated-v0.2.0");
    let _ = std::fs::remove_file(&temporary);
    let migration_result = (|| {
        Database::copy_database(&legacy_database, &temporary)?;
        Database::validate_database(&temporary)?;
        let temporary_database = Database::new(&temporary);
        for provider in temporary_database.list_providers()? {
            llm::migrate_legacy_secret(&provider.id)?;
        }
        write_marker(&marker, &format!("{}\n", time::shanghai_rfc3339()))?;
        std::fs::rename(&temporary, &current_database).map_err(|error| error.to_string())?;
        Ok::<(), String>(())
    })();
    if let Err(error) = migration_result {
        let _ = std::fs::remove_file(&temporary);
        if !current_database.exists() {
            let _ = std::fs::remove_file(&marker);
        }
        return Err(format!("legacy identity migration failed: {error}"));
    }
    Ok(LegacyMigration {
        legacy_dir,
        migrated: true,
    })
}

pub fn require_privacy(state: &AppState) -> Result<(), String> {
    let settings = state.db.settings()?;
    if settings.privacy_acknowledged_version.as_deref() != Some(PRIVACY_ACKNOWLEDGED_VERSION) {
        return Err("privacy_required: 请先阅读并同意隐私与使用说明".into());
    }
    Ok(())
}

pub fn prepare_database(db: &Database, data_dir: &Path) -> Result<(), String> {
    if db.path().exists() {
        if let Ok(version) = Database::validate_database(db.path()) {
            if version < CURRENT_SCHEMA_VERSION {
                create_automatic_backup_inner(db, data_dir, "pre-migration")?;
            }
        }
    }
    db.initialize()
}

#[tauri::command]
pub async fn get_app_info(app: AppHandle, state: State<'_, AppState>) -> Result<AppInfo, String> {
    let environment = sidecar::request(json!({"op":"environment_status","params":{}}))
        .await
        .unwrap_or_else(
            |error| json!({"protocolVersion":"unavailable","error": secrets::redact(&error)}),
        );
    let chrome = environment
        .get("chrome")
        .cloned()
        .and_then(|value| serde_json::from_value::<ChromeStatus>(value).ok())
        .unwrap_or_default();
    let sidecar_protocol = environment
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("unavailable")
        .to_string();
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        identifier: IDENTIFIER.into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        webview: tauri::webview_version().unwrap_or_else(|_| "unavailable".into()),
        schema_version: state.db.schema_version()?,
        sidecar_protocol,
        chrome,
        data_dir: state.data_dir.to_string_lossy().to_string(),
        legacy_data_detected: state
            .legacy_data_dir
            .as_ref()
            .is_some_and(|path| path.exists()),
        last_update_check_status: state
            .last_update_status
            .lock()
            .map_err(|_| "update status lock is poisoned".to_string())?
            .clone(),
    })
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, AppState>,
    manual: bool,
) -> Result<Option<AppUpdateInfo>, String> {
    require_privacy(&state)?;
    let mut settings = state.db.settings()?;
    if !automatic_update_check_allowed(&settings, manual) {
        return Ok(None);
    }
    if !manual && !update_check_due(settings.last_update_check_at.as_deref(), Utc::now()) {
        return Ok(None);
    }
    state
        .pending_update
        .lock()
        .map_err(|_| "update lock is poisoned".to_string())?
        .take();
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(error) => {
            let safe_error = secrets::redact(&error.to_string());
            set_update_status(&state, format!("error: {safe_error}"))?;
            return persist_update_check_outcome(
                &state.db,
                &mut settings,
                Err(format!("update_check_failed: {safe_error}")),
            );
        }
    };
    let update = match updater.check().await {
        Ok(update) => update,
        Err(error) => {
            set_update_status(
                &state,
                format!("error: {}", secrets::redact(&error.to_string())),
            )?;
            return persist_update_check_outcome(
                &state.db,
                &mut settings,
                Err(format!(
                    "update_check_failed: {}",
                    secrets::redact(&error.to_string())
                )),
            );
        }
    };
    if let Some(update) = update.as_ref() {
        if let Err(error) = validate_update_metadata(
            &update.current_version,
            &update.version,
            update.download_url.as_str(),
            &update.signature,
        ) {
            set_update_status(&state, "rejected invalid update metadata".into())?;
            return persist_update_check_outcome(&state.db, &mut settings, Err(error));
        }
    }
    let update = persist_update_check_outcome(&state.db, &mut settings, Ok(update))?;
    let Some(update) = update else {
        set_update_status(&state, "up-to-date".into())?;
        return Ok(None);
    };
    let download_size = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()
        .map(|client| (client, update.download_url.clone()))
        .map(|(client, url)| async move {
            client
                .head(url)
                .send()
                .await
                .ok()
                .and_then(|response| response.content_length())
        });
    let download_size = match download_size {
        Some(future) => future.await,
        None => None,
    };
    let info = AppUpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        published_at: update.date.map(|date| date.to_string()),
        notes: update.body.clone().unwrap_or_default(),
        download_size,
    };
    *state
        .pending_update
        .lock()
        .map_err(|_| "update lock is poisoned".to_string())? = Some(update);
    set_update_status(&state, format!("available: {}", info.version))?;
    Ok(Some(info))
}

#[tauri::command]
pub async fn download_and_install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<UpdateEvent>,
) -> Result<(), String> {
    require_privacy(&state)?;
    ensure_not_busy(&state)?;
    let update = state
        .pending_update
        .lock()
        .map_err(|_| "update lock is poisoned".to_string())?
        .clone()
        .ok_or_else(|| "update_not_checked: 请先检查更新".to_string())?;
    create_automatic_backup_inner(&state.db, &state.data_dir, "pre-update")?;
    let _ = on_event.send(UpdateEvent {
        event: "started".into(),
        downloaded: 0,
        total: None,
        message: None,
    });
    let downloaded = Arc::new(AtomicU64::new(0));
    let progress_downloaded = downloaded.clone();
    let progress_channel = on_event.clone();
    let finish_channel = on_event.clone();
    update
        .download_and_install(
            move |chunk, total| {
                let current =
                    progress_downloaded.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                let _ = progress_channel.send(UpdateEvent {
                    event: "progress".into(),
                    downloaded: current,
                    total,
                    message: None,
                });
            },
            move || {
                let _ = finish_channel.send(UpdateEvent {
                    event: "downloaded".into(),
                    downloaded: downloaded.load(Ordering::Relaxed),
                    total: None,
                    message: None,
                });
            },
        )
        .await
        .map_err(|error| {
            format!(
                "update_install_failed: {}",
                secrets::redact(&error.to_string())
            )
        })?;
    let _ = on_event.send(UpdateEvent {
        event: "finished".into(),
        downloaded: 0,
        total: None,
        message: Some("更新已安装，正在重启".into()),
    });
    app.restart()
}

#[tauri::command]
pub fn create_backup(
    state: State<'_, AppState>,
    output_path: String,
) -> Result<BackupInfo, String> {
    let mut path = PathBuf::from(output_path);
    if path.extension().and_then(|value| value.to_str()) != Some("aijobbackup") {
        path.set_extension("aijobbackup");
    }
    state.db.backup_to(&path)?;
    backup_info(&path)
}

#[tauri::command]
pub fn list_automatic_backups(state: State<'_, AppState>) -> Result<Vec<BackupInfo>, String> {
    list_automatic_backups_inner(&state.data_dir)
}

#[tauri::command]
pub fn restore_backup(state: State<'_, AppState>, backup_path: String) -> Result<(), String> {
    ensure_not_busy(&state)?;
    let source = PathBuf::from(backup_path);
    restore_backup_inner(&state.db, &state.data_dir, &source)
}

fn restore_backup_inner(db: &Database, data_dir: &Path, source: &Path) -> Result<(), String> {
    let version = Database::validate_database(source)?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "backup_schema_too_new: 备份 schema {version} 高于当前支持的 {CURRENT_SCHEMA_VERSION}"
        ));
    }
    let temporary = data_dir.join(format!(".restore-{}.tmp", Uuid::new_v4()));
    if let Err(error) = Database::copy_database(source, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    let result = (|| {
        Database::validate_database(&temporary)?;
        let current = db.path();
        let rollback = data_dir.join(".restore-rollback.tmp");
        let maintenance = db.begin_maintenance()?;
        if maintenance.has_active_tasks()? {
            return Err("busy: 当前有排队或运行中的任务，请在任务结束后重试".into());
        }
        maintenance.checkpoint()?;
        let directory = data_dir.join("backups").join("automatic");
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let timestamp = time::shanghai_now().format("%Y%m%dT%H%M%S");
        let backup = directory.join(format!(
            "pre-restore-v{}-{timestamp}.aijobbackup",
            env!("CARGO_PKG_VERSION")
        ));
        maintenance.backup_to(&backup)?;
        prune_automatic_backups(&directory)?;
        remove_database_side_files(current)?;
        let _ = std::fs::remove_file(&rollback);
        std::fs::rename(current, &rollback).map_err(|error| error.to_string())?;
        if let Err(error) = std::fs::rename(&temporary, current) {
            return match std::fs::rename(&rollback, current) {
                Ok(()) => Err(format!("cannot activate restored backup: {error}")),
                Err(rollback_error) => Err(format!(
                    "cannot activate restored backup ({error}); rollback failed: {rollback_error}"
                )),
            };
        }
        if let Err(error) = Database::validate_database(current) {
            let _ = std::fs::remove_file(current);
            std::fs::rename(&rollback, current).map_err(|rollback_error| {
                format!("restored backup is invalid ({error}); rollback failed: {rollback_error}")
            })?;
            return Err(format!("restored backup is invalid: {error}"));
        }
        let rollback_cleanup = std::fs::remove_file(&rollback).map_err(|error| error.to_string());
        let disable_result = maintenance.disable_until_restart();
        rollback_cleanup?;
        disable_result?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[tauri::command]
pub async fn clear_data(
    state: State<'_, AppState>,
    scope: String,
) -> Result<ClearDataResult, String> {
    ensure_not_busy(&state)?;
    if !matches!(
        scope.as_str(),
        "modelKeys" | "bossProfile" | "legacyData" | "all"
    ) {
        return Err("invalid_clear_scope".into());
    }
    let mut items = Vec::new();
    if matches!(scope.as_str(), "modelKeys" | "all") {
        clear_model_keys(&state, &mut items);
    }
    if matches!(scope.as_str(), "bossProfile" | "all") {
        clear_boss_profile(&state, &mut items).await;
    }
    if matches!(scope.as_str(), "legacyData" | "all") {
        clear_legacy_data(&state, &mut items);
    }
    if scope == "all" {
        clear_application_files(&state, &mut items);
    }
    Ok(ClearDataResult {
        complete: items.iter().all(|item| item.ok),
        items,
        restart_required: scope == "all",
    })
}

#[tauri::command]
pub fn export_diagnostics(
    app: AppHandle,
    state: State<'_, AppState>,
    output_path: String,
) -> Result<String, String> {
    maintain_logs(&state.data_dir)?;
    let mut output = PathBuf::from(output_path);
    if output.extension().and_then(|value| value.to_str()) != Some("zip") {
        output.set_extension("zip");
    }
    let file = File::create(&output).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let manifest = json!({
        "generatedAt": time::shanghai_rfc3339(),
        "appVersion": app.package_info().version.to_string(),
        "identifier": IDENTIFIER,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "schemaVersion": state.db.schema_version()?,
        "dataDirectory": "<app-data>",
        "containsPersonalData": false
    });
    zip.start_file("manifest.json", options)
        .map_err(|error| error.to_string())?;
    zip.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    )
    .map_err(|error| error.to_string())?;
    let log_dir = state.data_dir.join("logs");
    if log_dir.exists() {
        for entry in std::fs::read_dir(log_dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_file()
            {
                continue;
            }
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let safe_name = entry
                .file_name()
                .to_string_lossy()
                .replace(['/', '\\'], "_");
            zip.start_file(format!("logs/{safe_name}"), options)
                .map_err(|error| error.to_string())?;
            zip.write_all(sanitize_diagnostic_log(&text).as_bytes())
                .map_err(|error| error.to_string())?;
        }
    }
    zip.finish().map_err(|error| error.to_string())?;
    Ok(output.to_string_lossy().to_string())
}

#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart()
}

#[tauri::command]
pub fn exit_app(app: AppHandle) {
    app.exit(0);
}

pub fn maintain_logs(data_dir: &Path) -> Result<(), String> {
    let directory = data_dir.join("logs");
    if !directory.exists() {
        return Ok(());
    }
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(7 * 24 * 60 * 60))
        .unwrap_or(std::time::UNIX_EPOCH);
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if !metadata.is_file() {
            continue;
        }
        let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
        if modified < cutoff {
            let _ = std::fs::remove_file(entry.path());
        } else {
            files.push((entry.path(), modified, metadata.len()));
        }
    }
    files.sort_by_key(|(_, modified, _)| *modified);
    let mut total: u64 = files.iter().map(|(_, _, size)| *size).sum();
    for (path, _, size) in files {
        if total <= MAX_LOG_BYTES {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
    Ok(())
}

#[cfg(feature = "updater-e2e")]
pub fn run_updater_e2e(app: AppHandle, marker: PathBuf, expected_version: String) {
    let current_version = app.package_info().version.to_string();
    if current_version == expected_version {
        let previous = std::fs::read(&marker)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or_default();
        let progress_events = previous
            .get("progressEvents")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let downloaded_bytes = previous
            .get("downloadedBytes")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let ok = previous.get("stage").and_then(Value::as_str) == Some("installed")
            && progress_events > 0
            && downloaded_bytes > 0;
        let result = json!({
            "ok": ok,
            "stage": "restarted",
            "version": current_version,
            "progressEvents": progress_events,
            "downloadedBytes": downloaded_bytes
        });
        let _ = write_updater_e2e_marker(&marker, &result);
        app.exit(if ok { 0 } else { 1 });
        return;
    }

    let _ = write_updater_e2e_marker(
        &marker,
        &json!({
            "ok": false,
            "stage": "starting",
            "fromVersion": current_version,
            "expectedVersion": expected_version
        }),
    );

    tauri::async_runtime::spawn(async move {
        let result = async {
            let update = app
                .updater()
                .map_err(|error| error.to_string())?
                .check()
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "test updater did not return an update".to_string())?;
            validate_update_metadata(
                &update.current_version,
                &update.version,
                update.download_url.as_str(),
                &update.signature,
            )?;
            if update.version != expected_version {
                return Err(format!(
                    "test updater returned {}, expected {expected_version}",
                    update.version
                ));
            }
            let progress_events = Arc::new(AtomicU64::new(0));
            let downloaded_bytes = Arc::new(AtomicU64::new(0));
            let progress_events_callback = progress_events.clone();
            let downloaded_bytes_callback = downloaded_bytes.clone();
            update
                .download_and_install(
                    move |chunk, _| {
                        progress_events_callback.fetch_add(1, Ordering::Relaxed);
                        downloaded_bytes_callback.fetch_add(chunk as u64, Ordering::Relaxed);
                    },
                    || {},
                )
                .await
                .map_err(|error| error.to_string())?;
            write_updater_e2e_marker(
                &marker,
                &json!({
                    "ok": false,
                    "stage": "installed",
                    "fromVersion": current_version,
                    "expectedVersion": expected_version,
                    "progressEvents": progress_events.load(Ordering::Relaxed),
                    "downloadedBytes": downloaded_bytes.load(Ordering::Relaxed)
                }),
            )?;
            Ok::<(), String>(())
        }
        .await;
        match result {
            Ok(()) => app.restart(),
            Err(error) => {
                let _ = write_updater_e2e_marker(
                    &marker,
                    &json!({
                        "ok": false,
                        "stage": "failed",
                        "error": secrets::redact(&error)
                    }),
                );
                app.exit(1);
            }
        }
    });
}

#[cfg(feature = "updater-e2e")]
fn write_updater_e2e_marker(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn sanitize_diagnostic_log(value: &str) -> String {
    static SENSITIVE_LINE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static USER_PATH: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let sensitive_line = SENSITIVE_LINE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(api[_ -]?key|authorization|cookie|prompt|resume|job[_ -]?(?:body|description|content|payload)|简历|岗位正文|职位正文|zp_stoken)",
        )
        .expect("valid diagnostic sensitive-line regex")
    });
    let user_path = USER_PATH.get_or_init(|| {
        regex::Regex::new(r"(?i)(?:[a-z]:\\Users\\|/Users/|/home/)[^\\/\s]+")
            .expect("valid user-path regex")
    });

    value
        .lines()
        .map(|line| {
            if sensitive_line.is_match(line) {
                "[SENSITIVE LOG LINE OMITTED]".to_string()
            } else {
                let redacted = secrets::redact(line);
                user_path.replace_all(&redacted, "<user-home>").into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn update_check_due(last_check: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(last_check) = last_check else {
        return true;
    };
    DateTime::parse_from_rfc3339(last_check)
        .map(|value| {
            now.signed_duration_since(value.with_timezone(&Utc)) >= ChronoDuration::hours(24)
        })
        .unwrap_or(true)
}

fn automatic_update_check_allowed(settings: &AppSettings, manual: bool) -> bool {
    manual || settings.automatic_update_checks
}

fn validate_update_metadata(
    current_version: &str,
    candidate_version: &str,
    download_url: &str,
    signature: &str,
) -> Result<(), String> {
    let current = semver::Version::parse(current_version)
        .map_err(|_| "invalid_update_metadata: 当前应用版本不是有效的 semver".to_string())?;
    let candidate = semver::Version::parse(candidate_version)
        .map_err(|_| "invalid_update_metadata: 更新版本不是有效的 semver".to_string())?;
    if candidate <= current {
        return Err("invalid_update_metadata: 更新版本必须高于当前版本".into());
    }
    if !download_url.starts_with("https://") || signature.trim().is_empty() {
        return Err("invalid_update_metadata: 更新元数据必须使用 HTTPS 并包含签名".into());
    }
    Ok(())
}

fn set_update_status(state: &AppState, status: String) -> Result<(), String> {
    *state
        .last_update_status
        .lock()
        .map_err(|_| "update status lock is poisoned".to_string())? = Some(status);
    Ok(())
}

fn persist_update_check_outcome<T>(
    db: &Database,
    settings: &mut AppSettings,
    outcome: Result<T, String>,
) -> Result<T, String> {
    let value = outcome?;
    settings.last_update_check_at = Some(time::shanghai_rfc3339());
    db.save_settings(settings)?;
    Ok(value)
}

fn ensure_not_busy(state: &AppState) -> Result<(), String> {
    if state.db.has_active_tasks()? {
        return Err("busy: 当前有排队或运行中的任务，请在任务结束后重试".into());
    }
    Ok(())
}

fn create_automatic_backup_inner(
    db: &Database,
    data_dir: &Path,
    reason: &str,
) -> Result<BackupInfo, String> {
    let directory = data_dir.join("backups").join("automatic");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = time::shanghai_now().format("%Y%m%dT%H%M%S");
    let file = directory.join(format!(
        "{reason}-v{}-{timestamp}.aijobbackup",
        env!("CARGO_PKG_VERSION")
    ));
    db.backup_to(&file)?;
    prune_automatic_backups(&directory)?;
    backup_info(&file)
}

fn list_automatic_backups_inner(data_dir: &Path) -> Result<Vec<BackupInfo>, String> {
    let directory = data_dir.join("backups").join("automatic");
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut values = std::fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().and_then(|value| value.to_str()) == Some("aijobbackup")
        })
        .filter_map(|entry| backup_info(&entry.path()).ok())
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(values)
}

fn prune_automatic_backups(directory: &Path) -> Result<(), String> {
    let mut entries = std::fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((entry.path(), modified))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    for (path, _) in entries.into_iter().skip(3) {
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn backup_info(path: &Path) -> Result<BackupInfo, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    let created_at = metadata
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    Ok(BackupInfo {
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("backup.aijobbackup")
            .to_string(),
        path: path.to_string_lossy().to_string(),
        size: metadata.len(),
        created_at,
        source_version: env!("CARGO_PKG_VERSION").into(),
    })
}

fn clear_model_keys(state: &AppState, items: &mut Vec<ClearDataItemResult>) {
    let result = (|| {
        for provider_id in provider_ids_for_key_cleanup(state)? {
            llm::delete_secret(&provider_id)?;
            llm::delete_legacy_secret(&provider_id)?;
        }
        state.db.clear_provider_secret_references()
    })();
    push_result(items, "modelKeys", result, "模型密钥和验证状态已清除");
}

async fn clear_boss_profile(state: &AppState, items: &mut Vec<ClearDataItemResult>) {
    let result = async {
        sidecar::request(json!({"op":"close_boss","params":{}})).await?;
        sidecar::request(json!({"op":"clear_boss_data","params":{}})).await?;
        state
            .db
            .save_boss_profile_state(&BossProfileState::default())?;
        Ok::<(), String>(())
    }
    .await;
    push_result(items, "bossProfile", result, "BOSS 登录数据已清除");
}

fn clear_legacy_data(state: &AppState, items: &mut Vec<ClearDataItemResult>) {
    let result = (|| {
        if let Some(path) = state.legacy_data_dir.as_ref().filter(|path| path.exists()) {
            for provider_id in provider_ids_from_database(&path.join(DATABASE_NAME))? {
                llm::delete_legacy_secret(&provider_id)?;
            }
            std::fs::remove_dir_all(path).map_err(|error| error.to_string())?;
        }
        Ok::<(), String>(())
    })();
    push_result(items, "legacyData", result, "旧版标识数据已清除");
}

fn provider_ids_for_key_cleanup(state: &AppState) -> Result<HashSet<String>, String> {
    let mut ids = state
        .db
        .list_providers()?
        .into_iter()
        .map(|provider| provider.id)
        .collect::<HashSet<_>>();
    if let Some(path) = state.legacy_data_dir.as_ref().filter(|path| path.exists()) {
        ids.extend(provider_ids_from_database(&path.join(DATABASE_NAME))?);
    }
    Ok(ids)
}

fn provider_ids_from_database(path: &Path) -> Result<HashSet<String>, String> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    Ok(Database::new(path)
        .list_providers()?
        .into_iter()
        .map(|provider| provider.id)
        .collect())
}

fn clear_application_files(state: &AppState, items: &mut Vec<ClearDataItemResult>) {
    let result = (|| {
        let maintenance = state.db.begin_maintenance()?;
        if maintenance.has_active_tasks()? {
            return Err("busy: 当前有排队或运行中的任务，请在任务结束后重试".into());
        }
        maintenance.checkpoint()?;
        remove_database_side_files(state.db.path())?;
        if state.db.path().exists() {
            std::fs::remove_file(state.db.path()).map_err(|error| error.to_string())?;
        }
        let cleanup_result = (|| {
            for name in ["backups", "logs", "imports", "result"] {
                let path = state.data_dir.join(name);
                if path.exists() {
                    std::fs::remove_dir_all(path).map_err(|error| error.to_string())?;
                }
            }
            for entry in std::fs::read_dir(&state.data_dir).map_err(|error| error.to_string())? {
                let entry = entry.map_err(|error| error.to_string())?;
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".resume-import-")
                {
                    if entry
                        .file_type()
                        .map_err(|error| error.to_string())?
                        .is_dir()
                    {
                        std::fs::remove_dir_all(entry.path()).map_err(|error| error.to_string())?;
                    } else {
                        std::fs::remove_file(entry.path()).map_err(|error| error.to_string())?;
                    }
                }
            }
            Ok::<(), String>(())
        })();
        let disable_result = maintenance.disable_until_restart();
        cleanup_result?;
        disable_result?;
        Ok::<(), String>(())
    })();
    push_result(
        items,
        "applicationData",
        result,
        "数据库、备份、日志和临时文件已清除",
    );
}

fn remove_database_side_files(database: &Path) -> Result<(), String> {
    for suffix in ["-wal", "-shm"] {
        let side_file = PathBuf::from(format!("{}{suffix}", database.to_string_lossy()));
        if side_file.exists() {
            std::fs::remove_file(side_file).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn push_result(
    items: &mut Vec<ClearDataItemResult>,
    item: &str,
    result: Result<(), String>,
    success_message: &str,
) {
    match result {
        Ok(()) => items.push(ClearDataItemResult {
            item: item.into(),
            ok: true,
            message: success_message.into(),
        }),
        Err(error) => items.push(ClearDataItemResult {
            item: item.into(),
            ok: false,
            message: secrets::redact(&error),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskRun;
    use tempfile::tempdir;

    #[test]
    fn update_interval_is_twenty_four_hours() {
        let now = Utc::now();
        assert!(update_check_due(None, now));
        assert!(!update_check_due(
            Some(&(now - ChronoDuration::hours(23)).to_rfc3339()),
            now
        ));
        assert!(update_check_due(
            Some(&(now - ChronoDuration::hours(24)).to_rfc3339()),
            now
        ));
    }

    #[test]
    fn disabled_automatic_checks_still_allow_manual_checks() {
        let settings = AppSettings {
            automatic_update_checks: false,
            ..AppSettings::default()
        };
        assert!(!automatic_update_check_allowed(&settings, false));
        assert!(automatic_update_check_allowed(&settings, true));
    }

    #[test]
    fn failed_update_checks_do_not_advance_the_successful_check_time() {
        let directory = tempdir().unwrap();
        let db = Database::new(directory.path().join(DATABASE_NAME));
        db.initialize().unwrap();
        let mut settings = db.settings().unwrap();

        assert!(
            persist_update_check_outcome::<()>(&db, &mut settings, Err("offline".into())).is_err()
        );
        assert!(db.settings().unwrap().last_update_check_at.is_none());

        persist_update_check_outcome(&db, &mut settings, Ok(())).unwrap();
        assert!(db.settings().unwrap().last_update_check_at.is_some());
    }

    #[test]
    fn update_metadata_requires_a_newer_semver_https_and_signature() {
        assert!(validate_update_metadata(
            "0.2.0",
            "0.2.1",
            "https://example.test/update",
            "signature"
        )
        .is_ok());
        for invalid in [
            ("0.2.0", "0.2.0", "https://example.test/update", "signature"),
            ("0.2.0", "0.1.9", "https://example.test/update", "signature"),
            ("0.2.0", "0.2.1", "http://example.test/update", "signature"),
            ("0.2.0", "0.2.1", "https://example.test/update", ""),
            (
                "invalid",
                "0.2.1",
                "https://example.test/update",
                "signature",
            ),
        ] {
            assert!(validate_update_metadata(invalid.0, invalid.1, invalid.2, invalid.3).is_err());
        }
    }

    #[test]
    fn diagnostic_logs_omit_content_secrets_and_user_paths() {
        let input = concat!(
            "safe event at C:\\Users\\alice\\AppData\\Local\\app\n",
            "prompt: full resume content\n",
            "Cookie: zp_stoken=private\n",
            "token=github_pat_1234567890\n"
        );
        let output = sanitize_diagnostic_log(input);
        assert!(output.contains("safe event at <user-home>"));
        assert!(output.contains("[SENSITIVE LOG LINE OMITTED]"));
        assert!(output.contains("[REDACTED]"));
        for forbidden in ["alice", "full resume content", "private", "github_pat_"] {
            assert!(!output.contains(forbidden), "{output}");
        }
    }

    #[test]
    fn backup_integrity_and_retention_are_enforced() {
        let directory = tempdir().unwrap();
        let db = Database::new(directory.path().join(DATABASE_NAME));
        db.initialize().unwrap();
        for index in 0..5 {
            let backup_dir = directory.path().join("backups").join("automatic");
            std::fs::create_dir_all(&backup_dir).unwrap();
            let path = backup_dir.join(format!("{index}.aijobbackup"));
            db.backup_to(&path).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let backup_dir = directory.path().join("backups").join("automatic");
        prune_automatic_backups(&backup_dir).unwrap();
        assert_eq!(std::fs::read_dir(backup_dir).unwrap().count(), 3);
    }

    #[test]
    fn corrupted_and_future_backups_are_rejected() {
        let directory = tempdir().unwrap();
        let corrupted = directory.path().join("corrupted.aijobbackup");
        std::fs::write(&corrupted, b"not sqlite").unwrap();
        assert!(Database::validate_database(&corrupted).is_err());

        let db = Database::new(directory.path().join(DATABASE_NAME));
        db.initialize().unwrap();
        db.connect()
            .unwrap()
            .execute(
                "INSERT INTO schema_migrations(version,applied_at) VALUES (99,datetime('now'))",
                [],
            )
            .unwrap();
        assert!(Database::validate_database(db.path()).unwrap() > CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn busy_guard_blocks_destructive_commands() {
        let directory = tempdir().unwrap();
        let db = Database::new(directory.path().join(DATABASE_NAME));
        db.initialize().unwrap();
        let task = TaskRun {
            id: "busy-task".into(),
            kind: "fit".into(),
            title: "busy".into(),
            state: "running".into(),
            progress: 50,
            message: "running".into(),
            recoverable_error: None,
            created_at: time::shanghai_rfc3339(),
            updated_at: time::shanghai_rfc3339(),
            logs: vec![],
        };
        db.save_task(&task).unwrap();
        let state = AppState {
            db,
            data_dir: directory.path().to_path_buf(),
            legacy_data_dir: None,
            pending_update: std::sync::Mutex::new(None),
            last_update_status: std::sync::Mutex::new(None),
        };
        assert!(ensure_not_busy(&state).unwrap_err().starts_with("busy:"));
    }

    #[test]
    fn clearing_application_files_removes_the_database_and_blocks_recreation() {
        let directory = tempdir().unwrap();
        let db = Database::new(directory.path().join(DATABASE_NAME));
        db.initialize().unwrap();
        let state = AppState {
            db: db.clone(),
            data_dir: directory.path().to_path_buf(),
            legacy_data_dir: None,
            pending_update: std::sync::Mutex::new(None),
            last_update_status: std::sync::Mutex::new(None),
        };
        let mut items = Vec::new();

        clear_application_files(&state, &mut items);

        assert_eq!(items.len(), 1);
        assert!(items[0].ok, "{}", items[0].message);
        assert!(!db.path().exists());
        assert!(db
            .connect()
            .err()
            .is_some_and(|error| error.starts_with("database_unavailable:")));
        assert!(!db.path().exists());
    }

    #[test]
    fn identity_migration_never_overwrites_an_existing_new_database() {
        let root = tempdir().unwrap();
        let current = root.path().join(IDENTIFIER);
        let legacy = root.path().join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(current.join(DATABASE_NAME), b"current").unwrap();
        std::fs::write(legacy.join(DATABASE_NAME), b"legacy").unwrap();
        let result = migrate_legacy_identity(&current).unwrap();
        assert!(!result.migrated);
        assert_eq!(
            std::fs::read(current.join(DATABASE_NAME)).unwrap(),
            b"current"
        );
    }

    #[test]
    fn identity_migration_does_not_activate_a_database_when_marker_write_fails() {
        let root = tempdir().unwrap();
        let current = root.path().join(IDENTIFIER);
        let legacy = root.path().join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(&legacy).unwrap();
        let legacy_database = legacy.join(DATABASE_NAME);
        let connection = rusqlite::Connection::open(&legacy_database).unwrap();
        connection
            .execute_batch(&format!(
                "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);\
                 INSERT INTO schema_migrations(version, applied_at) VALUES ({CURRENT_SCHEMA_VERSION}, datetime('now'));\
                 CREATE TABLE ai_providers(id TEXT PRIMARY KEY, payload_json TEXT NOT NULL);"
            ))
            .unwrap();
        drop(connection);

        let error =
            migrate_legacy_identity_with(&current, |_, _| Err("marker denied".into())).unwrap_err();

        assert!(error.contains("marker denied"));
        assert!(legacy_database.exists());
        assert!(!current.join(DATABASE_NAME).exists());
        assert!(!current.join(".legacy-migrated-v0.2.0").exists());

        let retried = migrate_legacy_identity(&current).unwrap();
        assert!(retried.migrated);
        assert_eq!(
            Database::validate_database(&current.join(DATABASE_NAME)).unwrap(),
            CURRENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn v0_1_7_fixture_migrates_without_removing_the_legacy_copy() {
        let root = tempdir().unwrap();
        let current = root.path().join(IDENTIFIER);
        let legacy = root.path().join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(&legacy).unwrap();
        let legacy_database = legacy.join(DATABASE_NAME);
        let connection = rusqlite::Connection::open(&legacy_database).unwrap();
        connection
            .execute_batch(include_str!("../tests/fixtures/v0.1.7.sql"))
            .unwrap();
        drop(connection);

        let result = migrate_legacy_identity(&current).unwrap();
        assert!(result.migrated);
        assert!(legacy_database.exists());
        let migrated = Database::new(current.join(DATABASE_NAME));
        prepare_database(&migrated, &current).unwrap();
        assert_eq!(migrated.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        let settings = migrated.settings().unwrap();
        assert!(settings.privacy_acknowledged_version.is_none());
        assert!(current.join(".legacy-migrated-v0.2.0").exists());
    }

    #[test]
    fn all_pending_schema_migrations_roll_back_together() {
        let directory = tempdir().unwrap();
        let db = Database::new(directory.path().join(DATABASE_NAME));
        db.initialize().unwrap();
        let connection = db.connect().unwrap();
        connection
            .execute(
                "INSERT INTO jobs(id,source,external_key,fingerprint,title,company,location,first_seen,last_seen,payload_json) VALUES ('bad','test','bad','bad','bad','bad','bad','now','now','invalid-json')",
                [],
            )
            .unwrap();
        connection
            .execute("DELETE FROM schema_migrations WHERE version IN (4,5)", [])
            .unwrap();
        drop(connection);
        assert!(db.initialize().is_err());
        assert_eq!(db.schema_version().unwrap(), 3);
    }

    #[test]
    fn restore_uses_an_exclusive_checkpoint_and_requires_restart() {
        let directory = tempdir().unwrap();
        let current = Database::new(directory.path().join(DATABASE_NAME));
        current.initialize().unwrap();
        let source_path = directory.path().join("selected.aijobbackup");
        let source = Database::new(&source_path);
        source.initialize().unwrap();
        let mut settings = source.settings().unwrap();
        settings.advanced_mode = true;
        source.save_settings(&settings).unwrap();

        restore_backup_inner(&current, directory.path(), &source_path).unwrap();

        assert!(current
            .connect()
            .err()
            .is_some_and(|error| error.starts_with("database_unavailable:")));
        let restored = Database::new(current.path());
        assert!(restored.settings().unwrap().advanced_mode);
        let backups = list_automatic_backups_inner(directory.path()).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            Database::validate_database(Path::new(&backups[0].path)).unwrap(),
            CURRENT_SCHEMA_VERSION
        );
    }
}
