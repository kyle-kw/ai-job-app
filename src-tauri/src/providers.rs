use crate::llm;
use crate::models::{AiProviderConfig, ProviderSaveResult, ProviderTestResult};
use crate::time;
use crate::AppState;
use tauri::State;

fn with_existing_secret(
    mut provider: AiProviderConfig,
    existing: Option<&AiProviderConfig>,
) -> AiProviderConfig {
    if provider
        .api_key
        .as_deref()
        .is_none_or(|key| key.trim().is_empty())
    {
        provider.api_key = None;
        provider.api_key_ref = existing.and_then(|item| item.api_key_ref.clone());
    }
    provider
}

fn failed_test(error: String) -> ProviderTestResult {
    ProviderTestResult {
        ok: false,
        message: llm::redact(&error),
        latency_ms: 0,
        structured_output: false,
        vision_supported: false,
        vision_message: "未进行图片能力测试".into(),
    }
}

#[tauri::command]
pub async fn test_provider(
    state: State<'_, AppState>,
    provider: AiProviderConfig,
) -> Result<ProviderTestResult, String> {
    let existing = state.db.provider_by_id(&provider.id)?;
    let candidate = with_existing_secret(provider, existing.as_ref());
    Ok(llm::test(&candidate).await.unwrap_or_else(failed_test))
}

#[tauri::command]
pub async fn save_provider(
    state: State<'_, AppState>,
    provider: AiProviderConfig,
) -> Result<ProviderSaveResult, String> {
    if provider.kind == "openrouter" {
        return Err("OpenRouter 预设已移除，请使用自定义模型。".into());
    }
    let existing = state.db.provider_by_id(&provider.id)?;
    let mut candidate = with_existing_secret(provider, existing.as_ref());
    let test_result = llm::test(&candidate)
        .await
        .map_err(|error| llm::redact(&error))?;
    if !test_result.ok {
        return Err(test_result.message);
    }

    let new_key = candidate
        .api_key
        .take()
        .filter(|key| !key.trim().is_empty());
    let old_key = existing
        .as_ref()
        .and_then(|provider| llm::load_secret(provider).ok());
    if let Some(key) = new_key.as_deref() {
        candidate.api_key_ref = Some(llm::store_secret(&candidate.id, key)?);
    }
    candidate.verified = true;
    candidate.vision_verified = test_result.vision_supported;
    candidate.last_tested_at = Some(time::shanghai_rfc3339());
    candidate.last_test_error = None;

    if let Err(error) = state.db.save_provider(&candidate) {
        if new_key.is_some() {
            if let Some(old_key) = old_key {
                let _ = llm::store_secret(&candidate.id, &old_key);
            } else {
                let _ = llm::delete_secret(&candidate.id);
            }
        }
        return Err(error);
    }
    Ok(ProviderSaveResult {
        providers: state.db.list_providers()?,
        test_result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> AiProviderConfig {
        AiProviderConfig {
            id: "provider-test".into(),
            kind: "custom".into(),
            name: "Test".into(),
            base_url: "https://example.com/v1".into(),
            model: "test".into(),
            allow_insecure_http: false,
            api_key: None,
            api_key_ref: None,
            is_default: true,
            verified: false,
            vision_verified: false,
            last_tested_at: None,
            last_test_error: None,
        }
    }

    #[test]
    fn temporary_test_input_reuses_the_existing_secret_reference() {
        let mut existing = provider();
        existing.api_key_ref = Some("keychain://provider-test".into());
        let prepared = with_existing_secret(provider(), Some(&existing));
        assert_eq!(prepared.api_key_ref, existing.api_key_ref);
        assert!(prepared.api_key.is_none());
    }

    #[test]
    fn failed_test_result_is_transient_and_redacted() {
        let result = failed_test("request failed sk-secret".into());
        assert!(!result.ok);
        assert!(!result.message.contains("sk-secret"));
    }
}
