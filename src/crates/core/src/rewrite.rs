//! Path rewriting for the Figma AI gateway.
//!
//! The `claude` process calls `https://api.figma.com/ai-gateway/llm/provider/universal/v1/...`.
//! Our local proxy rewrites that prefix to the LiteLLM proxy root (`/v1/...`).

pub const AI_GATEWAY_PREFIX: &str = "/ai-gateway/llm/provider/universal";

/// Rewrites an AI gateway path (optionally with query string) to the LiteLLM path.
///
/// Returns `None` when the path does not belong to the AI gateway, so the proxy can forward it
/// untouched.
pub fn rewrite_ai_path(path_and_query: &str) -> Option<String> {
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path_and_query, None),
    };

    let rest = path.strip_prefix(AI_GATEWAY_PREFIX)?;
    if !rest.is_empty() && !rest.starts_with('/') {
        return None;
    }

    let rewritten = if rest.is_empty() { "/" } else { rest };
    Some(match query {
        Some(query) => format!("{rewritten}?{query}"),
        None => rewritten.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_messages_path() {
        assert_eq!(
            rewrite_ai_path("/ai-gateway/llm/provider/universal/v1/messages").as_deref(),
            Some("/v1/messages")
        );
    }

    #[test]
    fn keeps_query_string() {
        assert_eq!(
            rewrite_ai_path(
                "/ai-gateway/llm/provider/universal/v1/messages/count_tokens?beta=true"
            )
            .as_deref(),
            Some("/v1/messages/count_tokens?beta=true")
        );
    }

    #[test]
    fn rewrites_exact_prefix_to_root() {
        assert_eq!(
            rewrite_ai_path("/ai-gateway/llm/provider/universal").as_deref(),
            Some("/")
        );
    }

    #[test]
    fn ignores_non_ai_paths() {
        assert_eq!(rewrite_ai_path("/api/files/123"), None);
        assert_eq!(
            rewrite_ai_path("/ai-gateway/llm/provider/universalx/v1"),
            None
        );
        assert_eq!(rewrite_ai_path("/ai-gateway/llm/provider"), None);
    }
}
