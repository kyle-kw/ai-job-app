use regex::Regex;
use std::sync::OnceLock;

fn secret_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?i)authorization\s*[:=]\s*(?:bearer\s+)?[^\s,;"']+|"?(?:api[_-]?key|access[_-]?token|secret|token)"?\s*[:=]\s*["']?[^\s,;"']+|\b(?:sk-|tp-|ghp_|github_pat_|AIza)[A-Za-z0-9._-]+"#)
        .expect("valid secret redaction regex")
    })
}

pub(crate) fn redact(value: &str) -> String {
    secret_pattern()
        .replace_all(value, "[REDACTED]")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_embedded_and_labeled_secrets() {
        for value in [
            "request(sk-secret-value)",
            "Authorization: Bearer abc.def.secret",
            r#"{"api_key":"custom-secret-value"}"#,
            "token=github_pat_1234567890",
        ] {
            let redacted = redact(value);
            assert!(redacted.contains("[REDACTED]"), "{redacted}");
            assert!(!redacted.contains("secret-value"), "{redacted}");
        }
    }
}
