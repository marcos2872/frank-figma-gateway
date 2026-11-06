//! Provider catalog and pure validation of provider profiles.
//!
//! No adapter is implemented here: LiteLLM translates provider requests. This
//! module only describes the fields the wizard needs and validates the input.

use crate::setup::{ProviderKind, ProviderSelection, TestStatus};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldKind {
    Text,
    Password,
    Url,
    Select,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderField {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
    pub required: bool,
    pub secret: bool,
    #[serde(default)]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub kind: ProviderKind,
    pub label: String,
    pub description: String,
    pub requires_postgres: bool,
    pub fields: Vec<ProviderField>,
    #[serde(default)]
    pub recommended_model: Option<String>,
}

pub fn all_provider_kinds() -> [ProviderKind; 7] {
    [
        ProviderKind::OpenRouter,
        ProviderKind::AzureAi,
        ProviderKind::Anthropic,
        ProviderKind::OpenAI,
        ProviderKind::Gemini,
        ProviderKind::GitHubCopilot,
        ProviderKind::OpenAICompatible,
    ]
}

pub fn requires_api_key(kind: ProviderKind) -> bool {
    matches!(
        kind,
        ProviderKind::OpenRouter
            | ProviderKind::AzureAi
            | ProviderKind::Anthropic
            | ProviderKind::OpenAI
            | ProviderKind::Gemini
    )
}

pub fn requires_postgres(kind: ProviderKind) -> bool {
    kind == ProviderKind::GitHubCopilot
}

pub fn default_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::OpenRouter => "OpenRouter",
        ProviderKind::AzureAi => "Azure AI",
        ProviderKind::Anthropic => "Anthropic",
        ProviderKind::OpenAI => "OpenAI",
        ProviderKind::Gemini => "Google Gemini",
        ProviderKind::GitHubCopilot => "GitHub Copilot",
        ProviderKind::OpenAICompatible => "OpenAI-compatible / local",
    }
}

pub fn default_profile_id(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::OpenRouter => "openrouter",
        ProviderKind::AzureAi => "azure",
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::OpenAI => "openai",
        ProviderKind::Gemini => "gemini",
        ProviderKind::GitHubCopilot => "copilot",
        ProviderKind::OpenAICompatible => "local",
    }
}

pub fn provider_env_var(kind: ProviderKind) -> Option<&'static str> {
    match kind {
        ProviderKind::OpenRouter => Some("OPENROUTER_API_KEY"),
        ProviderKind::AzureAi => Some("AZURE_API_KEY"),
        ProviderKind::Anthropic => Some("ANTHROPIC_API_KEY"),
        ProviderKind::OpenAI => Some("OPENAI_API_KEY"),
        ProviderKind::Gemini => Some("GEMINI_API_KEY"),
        ProviderKind::GitHubCopilot => None,
        ProviderKind::OpenAICompatible => Some("OPENAI_API_KEY"),
    }
}

pub fn secret_ref_for(kind: ProviderKind, profile_id: &str) -> Option<String> {
    if requires_api_key(kind) || kind == ProviderKind::OpenAICompatible {
        Some(format!("ffg/provider/{profile_id}"))
    } else {
        None
    }
}

fn field(
    id: &str,
    label: &str,
    kind: FieldKind,
    required: bool,
    secret: bool,
    placeholder: Option<&str>,
) -> ProviderField {
    ProviderField {
        id: id.to_owned(),
        label: label.to_owned(),
        kind,
        required,
        secret,
        placeholder: placeholder.map(str::to_owned),
    }
}

