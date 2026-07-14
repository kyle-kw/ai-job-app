use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

type EventHandler = Box<dyn FnMut(Value) -> Result<(), String> + Send>;

pub async fn request(payload: Value) -> Result<Value, String> {
    request_with_handler(payload, None).await
}

pub async fn request_with_events<F>(payload: Value, on_event: F) -> Result<Value, String>
where
    F: FnMut(Value) -> Result<(), String> + Send + 'static,
{
    request_with_handler(payload, Some(Box::new(on_event))).await
}

async fn request_with_handler(
    payload: Value,
    handler: Option<EventHandler>,
) -> Result<Value, String> {
    let timeout = operation_timeout(&payload);
    let boss_operation = matches!(
        payload.get("op").and_then(Value::as_str),
        Some("setup_boss" | "scrape_jobs")
    );
    match tokio::time::timeout(timeout, run_child(payload, handler)).await {
        Ok(result) => result,
        Err(_) => {
            if boss_operation {
                let _ = tokio::time::timeout(
                    Duration::from_secs(30),
                    run_child(json!({"op":"close_boss","params":{}}), None),
                )
                .await;
            }
            Err(format!(
                "Python sidecar timed out after {} seconds and was terminated.",
                timeout.as_secs()
            ))
        }
    }
}

async fn run_child(payload: Value, mut on_event: Option<EventHandler>) -> Result<Value, String> {
    let mut command = sidecar_command()?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    command
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("TZ", "Asia/Shanghai")
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Unable to start Python sidecar: {error}"))?;

    let serialized = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&serialized)
            .await
            .map_err(|error| format!("Unable to write sidecar request: {error}"))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|error| format!("Unable to finish sidecar request: {error}"))?;
    }

    let stdout = child.stdout.take().ok_or("Unable to read sidecar stdout")?;
    let mut stderr = child.stderr.take().ok_or("Unable to read sidecar stderr")?;
    let stderr_task = tokio::spawn(async move {
        let mut output = String::new();
        let _ = stderr.read_to_string(&mut output).await;
        output
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut result = None;
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|error| format!("Unable to read sidecar output: {error}"))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .map_err(|error| format!("Sidecar returned invalid JSON: {error}\n{line}"))?;
        if value.get("type").and_then(Value::as_str) == Some("result") {
            result = Some(value);
        } else if let Some(handler) = on_event.as_mut() {
            if let Err(error) = handler(value) {
                terminate(&mut child).await;
                let _ = stderr_task.await;
                return Err(error);
            }
        }
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("Sidecar wait failed: {error}"))?;
    let stderr = stderr_task.await.unwrap_or_default();
    let result = result.ok_or_else(|| {
        format!(
            "Sidecar returned no result (exit={status}): {}",
            crate::secrets::redact(&stderr)
        )
    })?;
    if result.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        Ok(result.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(result
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Sidecar operation failed")
            .to_string())
    }
}

async fn terminate(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn operation_timeout(payload: &Value) -> Duration {
    let operation = payload
        .get("op")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = payload.get("params").unwrap_or(&Value::Null);
    let seconds = match operation {
        "ping" => 15,
        "close_boss" => 30,
        "extract_resume" | "render_resume" => 5 * 60,
        "setup_boss" => {
            params
                .get("loginTimeout")
                .and_then(Value::as_u64)
                .unwrap_or(300)
                + 60
        }
        "scrape_jobs" => {
            let login = params
                .get("loginTimeout")
                .and_then(Value::as_u64)
                .unwrap_or(300);
            let pages = params
                .get("pages")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 5);
            login + pages * 20 * 60 + 2 * 60
        }
        _ => 5 * 60,
    };
    Duration::from_secs(seconds)
}

fn sidecar_command() -> Result<Command, String> {
    if let Ok(explicit) = std::env::var("AI_JOB_SIDECAR") {
        return Ok(Command::new(explicit));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let worker = manifest.join("..").join("sidecar").join("worker.py");
    if cfg!(debug_assertions) && worker.exists() {
        return Ok(source_sidecar_command(&manifest, &worker));
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let parent = executable.parent().unwrap_or_else(|| Path::new("."));
    let binary_name = if cfg!(windows) {
        "job-assistant-sidecar.exe"
    } else {
        "job-assistant-sidecar"
    };
    let packaged = parent.join(binary_name);
    if packaged.exists() {
        return Ok(Command::new(packaged));
    }
    if worker.exists() {
        return Ok(source_sidecar_command(&manifest, &worker));
    }
    Err("job-assistant-sidecar was not found.".into())
}

fn source_sidecar_command(manifest: &Path, worker: &Path) -> Command {
    let workspace = manifest.join("..");
    let local_python = if cfg!(windows) {
        workspace
            .join("sidecar")
            .join(".venv")
            .join("Scripts")
            .join("python.exe")
    } else {
        workspace
            .join("sidecar")
            .join(".venv")
            .join("bin")
            .join("python")
    };
    let python = std::env::var("AI_JOB_PYTHON").unwrap_or_else(|_| {
        if local_python.exists() {
            local_python.to_string_lossy().to_string()
        } else if cfg!(windows) {
            "python".to_string()
        } else {
            "python3".to_string()
        }
    });
    let mut command = Command::new(python);
    command.arg(worker);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_bounded_operation_deadlines() {
        assert_eq!(
            operation_timeout(&json!({"op":"ping"})),
            Duration::from_secs(15)
        );
        assert_eq!(
            operation_timeout(&json!({"op":"scrape_jobs","params":{"pages":5,"loginTimeout":120}})),
            Duration::from_secs(6240)
        );
    }
}
