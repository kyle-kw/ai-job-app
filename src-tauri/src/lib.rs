mod analytics;
mod commands;
mod db;
mod llm;
mod models;
mod scoring;
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
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Database::new(data_dir.join("ai-job-app.db"));
            db.initialize().map_err(std::io::Error::other)?;
            app.manage(AppState { db, data_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::get_job_data_report,
            commands::export_job_data_report,
            commands::start_scrape,
            commands::setup_boss,
            commands::import_resume,
            commands::save_resume,
            commands::save_preferences,
            commands::analyze_job,
            commands::generate_greeting,
            commands::propose_tailoring,
            commands::update_resume_patch,
            commands::render_resume,
            commands::save_provider,
            commands::test_provider,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AI Job App");
}
