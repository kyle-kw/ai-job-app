use crate::models::{AiProviderConfig, ProviderTestResult};
use keyring::Entry;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
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

pub fn secret_available(provider: &AiProviderConfig) -> bool {
    load_secret(provider).is_ok()
}

pub fn delete_secret(provider_id: &str) -> Result<(), String> {
    let entry = Entry::new(KEYRING_SERVICE, provider_id).map_err(|error| error.to_string())?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
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
    let (vision_supported, vision_message) = match test_vision(provider, &key).await {
        Ok(true) => (true, "图片识别能力正常".to_string()),
        Ok(false) => (false, "模型接收了图片，但未能读取测试文字".to_string()),
        Err(error) => (false, format!("图片能力未通过：{}", redact(&error))),
    };
    Ok(ProviderTestResult {
        ok: structured,
        message: if structured {
            "连接成功，结构化输出正常".into()
        } else {
            "模型已响应，但结构化输出未通过".into()
        },
        latency_ms: started.elapsed().as_millis() as i64,
        structured_output: structured,
        vision_supported,
        vision_message,
    })
}

const VISION_PROBE_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAANwAAABQCAYAAAByKBsiAAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAAAJcEhZcwAADsMAAA7DAcdvqGQAAAVOSURBVHhe7ZntdSoxDAVTHgVRDr2klXTCW5LwwqJr+8q7KJwwc45+gb9kjW2StzMAlIFwAIUgHEAhCAdQCMIBFIJwAIUgHEAhCAdQCMIBFIJwAIUgHEAhCAdQCMIBFIJwAIUgHEAhCAdQCMIBFIJwAIUgHEAhCAdQCMIBFIJwAIUgHEAhCAdQCMIBFIJwAIUgHEAhE8J9nE+Ht/Pb220czqeP74+H9NqLz47vn626vB/Xbe5i3MXkuN98nA7rtqvI5OaylNjHzPwPmUHvUHNwozvXj9P5INpsmWvoM7Fvv8HUDacKzE6aSvrhtJTM54epwu8Xeoz2HOeEyxamkyPZ5//8tHh24UR+Q+QOpithrn9RuL40fZQkPznyCz8r2zV0IWaFez8fb7+biUGeWsXeF+i5hfP7S0qnXjZ/UjhVoFayVLvjUr5XzMIfPCH7oeaZEW6DbNfoSNcuzl5+n0Q4ta7sXpkHd7PfvyncssUzz0p1M64S5BW+LAiZaCW4+q4rXKO/S8hC6cgp+x8Ue6ONmtcW4SzCXt4enD/E9QwOWOPgnsvRczAtnJRncDr1n5MXnMKPRZx9bsV5OuMuyFPVuNkT7Ua3i66nauHieHpeca/C90Qd6b4WnNuy2fg5mBduIRZH7tkTT0Wn8LPCOTjjqvl3iuMO+ZtTNB4Jpw+1OLdHChfW0kvCSihxC5rC2b/Z3Q35JTYJp06c5kaLxMbvzha+cct0McaduNHXqOdlLMChcEs4eXuYcCEP+ilpE2pI96dfR069PBfbhFNF1CjCmDAliZfAXlHO5dsYN3O4NHBeBOE7h0MUPbSrE+5+fpvGSeR0VT//a+zlhPOKSCbGfBrJBKrbRoWd/PG4+oT9/tDE6SMKt+RJ/XZZNSwSbqfbTeXhEr05f7YJNfOCwlmnlPWcvJBIoCrCTvQLcDyud7AMMHIlhVPzW+JnijXC7XW77ZLLT15ROONZGU+0VoKTCXRvupvQ3T27cAtqrf/zXCDcTrebzPVN5Hx5SeFGxSiSEp4GVyYTmBQvFuN43F8XbkE9xb7ax/nvLVwYe6fCVmvyu35R4bqFJGRo52SHBFpPzXtZxuNuK4wvnD56wi2fir90XtZiCGfkpb2eOG527T1CXpoH8j2vKlznWRmLrPcU2TmBnZtvXZDGuMbtNMK5JfvCLShxjqfHChfabvxXwD1hn9zXw8sK1yqmbEIelUBxIKz6NcZV8ton8QV1O8XCHQq3EHMdY0/hwni77MkNqVfQLS8snNrQw/GYTOQogfMJ7heN06/4zhLuLaeek2rujnC9m/sa+wkX1+2u2VrLBYSbQZ3g9zF6iowTOPdbaocb7kKjaEfjS9ka7dwibfV5DVeKIdPPPTVH3TasmSelx/YiMBLYOt1bibYkcTdOfO8aUozOIdSYr30rDA643YQL+Uv8fpN7tZZJ1kxzzfe8uHBNGT7DObW8BMYTMRlhQzMb59zko2gXrS/cQueZuJdwQQhbhi9m9sp35tWFUwm4hrVRbgI74wxDiZ/duA3SDfKQEm6hVdB7CZedTySXq9y8X164JQWNZ6WXyFwC86dn62aZ27js+E4O0gXeeFU8TLipgvaky88Z4RoF8NgfwaPfjuM+Nm5c9y+A7tq/mLlR1Pr3EW7ngs7+/h6CcADQAeEACkE4gEIQDqAQhAMoBOEACkE4gEIQDqAQhAMoBOEACkE4gEIQDqAQhAMoBOEACkE4gEIQDqAQhAMoBOEACkE4gEIQDqAQhAMoBOEACkE4gEIQDqAQhAMoBOEACkE4gEIQDqAQhAMoBOEAyjif/wEVqgfKZ+5UfAAAAABJRU5ErkJggg==";

async fn test_vision(provider: &AiProviderConfig, api_key: &str) -> Result<bool, String> {
    let content = json!([
        {"type":"text","text":"Read the exact code in this image. Return JSON only: {\"vision\":\"the code\"}."},
        {"type":"image_url","image_url":{"url":format!("data:image/png;base64,{VISION_PROBE_PNG_BASE64}")}}
    ]);
    let value = chat_json_with_content(provider, api_key, "You test image-reading capability and return JSON only.", content).await?;
    Ok(value.get("vision").and_then(Value::as_str).is_some_and(|value| value.trim().eq_ignore_ascii_case("VISION-731")))
}

pub async fn transcribe_resume_page(
    provider: &AiProviderConfig,
    image_data_url: &str,
    page_number: usize,
) -> Result<String, String> {
    let key = load_secret(provider)?;
    let content = json!([
        {"type":"text","text":format!("Transcribe resume page {page_number} exactly in reading order. Preserve names, dates, numbers, links, headings, bullets, and table cell text. Do not summarize or infer. Return JSON only as {{\"text\":\"...\"}}.")},
        {"type":"image_url","image_url":{"url":image_data_url}}
    ]);
    let value = chat_json_with_content(
        provider,
        &key,
        "You are a precise resume OCR transcriber. Never add text not visible in the image.",
        content,
    )
    .await?;
    value.get("text").and_then(Value::as_str).map(str::to_string).ok_or_else(|| "视觉模型未返回页面文字。".to_string())
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
    chat_json_with_content(provider, api_key, system, Value::String(user.to_string())).await
}

async fn chat_json_with_content(
    provider: &AiProviderConfig,
    api_key: &str,
    system: &str,
    user_content: Value,
) -> Result<Value, String> {
    let endpoint = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let body = json!({
        "model": provider.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.2,
        "max_completion_tokens": 3000,
        "response_format": {"type": "json_object"}
    });
    let response = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("无法创建模型客户端：{error}"))?
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
