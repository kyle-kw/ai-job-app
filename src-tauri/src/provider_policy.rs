use crate::models::AiProviderConfig;

pub fn is_production_build() -> bool {
    !cfg!(debug_assertions)
}

pub fn provider_allowed(provider: &AiProviderConfig) -> bool {
    provider_allowed_for(provider, is_production_build())
}

pub fn provider_allowed_for(provider: &AiProviderConfig, production: bool) -> bool {
    provider.kind != "openrouter" && (!production || provider.kind == "custom")
}

pub fn available_providers(providers: Vec<AiProviderConfig>) -> Vec<AiProviderConfig> {
    available_providers_for(providers, is_production_build())
}

pub fn available_providers_for(
    providers: Vec<AiProviderConfig>,
    production: bool,
) -> Vec<AiProviderConfig> {
    let mut available = providers
        .into_iter()
        .filter(|provider| provider_allowed_for(provider, production))
        .collect::<Vec<_>>();
    if !available.is_empty() && !available.iter().any(|provider| provider.is_default) {
        available[0].is_default = true;
    }
    available
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &str, kind: &str, is_default: bool) -> AiProviderConfig {
        AiProviderConfig {
            id: id.into(),
            kind: kind.into(),
            name: id.into(),
            base_url: "https://example.com/v1".into(),
            model: "model".into(),
            allow_insecure_http: false,
            api_key: None,
            api_key_ref: None,
            is_default,
            verified: true,
            vision_verified: false,
            last_tested_at: None,
            last_test_error: None,
        }
    }

    #[test]
    fn development_keeps_xiaomi_and_custom_providers() {
        let providers = available_providers_for(
            vec![provider("xiaomi", "xiaomi", true), provider("custom", "custom", false)],
            false,
        );
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].kind, "xiaomi");
        assert!(providers[0].is_default);
    }

    #[test]
    fn production_only_exposes_an_effective_custom_default() {
        let providers = available_providers_for(
            vec![provider("xiaomi", "xiaomi", true), provider("custom", "custom", false)],
            true,
        );
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].kind, "custom");
        assert!(providers[0].is_default);
    }

    #[test]
    fn production_rejects_direct_xiaomi_access() {
        assert!(!provider_allowed_for(&provider("xiaomi", "xiaomi", true), true));
        assert!(provider_allowed_for(&provider("custom", "custom", true), true));
    }
}
