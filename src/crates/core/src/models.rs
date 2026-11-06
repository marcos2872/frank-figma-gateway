//! Model names used by the Figma agent and constraints imposed by Claude Code.

/// Default model sent by figcode for the main agent.
pub const DEFAULT_OPUS_MODEL: &str = "claude-opus-4-8";

/// Model used for auxiliary/fast tasks.
pub const DEFAULT_HAIKU_MODEL: &str = "claude-haiku-4-5";

/// Ids que o seletor do Make envia (validados no Figma desktop 126.11.7).
///
/// São codenames da Figma, não ids do Claude Code. O agente também usa ids
/// `claude-*` para as tarefas do filho `claude`, então o mapeamento precisa
/// aceitar as duas famílias.
pub const FIGMA_MODEL_IDS: &[&str] = &[
    "default",
    "anthropic-claude-4.6-opus",
    "anthropic-claude-4.8-opus",
    "anthropic-claude-5-opus",
    "anthropic-claude-5.5-opus",
    "anthropic-claude-4.6-sonnet",
    "anthropic-claude-5-sonnet",
    "anthropic-claude-5.5-sonnet",
    "figaro-sonnet-4.6",
    "figaro-gpt-5.6-sol",
    "figaro-gpt-5.6-terra",
    "figaro-gpt-6-astra",
    "figaro-gpt-6-luna",
    "figaro-gpt-6-sol",
    "google-gemini-3-pro",
    "google-gemini-3.1-pro",
    "google-gemini-3-flash",
    "google-gemini-3.8-flash",
    "aqua",
    "velvet",
    "goldenrod",
    "emerald",
    "tangelo",
];

/// Claude Code only lists gateway models whose id starts with `claude-` or `anth-`.
pub fn is_claude_model_name(name: &str) -> bool {
    name.starts_with("claude-") || name.starts_with("anth-")
}

/// Ids do seletor do Make (codenames da Figma).
pub fn is_figma_model_id(name: &str) -> bool {
    FIGMA_MODEL_IDS.contains(&name)
}

/// Public names are shown to Claude Code and to the Figma picker. Aceita as duas
/// famílias de ids e exige apenas caracteres seguros de identificador.
pub fn is_valid_public_name(name: &str) -> bool {
    !name.is_empty()
        && (is_claude_model_name(name) || is_figma_model_id(name))
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
    fn recognizes_figma_picker_ids() {
        for id in [
            "default",
            "anthropic-claude-5.5-opus",
            "figaro-gpt-6-sol",
            "google-gemini-3.8-flash",
            "aqua",
            "velvet",
        ] {
            assert!(is_figma_model_id(id), "{id}");
        }
        assert!(!is_figma_model_id("gpt-6.1-sol"));
        assert!(!is_figma_model_id("claude-opus-4-8"));
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
        assert!(is_valid_public_name("figaro-gpt-6-sol"));
        assert!(is_valid_public_name("anthropic-claude-5.5-opus"));
        assert!(is_valid_public_name("aqua"));
        assert!(!is_valid_public_name(""));
        assert!(!is_valid_public_name("gpt-5"));
        assert!(!is_valid_public_name("claude opus"));
        assert!(!is_valid_public_name("claude/opus"));
        assert!(!is_valid_public_name("Gemini 4 Argon"));
    }
}
