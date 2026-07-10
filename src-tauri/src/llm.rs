use crate::models::{AiProviderConfig, ProviderTestResult};
use keyring::Entry;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Instant;

const KEYRING_SERVICE: &str = "com.localfirst.aijobapp";

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Value,
}

pub fn store_secret(provider_id: &str, api_key: &str) -> Result<String, String> {
    let entry = Entry::new(KEYRING_SERVICE, provider_id).map_err(|error| error.to_string())?;
    entry
        .set_password(api_key)
        .map_err(|error| error.to_string())?;
    Ok(format!("keychain://{provider_id}"))
}

pub fn load_secret(provider: &AiProviderConfig) -> Result<String, String> {
    if let Some(key) = provider
        .api_key
        .as_ref()
        .filter(|key| !key.trim().is_empty())
    {
        return Ok(key.clone());
    }
    let entry = Entry::new(KEYRING_SERVICE, &provider.id).map_err(|error| error.to_string())?;
    entry
        .get_password()
        .map_err(|_| "未找到 API Key，请在设置中重新填写。".to_string())
}

pub async fn test(provider: &AiProviderConfig) -> Result<ProviderTestResult, String> {
    if provider.base_url.trim().is_empty() || provider.model.trim().is_empty() {
        return Err("请填写 Base URL 和模型名。".into());
    }
    let key = load_secret(provider)?;
    let started = Instant::now();
    let value: Value = chat_json(
        provider,
        &key,
        "You are a connectivity tester. Return JSON only.",
        r#"Return exactly this JSON object: {"ok":true,"message":"connected"}"#,
    )
    .await?;
    let structured = value.get("ok").and_then(Value::as_bool) == Some(true);
    Ok(ProviderTestResult {
        ok: structured,
        message: if structured {
            "连接成功，结构化输出正常".into()
        } else {
            "模型已响应，但结构化输出未通过".into()
        },
        latency_ms: started.elapsed().as_millis() as i64,
        structured_output: structured,
    })
}

pub async fn run_skill<T: DeserializeOwned>(
    provider: &AiProviderConfig,
    system_prompt: &str,
    input: &Value,
) -> Result<T, String> {
    let key = load_secret(provider)?;
    let user = format!(
        "Use the following input. Return only one JSON object matching the output contract in the skill.\n\n{}",
        serde_json::to_string_pretty(input).map_err(|error| error.to_string())?
    );
    let value = chat_json(provider, &key, system_prompt, &user).await?;
    serde_json::from_value(value).map_err(|error| format!("模型输出不符合 Skill 合约：{error}"))
}

async fn chat_json(
    provider: &AiProviderConfig,
    api_key: &str,
    system: &str,
    user: &str,
) -> Result<Value, String> {
    let endpoint = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let body = json!({
        "model": provider.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "temperature": 0.2,
        "max_completion_tokens": 3000,
        "response_format": {"type": "json_object"}
    });
    let response = Client::new()
        .post(endpoint)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("无法连接模型服务：{error}"))?;
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("模型服务返回 {status}：{}", redact(&text)));
    }
    let parsed: ChatResponse =
        serde_json::from_str(&text).map_err(|error| format!("无法解析模型响应：{error}"))?;
    let content = parsed
        .choices
        .first()
        .ok_or_else(|| "模型没有返回内容。".to_string())?
        .message
        .content
        .clone();
    match content {
        Value::String(text) => parse_json_content(&text),
        value if value.is_object() => Ok(value),
        _ => Err("模型返回了不支持的内容格式。".into()),
    }
}

fn parse_json_content(content: &str) -> Result<Value, String> {
    let trimmed = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }
    let start = trimmed
        .find('{')
        .ok_or_else(|| "模型输出中没有 JSON 对象。".to_string())?;
    let end = trimmed
        .rfind('}')
        .ok_or_else(|| "模型输出中的 JSON 不完整。".to_string())?;
    serde_json::from_str(&trimmed[start..=end]).map_err(|error| format!("模型 JSON 无效：{error}"))
}

fn redact(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for token in value.split_whitespace() {
        if token.starts_with("sk-") || token.starts_with("tp-") {
            output.push_str("[REDACTED]");
        } else {
            output.push_str(token);
        }
        output.push(' ');
    }
    output.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_fenced_content() {
        let value = parse_json_content("```json\n{\"ok\":true}\n```").unwrap();
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn redacts_common_key_prefixes() {
        assert!(!redact("failure sk-secret-value").contains("secret"));
    }
}
