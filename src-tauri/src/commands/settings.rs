use super::*;

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    state.db.save_settings(&settings)?;
    Ok(settings)
}
