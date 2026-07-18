use super::*;

#[tauri::command]
pub fn open_job_source(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    distribution::require_privacy(&state)?;
    let job = state
        .db
        .get_job(&job_id)?
        .ok_or_else(|| "岗位不存在。".to_string())?;
    let url =
        reqwest::Url::parse(job.source_url.trim()).map_err(|_| "原岗位链接不可用。".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("仅允许打开 http(s) 岗位链接。".into());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host != "zhipin.com" && !host.ends_with(".zhipin.com") {
        return Err("岗位链接不是受信任的 BOSS 域名。".into());
    }
    let _ = app;
    open_system_url(url.as_str())
}

#[tauri::command]
pub fn open_github_issues() -> Result<(), String> {
    open_system_url("https://github.com/kyle-kw/ai-job-app/issues")
}

#[cfg(target_os = "windows")]
fn open_system_url(url: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开系统默认浏览器：{error}"))
}

#[cfg(target_os = "macos")]
fn open_system_url(url: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开系统默认浏览器：{error}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_system_url(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开系统默认浏览器：{error}"))
}
