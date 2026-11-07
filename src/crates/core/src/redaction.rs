//! Redaction helpers for logs, diagnostics and error details.
//!
//! The rule is simple: secrets never leave the process. These helpers are used
//! before writing logs or exporting diagnostics.

pub const REDACTED: &str = "[REDACTED]";

pub fn is_sensitive_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "authorization"
            | "proxy-authorization"
            | "x-api-key"
            | "api-key"
            | "apikey"
            | "cookie"
            | "set-cookie"
            | "x-auth-token"
            | "x-access-token"
    )
}

pub fn redact_header(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        REDACTED.to_owned()
    } else {
        redact_text(value, &[])
    }
}

fn is_sensitive_query_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "token"
            | "access_token"
            | "api_key"
            | "apikey"
            | "key"
            | "password"
            | "secret"
            | "sig"
            | "signature"
            | "code"
            | "auth"
    )
}

fn redact_query(query: &str) -> String {
    query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, _)) if is_sensitive_query_key(key) => format!("{key}={REDACTED}"),
            None if is_sensitive_query_key(pair) => format!("{pair}={REDACTED}"),
            _ => pair.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Removes userinfo (user:password@) and sensitive query values from a URL.
pub fn redact_url(input: &str) -> String {
    let (without_fragment, fragment) = match input.split_once('#') {
        Some((url, fragment)) => (url, Some(fragment)),
        None => (input, None),
    };
    let (base, query) = match without_fragment.split_once('?') {
        Some((url, query)) => (url, Some(query)),
        None => (without_fragment, None),
    };

    let mut redacted_base = base.to_owned();
    if let Some(scheme_end) = base.find("://") {
        let authority_start = scheme_end + 3;
        let authority_end = base[authority_start..]
            .find('/')
            .map(|offset| authority_start + offset)
            .unwrap_or(base.len());
        let authority = &base[authority_start..authority_end];
        if let Some(at) = authority.rfind('@') {
            let host = &authority[at + 1..];
            redacted_base = format!(
                "{}{}@{}{}",
                &base[..authority_start],
                REDACTED,
                host,
                &base[authority_end..]
            );
        }
    }

    match query {
        Some(query) => {
            let mut output = format!("{redacted_base}?{}", redact_query(query));
            if let Some(fragment) = fragment {
                output.push('#');
                output.push_str(fragment);
            }
            output
        }
        None => {
            let mut output = redacted_base;
            if let Some(fragment) = fragment {
                output.push('#');
                output.push_str(fragment);
            }
            output
        }
    }
}

/// Replaces known secret values and bearer tokens in free-form text.
pub fn redact_text(text: &str, secrets: &[&str]) -> String {
    let mut output = text.to_owned();
    for secret in secrets {
        if !secret.is_empty() {
            output = output.replace(secret, REDACTED);
        }
    }
    redact_bearer_tokens(&output)
}

fn redact_bearer_tokens(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(position) = rest.find("Bearer ") {
        let (before, after) = rest.split_at(position);
        output.push_str(before);
        output.push_str("Bearer ");

        let after_prefix = &after["Bearer ".len()..];
        let end = after_prefix
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '"' | '\'' | ',' | ';' | ')' | ']')
            })
            .unwrap_or(after_prefix.len());

        if end == 0 {
            output.push_str(after_prefix);
            return output;
        }

        output.push_str(REDACTED);
        rest = &after_prefix[end..];
    }

    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_sensitive_headers_case_insensitively() {
        for header in [
            "Authorization",
            "PROXY-AUTHORIZATION",
            "x-api-key",
            "Cookie",
            "Set-Cookie",
            "x-access-token",
        ] {
            assert!(is_sensitive_header(header), "{header}");
        }
        assert!(!is_sensitive_header("Content-Type"));
        assert!(!is_sensitive_header("Accept"));
    }

    #[test]
    fn redacts_sensitive_header_values() {
        assert_eq!(redact_header("Authorization", "Bearer abc"), REDACTED);
        assert_eq!(
            redact_header("content-type", "application/json"),
            "application/json"
        );
    }

    #[test]
    fn redacts_url_userinfo_and_sensitive_query_values() {
        let url = "https://user:pass@example.com/v1/messages?api_key=secret&beta=true#top";
        let redacted = redact_url(url);
        assert!(!redacted.contains("user"));
        assert!(!redacted.contains("pass"));
        assert!(!redacted.contains("secret"));
        assert!(redacted.contains("example.com"));
        assert!(redacted.contains("beta=true"));
        assert!(redacted.contains("[REDACTED]"));
        assert!(redacted.ends_with("#top"));
    }

    #[test]
    fn redacts_sensitive_query_without_value() {
        let redacted = redact_url("https://example.com/callback?code&state=1");
        assert_eq!(
            redacted,
            "https://example.com/callback?code=[REDACTED]&state=1"
        );
    }

    #[test]
    fn keeps_urls_without_credentials_intact() {
        let url = "https://api.figma.com/ai-gateway/llm/provider/universal/v1/messages";
        assert_eq!(redact_url(url), url);
    }

    #[test]
    fn redacts_bearer_tokens_in_free_form_text() {
        let redacted = redact_text("request failed: Authorization: Bearer abc123, retry", &[]);
        assert!(!redacted.contains("abc123"));
        assert!(redacted.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn redacts_known_secret_values() {
        let redacted = redact_text(
            "provider failed with key sk-live-123 and password hunter2",
            &["sk-live-123", "hunter2"],
        );
        assert!(!redacted.contains("sk-live-123"));
        assert!(!redacted.contains("hunter2"));
        assert_eq!(redacted.matches(REDACTED).count(), 2);
    }

    #[test]
    fn empty_secrets_are_ignored() {
        assert_eq!(redact_text("texto", &[""]), "texto");
    }

    #[test]
    fn dangling_bearer_does_not_loop() {
        assert_eq!(redact_text("Bearer ", &[]), "Bearer ");
    }
}
