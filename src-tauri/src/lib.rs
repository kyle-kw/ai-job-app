mod analytics;
mod assistant;
mod commands;
mod db;
mod distribution;
mod llm;
mod models;
mod provider_policy;
mod providers;
mod scoring;
mod secrets;
mod sidecar;
mod skills;
mod time;

use db::Database;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_updater::Update;

pub struct AppState {
    pub db: Database,
    pub data_dir: PathBuf,
    pub legacy_data_dirs: Vec<PathBuf>,
    pub pending_update: Mutex<Option<Update>>,
}

fn show_startup_error(app: &tauri::App, error: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let handle = app.handle().clone();
    app.dialog()
        .message(format!(
            "求职舱无法安全启动。原数据未被删除，也不会创建空数据库掩盖问题。\n\n{error}"
        ))
        .title("求职舱启动失败")
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.exit(1));
}

#[cfg(feature = "updater-e2e")]
fn updater_e2e_args() -> Option<(PathBuf, String)> {
    let mut marker = None;
    let mut expected = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--updater-e2e-result" {
            marker = args.next().map(PathBuf::from);
        } else if argument == "--updater-e2e-expected" {
            expected = args.next().and_then(|value| value.into_string().ok());
        }
    }
    marker.map(|path| (path, expected.unwrap_or_else(|| "0.2.1".into())))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            #[cfg(feature = "updater-e2e")]
            if let Some((marker, expected)) = updater_e2e_args() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                distribution::run_updater_e2e(app.handle().clone(), marker, expected);
                return Ok(());
            }
            let startup = (|| {
                let data_dir = app
                    .path()
                    .app_data_dir()
                    .map_err(|error| error.to_string())?;
                std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
                let legacy = distribution::migrate_legacy_identity(&data_dir)?;
                let db = Database::new(data_dir.join("ai-job-app.db"));
                distribution::prepare_database(&db, &data_dir)?;
                distribution::maintain_logs(&data_dir)?;
                Ok::<_, String>((data_dir, legacy, db))
            })();
            let (data_dir, legacy, db) = match startup {
                Ok(value) => value,
                Err(error) => {
                    show_startup_error(app, &error);
                    return Ok(());
                }
            };
            if legacy.migrated {
                eprintln!("migrated application data to the current identifier");
            }
            let imports = data_dir.join("imports");
            if imports.exists() {
                if let Err(error) = std::fs::remove_dir_all(&imports) {
                    eprintln!("failed to clean stale resume imports: {error}");
                }
            }
            let _ = llm::delete_secret("provider-openrouter");
            let _ = llm::delete_legacy_secret("provider-openrouter");
            let smoke_marker = std::env::var_os("AI_JOB_APP_SMOKE_RESULT").map(PathBuf::from);
            let smoke_result = if smoke_marker.is_some() {
                let sidecar = tauri::async_runtime::block_on(sidecar::request(serde_json::json!({
                    "op": "ping",
                    "params": {}
                })));
                Some(serde_json::json!({
                    "ok": sidecar.is_ok(),
                    "schemaVersion": db.schema_version().unwrap_or_default(),
                    "sidecar": sidecar.unwrap_or_else(|error| serde_json::json!({"error": error}))
                }))
            } else {
                None
            };
            app.manage(AppState {
                db,
                data_dir,
                legacy_data_dirs: legacy.legacy_dirs,
                pending_update: Mutex::new(None),
            });
            if let (Some(marker), Some(result)) = (smoke_marker, smoke_result) {
                std::fs::write(
                    marker,
                    serde_json::to_vec_pretty(&result).map_err(std::io::Error::other)?,
                )?;
                if result.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
                    return Err(std::io::Error::other("desktop smoke check failed").into());
                }
                app.handle().exit(0);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::list_jobs_page,
            commands::list_job_options,
            commands::list_job_cities,
            commands::get_job,
            commands::delete_job,
            commands::delete_missing_description_jobs,
            commands::list_report_keywords,
            commands::get_job_data_report,
            commands::export_jobs_json,
            commands::export_job_data_report,
            commands::start_scrape,
            commands::start_job_detail_extraction,
            commands::setup_boss,
            commands::import_resume,
            commands::create_resume_from_template,
            commands::save_resume,
            commands::list_resume_variants,
            commands::get_resume_variant,
            commands::create_resume_variant,
            commands::save_resume_variant,
            commands::delete_resume_variant,
            commands::preview_resume_variant_rebase,
            commands::apply_resume_variant_rebase,
            commands::restore_resume_variant_version,
            commands::save_preferences,
            assistant::analyze_job,
            assistant::start_fit_batch,
            assistant::start_fit_batch_for_query,
            assistant::open_job_source,
            assistant::open_github_issues,
            assistant::get_interview_preparation_state,
            assistant::generate_interview_preparation,
            assistant::get_report_competitiveness_state,
            assistant::generate_report_competitiveness,
            assistant::propose_resume_chat_edits,
            assistant::apply_resume_chat_edits,
            assistant::analyze_resume_coverage,
            assistant::list_resume_versions,
            assistant::get_resume_version,
            assistant::restore_resume_version,
            commands::generate_greeting,
            commands::render_resume,
            providers::save_provider,
            providers::test_provider,
            commands::save_settings,
            distribution::get_app_info,
            distribution::check_for_update,
            distribution::download_and_install_update,
            distribution::create_backup,
            distribution::restore_backup,
            distribution::list_automatic_backups,
            distribution::clear_data,
            distribution::export_diagnostics,
            distribution::restart_app,
            distribution::exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AI Job App");
}