pub fn catalog() -> Vec<ProviderDescriptor> {
    all_provider_kinds()
        .into_iter()
        .map(|kind| {
            let fields = match kind {
                ProviderKind::OpenRouter
                | ProviderKind::Anthropic
                | ProviderKind::OpenAI
                | ProviderKind::Gemini => vec![field(
                    "apiKey",
                    "API key",
                    FieldKind::Password,
                    true,
                    true,
                    None,
                )],
                ProviderKind::AzureAi => vec![
                    field(
                        "apiBase",
                        "Endpoint",
                        FieldKind::Url,
                        true,
                        false,
                        Some("https://<resource>.openai.azure.com"),
                    ),
                    field("apiKey", "API key", FieldKind::Password, true, true, None),
                    field(
                        "apiVersion",
                        "API version",
                        FieldKind::Text,
                        true,
                        false,
                        Some("2024-10-21"),
                    ),
                    field(
                        "deployment",
                        "Deployment",
                        FieldKind::Text,
                        true,
                        false,
                        None,
                    ),
                ],
                ProviderKind::GitHubCopilot => vec![],
                ProviderKind::OpenAICompatible => vec![
                    field(
                        "apiBase",
                        "Base URL",
                        FieldKind::Url,
                        true,
                        false,
                        Some("http://127.0.0.1:11434/v1"),
                    ),
                    field(
                        "apiKey",
                        "API key (opcional)",
                        FieldKind::Password,
                        false,
                        true,
                        None,
                    ),
                ],
            };

            ProviderDescriptor {
                kind,
                label: default_label(kind).to_owned(),
                description: match kind {
                    ProviderKind::GitHubCopilot => {
                        "Requer Postgres embutido e OAuth do GitHub.".to_owned()
                    }
                    ProviderKind::OpenAICompatible => {
                        "Qualquer endpoint compatível com OpenAI, incluindo modelos locais."
                            .to_owned()
                    }
                    _ => "Conecta direto pelo LiteLLM, sem adapter próprio.".to_owned(),
                },
                requires_postgres: requires_postgres(kind),
                fields,
                recommended_model: None,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileInput {
    pub kind: ProviderKind,
    pub label: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub deployment: Option<String>,
}

impl ProviderProfileInput {
    pub fn profile_id(&self) -> String {
        self.profile_id
            .clone()
            .unwrap_or_else(|| default_profile_id(self.kind).to_owned())
    }

    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.label.trim().is_empty() {
            return Err(ProviderError::EmptyLabel);
        }
        if !is_safe_profile_id(&self.profile_id()) {
            return Err(ProviderError::InvalidProfileId(self.profile_id()));
        }

        if requires_api_key(self.kind) && !has_value(&self.api_key) {
            return Err(ProviderError::MissingField("apiKey"));
        }

        if self.kind == ProviderKind::GitHubCopilot && has_value(&self.api_key) {
            return Err(ProviderError::ApiKeyNotSupported);
        }

        if matches!(
            self.kind,
            ProviderKind::AzureAi | ProviderKind::OpenAICompatible
        ) {
            let api_base = self
                .api_base
                .as_deref()
                .ok_or(ProviderError::MissingField("apiBase"))?;
            validate_api_base(api_base)?;
        }

        if self.kind == ProviderKind::AzureAi {
            if !has_value(&self.api_version) {
                return Err(ProviderError::MissingField("apiVersion"));
            }
            if !has_value(&self.deployment) {
                return Err(ProviderError::MissingField("deployment"));
            }
        }

        Ok(())
    }

    pub fn to_selection(&self) -> Result<ProviderSelection, ProviderError> {
        self.validate()?;
        let profile_id = self.profile_id();
        Ok(ProviderSelection {
            profile_id: profile_id.clone(),
            kind: self.kind,
            label: self.label.trim().to_owned(),
            api_base: self.api_base.clone(),
            api_version: self.api_version.clone(),
            deployment: self.deployment.clone(),
            secret_ref: if has_value(&self.api_key) {
                secret_ref_for(self.kind, &profile_id)
            } else {
                None
            },
            test_status: TestStatus::NotTested,
            tested_at: None,
        })
    }
}

fn has_value(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

fn is_safe_profile_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn validate_api_base(value: &str) -> Result<(), ProviderError> {
    let value = value.trim();
    let https = value.starts_with("https://");
    let local_http = value.starts_with("http://127.0.0.1")
        || value.starts_with("http://localhost")
        || value.starts_with("http://[::1]");
    if !https && !local_http {
        return Err(ProviderError::InvalidUrl(value.to_owned()));
    }
    if value.len() <= value.find("://").map(|index| index + 3).unwrap_or(0) {
        return Err(ProviderError::InvalidUrl(value.to_owned()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    EmptyLabel,
    MissingField(&'static str),
    InvalidUrl(String),
    InvalidProfileId(String),
    ApiKeyNotSupported,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::EmptyLabel => f.write_str("nome do perfil não pode ser vazio"),
            ProviderError::MissingField(field) => write!(f, "campo obrigatório ausente: {field}"),
            ProviderError::InvalidUrl(url) => write!(f, "URL inválida: {url}"),
            ProviderError::InvalidProfileId(id) => write!(f, "id de perfil inválido: {id}"),
            ProviderError::ApiKeyNotSupported => {
                f.write_str("GitHub Copilot usa OAuth, não API key")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn openrouter() -> ProviderProfileInput {
        ProviderProfileInput {
            kind: ProviderKind::OpenRouter,
            label: "OpenRouter".to_owned(),
            profile_id: None,
            api_key: Some("sk-test".to_owned()),
            api_base: None,
            api_version: None,
            deployment: None,
        }
    }

    #[test]
    fn catalog_covers_every_provider() {
        let catalog = catalog();
        assert_eq!(catalog.len(), all_provider_kinds().len());
        let azure = catalog
            .iter()
            .find(|item| item.kind == ProviderKind::AzureAi)
            .expect("azure");
        assert_eq!(azure.fields.len(), 4);
        assert!(azure.fields.iter().any(|field| field.id == "apiBase"));
        assert!(
            catalog
                .iter()
                .find(|item| item.kind == ProviderKind::GitHubCopilot)
                .expect("copilot")
                .requires_postgres
        );
    }

    #[test]
    fn validates_api_key_providers() {
        let profile = openrouter();
        assert!(profile.validate().is_ok());
        let selection = profile.to_selection().expect("selection");
        assert_eq!(selection.profile_id, "openrouter");
        assert_eq!(
            selection.secret_ref.as_deref(),
            Some("ffg/provider/openrouter")
        );
        assert_eq!(selection.test_status, TestStatus::NotTested);
    }

    #[test]
    fn rejects_missing_api_key_and_empty_label() {
        let mut profile = openrouter();
        profile.api_key = None;
        assert_eq!(
            profile.validate().expect_err("must fail"),
            ProviderError::MissingField("apiKey")
        );

        profile.api_key = Some("  ".to_owned());
        assert_eq!(
            profile.validate().expect_err("must fail"),
            ProviderError::MissingField("apiKey")
        );

        profile.api_key = Some("sk-test".to_owned());
        profile.label = "  ".to_owned();
        assert_eq!(
            profile.validate().expect_err("must fail"),
            ProviderError::EmptyLabel
        );
    }

    #[test]
    fn validates_azure_fields_and_url() {
        let mut profile = ProviderProfileInput {
            kind: ProviderKind::AzureAi,
            label: "Azure".to_owned(),
            profile_id: None,
            api_key: Some("key".to_owned()),
            api_base: Some("https://example.openai.azure.com".to_owned()),
            api_version: Some("2024-10-21".to_owned()),
            deployment: Some("gpt".to_owned()),
        };
        assert!(profile.validate().is_ok());

        profile.api_base = Some("http://example.com".to_owned());
        assert!(matches!(
            profile.validate(),
            Err(ProviderError::InvalidUrl(_))
        ));

        profile.api_base = Some("https://example.openai.azure.com".to_owned());
        profile.deployment = None;
        assert_eq!(
            profile.validate().expect_err("must fail"),
            ProviderError::MissingField("deployment")
        );
    }

    #[test]
    fn allows_local_http_for_openai_compatible() {
        let profile = ProviderProfileInput {
            kind: ProviderKind::OpenAICompatible,
            label: "Local".to_owned(),
            profile_id: Some("local-dev".to_owned()),
            api_key: None,
            api_base: Some("http://127.0.0.1:11434/v1".to_owned()),
            api_version: None,
            deployment: None,
        };
        assert!(profile.validate().is_ok());
        assert!(profile
            .to_selection()
            .expect("selection")
            .secret_ref
            .is_none());
    }

    #[test]
    fn copilot_uses_oauth_not_api_key() {
        let profile = ProviderProfileInput {
            kind: ProviderKind::GitHubCopilot,
            label: "Copilot".to_owned(),
            profile_id: None,
            api_key: Some("should-not-be-here".to_owned()),
            api_base: None,
            api_version: None,
            deployment: None,
        };
        assert_eq!(
            profile.validate().expect_err("must fail"),
            ProviderError::ApiKeyNotSupported
        );

        let mut profile = profile;
        profile.api_key = None;
        assert!(profile.validate().is_ok());
        assert!(profile
            .to_selection()
            .expect("selection")
            .secret_ref
            .is_none());
    }

    #[test]
    fn rejects_unsafe_profile_ids() {
        let mut profile = openrouter();
        profile.profile_id = Some("../escape".to_owned());
        assert!(matches!(
            profile.validate(),
            Err(ProviderError::InvalidProfileId(_))
        ));
    }

    #[test]
    fn exposes_env_vars_and_defaults() {
        assert_eq!(
            provider_env_var(ProviderKind::OpenRouter),
            Some("OPENROUTER_API_KEY")
        );
        assert_eq!(provider_env_var(ProviderKind::GitHubCopilot), None);
        assert_eq!(default_profile_id(ProviderKind::GitHubCopilot), "copilot");
        assert!(requires_api_key(ProviderKind::Anthropic));
        assert!(!requires_api_key(ProviderKind::GitHubCopilot));
    }
}
