//! Deterministic LiteLLM `config.yaml` generation.
//!
//! The generated file never contains API keys: keys are referenced through
//! `os.environ/<VAR>` and Copilot uses `litellm_credential_name`.

use crate::models::is_valid_public_name;
use crate::setup::{ModelRoute, ModelSettings};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const MASTER_KEY_ENV: &str = "LITELLM_MASTER_KEY";
pub const DEFAULT_NUM_RETRIES: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LiteLlmConfig {
    model_list: Vec<LiteLlmModel>,
    router_settings: RouterSettings,
    general_settings: GeneralSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LiteLlmModel {
    model_name: String,
    litellm_params: LiteLlmParams,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LiteLlmParams {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    litellm_credential_name: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RouterSettings {
    fallbacks: Vec<BTreeMap<String, Vec<String>>>,
    num_retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct GeneralSettings {
    master_key: String,
}

const RESERVED_EXTRA_KEYS: [&str; 5] = [
    "model",
    "api_key",
    "api_base",
    "api_version",
    "litellm_credential_name",
];

pub fn generate_litellm_config(settings: &ModelSettings) -> Result<String, ConfigError> {
    validate_model_settings(settings)?;

    let model_list = settings
        .routes
        .iter()
        .map(|route| LiteLlmModel {
            model_name: route.public_name.clone(),
            litellm_params: LiteLlmParams {
                model: route.provider_model.clone(),
                api_key: route
                    .api_key_env
                    .as_ref()
                    .map(|env| format!("os.environ/{env}")),
                api_base: route.api_base.clone(),
                api_version: route.api_version.clone(),
                litellm_credential_name: route.credential_name.clone(),
                extra: route.extra.clone(),
            },
        })
        .collect();

    let fallbacks = settings
        .fallbacks
        .iter()
        .map(|(from, to)| {
            let mut entry = BTreeMap::new();
            entry.insert(from.clone(), to.clone());
            entry
        })
        .collect();

    let config = LiteLlmConfig {
        model_list,
        router_settings: RouterSettings {
            fallbacks,
            num_retries: DEFAULT_NUM_RETRIES,
        },
        general_settings: GeneralSettings {
            master_key: format!("os.environ/{MASTER_KEY_ENV}"),
        },
    };

    let yaml = serde_yaml::to_string(&config)
        .map_err(|error| ConfigError::Serialize(error.to_string()))?;
    serde_yaml::from_str::<serde_yaml::Value>(&yaml)
        .map_err(|error| ConfigError::InvalidYaml(error.to_string()))?;
    Ok(yaml)
}

pub fn validate_model_settings(settings: &ModelSettings) -> Result<(), ConfigError> {
    if settings.routes.is_empty() {
        return Err(ConfigError::EmptyRoutes);
    }

    let mut names = BTreeSet::new();
    for route in &settings.routes {
        validate_route(route)?;
        if !names.insert(route.public_name.clone()) {
            return Err(ConfigError::DuplicateRoute(route.public_name.clone()));
        }
    }

    validate_fallbacks(settings, &names)
}

fn validate_route(route: &ModelRoute) -> Result<(), ConfigError> {
    if !is_valid_public_name(&route.public_name) {
        return Err(ConfigError::InvalidPublicName(route.public_name.clone()));
    }
    if route.provider_model.trim().is_empty() {
        return Err(ConfigError::MissingProviderModel(route.public_name.clone()));
    }
    if route.credential_name.is_some() && route.api_key_env.is_some() {
        return Err(ConfigError::ConflictingCredentials(
            route.public_name.clone(),
        ));
    }
    if route.provider_model.starts_with("github_copilot/") && route.credential_name.is_none() {
        return Err(ConfigError::MissingCredential(route.public_name.clone()));
    }
    if let Some(env) = &route.api_key_env {
        if !is_env_name(env) {
            return Err(ConfigError::InvalidEnvName(env.clone()));
        }
    }
    for key in route.extra.keys() {
        if RESERVED_EXTRA_KEYS.contains(&key.as_str()) {
            return Err(ConfigError::ReservedExtra(key.clone()));
        }
    }
    Ok(())
}

fn validate_fallbacks(
    settings: &ModelSettings,
    names: &BTreeSet<String>,
) -> Result<(), ConfigError> {
    for (from, targets) in &settings.fallbacks {
        if !names.contains(from) {
            return Err(ConfigError::InvalidFallback(from.clone()));
        }
        if targets.is_empty() {
            return Err(ConfigError::InvalidFallback(from.clone()));
        }
        for target in targets {
            if !names.contains(target) {
                return Err(ConfigError::InvalidFallback(target.clone()));
            }
            if target == from {
                return Err(ConfigError::FallbackCycle(from.clone()));
            }
        }
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for name in names {
        visit_fallback(name, settings, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit_fallback(
    name: &str,
    settings: &ModelSettings,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<(), ConfigError> {
    if visited.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_owned()) {
        return Err(ConfigError::FallbackCycle(name.to_owned()));
    }
    if let Some(targets) = settings.fallbacks.get(name) {
        for target in targets {
            visit_fallback(target, settings, visiting, visited)?;
        }
    }
    visiting.remove(name);
    visited.insert(name.to_owned());
    Ok(())
}

fn is_env_name(value: &str) -> bool {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) if first.is_ascii_uppercase() => {}
        _ => return false,
    }
    characters.all(|character| {
        character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    EmptyRoutes,
    InvalidPublicName(String),
    DuplicateRoute(String),
    MissingProviderModel(String),
    ConflictingCredentials(String),
    MissingCredential(String),
    InvalidEnvName(String),
    ReservedExtra(String),
    InvalidFallback(String),
    FallbackCycle(String),
    Serialize(String),
    InvalidYaml(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::EmptyRoutes => f.write_str("nenhuma rota de modelo configurada"),
            ConfigError::InvalidPublicName(name) => {
                write!(f, "nome público de modelo inválido: {name}")
            }
            ConfigError::DuplicateRoute(name) => write!(f, "rota duplicada: {name}"),
            ConfigError::MissingProviderModel(name) => {
                write!(f, "modelo do provider ausente para {name}")
            }
            ConfigError::ConflictingCredentials(name) => {
                write!(f, "rota {name} define credencial e API key ao mesmo tempo")
            }
            ConfigError::MissingCredential(name) => {
                write!(f, "rota {name} do Copilot exige litellm_credential_name")
            }
            ConfigError::InvalidEnvName(name) => write!(f, "nome de variável inválido: {name}"),
            ConfigError::ReservedExtra(key) => {
                write!(f, "chave reservada em litellm_params: {key}")
            }
            ConfigError::InvalidFallback(name) => write!(f, "fallback inválido: {name}"),
            ConfigError::FallbackCycle(name) => write!(f, "ciclo de fallback em {name}"),
            ConfigError::Serialize(detail) => write!(f, "falha ao serializar config: {detail}"),
            ConfigError::InvalidYaml(detail) => write!(f, "YAML gerado inválido: {detail}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    fn route(public_name: &str, provider_model: &str) -> ModelRoute {
        ModelRoute {
            public_name: public_name.to_owned(),
            provider_model: provider_model.to_owned(),
            credential_name: None,
            api_key_env: Some("OPENROUTER_API_KEY".to_owned()),
            api_base: None,
            api_version: None,
            extra: BTreeMap::new(),
        }
    }

    fn base_settings() -> ModelSettings {
        ModelSettings {
            routes: vec![
                ModelRoute {
                    public_name: "claude-opus-4-8".to_owned(),
                    provider_model: "github_copilot/claude-sonnet-5.5".to_owned(),
                    credential_name: Some("github-copilot".to_owned()),
                    api_key_env: None,
                    api_base: None,
                    api_version: None,
                    extra: BTreeMap::new(),
                },
                route("claude-haiku-4-5", "openrouter/anthropic/claude-haiku-4.5"),
            ],
            fallbacks: BTreeMap::from([(
                "claude-opus-4-8".to_owned(),
                vec!["claude-haiku-4-5".to_owned()],
            )]),
            applied_at: None,
            config_revision: None,
        }
    }

    #[test]
    fn generates_the_expected_yaml_shape() {
        let yaml = generate_litellm_config(&base_settings()).expect("config");
        assert!(!yaml.contains("sk-"));
        assert!(!yaml.contains("api_key: sk"));

        let fixture: Value =
            serde_yaml::from_str(include_str!("../tests/fixtures/litellm-config.yaml"))
                .expect("fixture");
        let generated: Value = serde_yaml::from_str(&yaml).expect("generated");
        assert_eq!(generated, fixture);
    }

    #[test]
    fn rejects_empty_routes() {
        assert_eq!(
            generate_litellm_config(&ModelSettings::default()).expect_err("must fail"),
            ConfigError::EmptyRoutes
        );
    }

    #[test]
    fn rejects_invalid_public_names_and_duplicates() {
        let mut settings = base_settings();
        settings.routes[0].public_name = "gpt-5".to_owned();
        assert!(matches!(
            generate_litellm_config(&settings),
            Err(ConfigError::InvalidPublicName(_))
        ));

        let mut settings = base_settings();
        settings.routes[1].public_name = "claude-opus-4-8".to_owned();
        assert_eq!(
            generate_litellm_config(&settings).expect_err("must fail"),
            ConfigError::DuplicateRoute("claude-opus-4-8".to_owned())
        );
    }

    #[test]
    fn rejects_missing_provider_model_and_credentials() {
        let mut settings = base_settings();
        settings.routes[0].provider_model = " ".to_owned();
        assert!(matches!(
            generate_litellm_config(&settings),
            Err(ConfigError::MissingProviderModel(_))
        ));

        let mut settings = base_settings();
        settings.routes[0].credential_name = None;
        assert_eq!(
            generate_litellm_config(&settings).expect_err("must fail"),
            ConfigError::MissingCredential("claude-opus-4-8".to_owned())
        );

        let mut settings = base_settings();
        settings.routes[0].credential_name = None;
        settings.routes[0].provider_model = "openrouter/anthropic/claude-4".to_owned();
        settings.routes[0].api_key_env = Some("OPENROUTER_API_KEY".to_owned());
        settings.routes[0].credential_name = Some("github-copilot".to_owned());
        assert_eq!(
            generate_litellm_config(&settings).expect_err("must fail"),
            ConfigError::ConflictingCredentials("claude-opus-4-8".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_env_and_reserved_extra_keys() {
        let mut settings = base_settings();
        settings.routes[1].api_key_env = Some("openrouter-key".to_owned());
        assert_eq!(
            generate_litellm_config(&settings).expect_err("must fail"),
            ConfigError::InvalidEnvName("openrouter-key".to_owned())
        );

        let mut settings = base_settings();
        settings.routes[1]
            .extra
            .insert("api_key".to_owned(), "sk-literal".to_owned());
        assert_eq!(
            generate_litellm_config(&settings).expect_err("must fail"),
            ConfigError::ReservedExtra("api_key".to_owned())
        );
    }

    #[test]
    fn rejects_unknown_and_cyclic_fallbacks() {
        let mut settings = base_settings();
        settings
            .fallbacks
            .insert("claude-opus-4-8".to_owned(), vec!["missing".to_owned()]);
        assert_eq!(
            generate_litellm_config(&settings).expect_err("must fail"),
            ConfigError::InvalidFallback("missing".to_owned())
        );

        let mut settings = base_settings();
        settings.fallbacks.insert(
            "claude-haiku-4-5".to_owned(),
            vec!["claude-opus-4-8".to_owned()],
        );
        assert!(matches!(
            generate_litellm_config(&settings),
            Err(ConfigError::FallbackCycle(_))
        ));
    }

    #[test]
    fn rejects_empty_fallback_targets() {
        let mut settings = base_settings();
        settings
            .fallbacks
            .insert("claude-opus-4-8".to_owned(), vec![]);
        assert_eq!(
            generate_litellm_config(&settings).expect_err("must fail"),
            ConfigError::InvalidFallback("claude-opus-4-8".to_owned())
        );
    }
}
