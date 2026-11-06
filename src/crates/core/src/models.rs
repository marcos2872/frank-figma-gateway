//! Model names used by the Figma agent and constraints imposed by Claude Code.

/// Default model sent by figcode for the main agent.
pub const DEFAULT_OPUS_MODEL: &str = "claude-opus-4-8";

/// Model used for auxiliary/fast tasks.
pub const DEFAULT_HAIKU_MODEL: &str = "claude-haiku-4-5";

/// Claude Code only lists gateway models whose id starts with `claude-` or `anth-`.
pub fn is_claude_model_name(name: &str) -> bool {
    name.starts_with("claude-") || name.starts_with("anth-")
}

/// Public names are shown to Claude Code; they must keep the `claude-`/`anth-`
/// prefix and contain only identifier-safe characters.
pub fn is_valid_public_name(name: &str) -> bool {
    !name.is_empty()
        && is_claude_model_name(name)
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

pub fn default_models() -> Vec<&'static str> {
    vec![DEFAULT_OPUS_MODEL, DEFAULT_HAIKU_MODEL]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_claude_and_anth_names() {
        assert!(is_claude_model_name("claude-opus-4-8"));
        assert!(is_claude_model_name("anth-something"));
    }

    #[test]
    fn rejects_other_names() {
        assert!(!is_claude_model_name("gpt-5.2"));
        assert!(!is_claude_model_name("gemini-3.8-flash"));
    }

    #[test]
    fn exposes_defaults() {
        assert_eq!(
            default_models(),
            vec![DEFAULT_OPUS_MODEL, DEFAULT_HAIKU_MODEL]
        );
    }

    #[test]
    fn validates_public_names() {
        assert!(is_valid_public_name("claude-opus-4-8"));
        assert!(is_valid_public_name("anth-model.v2"));
        assert!(!is_valid_public_name(""));
        assert!(!is_valid_public_name("gpt-5"));
        assert!(!is_valid_public_name("claude opus"));
        assert!(!is_valid_public_name("claude/opus"));
    }
}
