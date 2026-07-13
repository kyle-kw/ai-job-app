mod analytics;
mod assistant;
mod commands;
mod db;
mod llm;
mod models;
mod providers;
mod scoring;
mod secrets;
mod sidecar;
mod skills;
mod time;

use db::Database;
use std::path::PathBuf;
use tauri::Manager;

pub struct AppState {
    pub db: Database,
    pub data_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let imports = data_dir.join("imports");
            if imports.exists() {
                if let Err(error) = std::fs::remove_dir_all(&imports) {
                    eprintln!("failed to clean stale resume imports: {error}");
                }
            }
            let db = Database::new(data_dir.join("ai-job-app.db"));
            db.initialize().map_err(std::io::Error::other)?;
            let _ = llm::delete_secret("provider-openrouter");
            app.manage(AppState { db, data_dir });
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
            commands::save_preferences,
            assistant::analyze_job,
            assistant::start_fit_batch,
            assistant::start_fit_batch_for_query,
            assistant::open_job_source,
            assistant::get_interview_preparation_state,
            assistant::generate_interview_preparation,
            assistant::propose_resume_chat_edits,
            assistant::apply_resume_chat_edits,
            assistant::list_resume_versions,
            assistant::get_resume_version,
            assistant::restore_resume_version,
            commands::generate_greeting,
            commands::render_resume,
            providers::save_provider,
            providers::test_provider,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AI Job App");
}
