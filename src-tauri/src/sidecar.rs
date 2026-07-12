use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub async fn request(payload: Value) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || request_blocking(payload, None))
        .await
        .map_err(|error| format!("sidecar task failed: {error}"))?
}

pub async fn request_with_events<F>(payload: Value, mut on_event: F) -> Result<Value, String>
where
    F: FnMut(Value) -> Result<(), String> + Send + 'static,
{
    tokio::task::spawn_blocking(move || request_blocking(payload, Some(&mut on_event)))
        .await
        .map_err(|error| format!("sidecar task failed: {error}"))?
}

fn request_blocking(
    payload: Value,
    mut on_event: Option<&mut dyn FnMut(Value) -> Result<(), String>>,
) -> Result<Value, String> {
    let mut command = sidecar_command()?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("TZ", "Asia/Shanghai");
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法启动 Python sidecar：{error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        writeln!(
            stdin,
            "{}",
            serde_json::to_string(&payload).map_err(|error| error.to_string())?
        )
        .map_err(|error| format!("无法写入 sidecar：{error}"))?;
    }

    let stdout = child.stdout.take().ok_or("无法读取 sidecar 输出")?;
    let mut stderr = child.stderr.take().ok_or("无法读取 sidecar 错误输出")?;
    let stderr_reader = std::thread::spawn(move || {
        let mut output = String::new();
        let _ = stderr.read_to_string(&mut output);
        output
    });
    let mut result: Option<Value> = None;
    for line in BufReader::new(stdout).lines() {
        let line = line.map_err(|error| format!("无法读取 sidecar 输出：{error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .map_err(|error| format!("sidecar 返回了无效 JSON：{error}\n{line}"))?;
        if value.get("type").and_then(Value::as_str) == Some("result") {
            result = Some(value);
        } else if let Some(handler) = on_event.as_mut() {
            if let Err(error) = handler(value) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stderr_reader.join();
                return Err(error);
            }
        }
    }
    let status = child
        .wait()
        .map_err(|error| format!("sidecar 执行失败：{error}"))?;
    let stderr = stderr_reader.join().unwrap_or_default();
    let result = result
        .ok_or_else(|| format!("sidecar 未返回结果（exit={}）：{}", status, redact(&stderr)))?;
    if result.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        Ok(result.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(result
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("sidecar 执行失败")
            .to_string())
    }
}

fn sidecar_command() -> Result<Command, String> {
    if let Ok(explicit) = std::env::var("AI_JOB_SIDECAR") {
        return Ok(Command::new(explicit));
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

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let worker = manifest.join("..").join("sidecar").join("worker.py");
    if worker.exists() {
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
        return Ok(command);
    }

    Err("未找到 job-assistant-sidecar。开发环境请安装 Python 依赖，生产环境请先运行 sidecar 构建脚本。".into())
}

fn redact(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| {
            if token.starts_with("sk-") || token.starts_with("tp-") {
                "[REDACTED]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
