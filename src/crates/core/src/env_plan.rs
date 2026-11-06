//! Pure planning of the Windows proxy environment variables.
//!
//! The actual registry writes and `WM_SETTINGCHANGE` broadcast belong to the
//! Windows adapter; this module computes the exact mutations and the snapshot
//! needed to restore the previous values.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const HTTPS_PROXY: &str = "HTTPS_PROXY";
pub const HTTP_PROXY: &str = "HTTP_PROXY";
pub const NO_PROXY: &str = "NO_PROXY";
pub const ALL_PROXY: &str = "ALL_PROXY";
pub const LOOPBACK_NO_PROXY: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnvSnapshot {
    #[serde(default)]
    pub https_proxy: Option<String>,
    #[serde(default)]
    pub http_proxy: Option<String>,
    #[serde(default)]
    pub no_proxy: Option<String>,
    #[serde(default)]
    pub all_proxy: Option<String>,
}

impl EnvSnapshot {
    pub fn is_empty(&self) -> bool {
        self.https_proxy.is_none()
            && self.http_proxy.is_none()
            && self.no_proxy.is_none()
            && self.all_proxy.is_none()
    }

    fn value_for(&self, name: &str) -> Option<&String> {
        match name {
            HTTPS_PROXY => self.https_proxy.as_ref(),
            HTTP_PROXY => self.http_proxy.as_ref(),
            NO_PROXY => self.no_proxy.as_ref(),
            ALL_PROXY => self.all_proxy.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyDesired {
    pub proxy_url: String,
    #[serde(default)]
    pub no_proxy: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum EnvMutation {
    Set { name: String, value: String },
    Remove { name: String },
}

impl EnvMutation {
    pub fn name(&self) -> &str {
        match self {
            EnvMutation::Set { name, .. } | EnvMutation::Remove { name } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEnvPlan {
    pub snapshot: EnvSnapshot,
    pub mutations: Vec<EnvMutation>,
    pub merged_no_proxy: String,
}

impl ProxyEnvPlan {
    /// Restores exactly the variables touched by this plan.
    pub fn restore_mutations(&self) -> Vec<EnvMutation> {
        self.mutations
            .iter()
            .map(|mutation| {
                let name = mutation.name();
                match self.snapshot.value_for(name) {
                    Some(value) => EnvMutation::Set {
                        name: name.to_owned(),
                        value: value.clone(),
                    },
                    None => EnvMutation::Remove {
                        name: name.to_owned(),
                    },
                }
            })
            .collect()
    }
}

pub fn plan_proxy_env(
    current: &EnvSnapshot,
    desired: &ProxyDesired,
) -> Result<ProxyEnvPlan, EnvError> {
    validate_proxy_url(&desired.proxy_url)?;
    let merged_no_proxy = merge_no_proxy(current.no_proxy.as_deref(), &desired.no_proxy)?;

    Ok(ProxyEnvPlan {
        snapshot: current.clone(),
        mutations: vec![
            EnvMutation::Set {
                name: HTTPS_PROXY.to_owned(),
                value: desired.proxy_url.clone(),
            },
            EnvMutation::Set {
                name: HTTP_PROXY.to_owned(),
                value: desired.proxy_url.clone(),
            },
            EnvMutation::Set {
                name: NO_PROXY.to_owned(),
                value: merged_no_proxy.clone(),
            },
        ],
        merged_no_proxy,
    })
}

pub fn merge_no_proxy(existing: Option<&str>, desired: &[String]) -> Result<String, EnvError> {
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();

    for entry in existing
        .into_iter()
        .flat_map(|value| value.split(','))
        .chain(desired.iter().map(String::as_str))
        .chain(LOOPBACK_NO_PROXY)
    {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if entry.chars().any(char::is_whitespace) {
            return Err(EnvError::InvalidNoProxyEntry(entry.to_owned()));
        }
        let key = entry.to_ascii_lowercase();
        if seen.insert(key) {
            entries.push(entry.to_owned());
        }
    }

    Ok(entries.join(","))
}

fn validate_proxy_url(value: &str) -> Result<(), EnvError> {
    let value = value.trim().trim_end_matches('/');
    let Some((host, port)) = value.rsplit_once(':') else {
        return Err(EnvError::InvalidProxyUrl(value.to_owned()));
    };
    let loopback =
        host == "http://127.0.0.1" || host == "http://localhost" || host == "http://[::1]";
    if !loopback {
        return Err(EnvError::InvalidProxyUrl(value.to_owned()));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| EnvError::InvalidProxyUrl(value.to_owned()))?;
    if port == 0 {
        return Err(EnvError::InvalidProxyUrl(value.to_owned()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvError {
    InvalidProxyUrl(String),
    InvalidNoProxyEntry(String),
}

impl fmt::Display for EnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvError::InvalidProxyUrl(url) => write!(f, "proxy deve ser loopback HTTP: {url}"),
            EnvError::InvalidNoProxyEntry(entry) => {
                write!(f, "entrada inválida em NO_PROXY: {entry}")
            }
        }
    }
}

impl std::error::Error for EnvError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn current() -> EnvSnapshot {
        EnvSnapshot {
            https_proxy: Some("http://corp-proxy:8080".to_owned()),
            http_proxy: None,
            no_proxy: Some("corp.local, localhost".to_owned()),
            all_proxy: Some("socks5://corp:1080".to_owned()),
        }
    }

    fn desired() -> ProxyDesired {
        ProxyDesired {
            proxy_url: "http://127.0.0.1:8787".to_owned(),
            no_proxy: vec!["api.figma.com".to_owned()],
        }
    }

    #[test]
    fn merges_existing_desired_and_loopback_entries() {
        let merged = merge_no_proxy(Some("corp.local, localhost"), &["api.figma.com".to_owned()])
            .expect("merge");
        assert_eq!(merged, "corp.local,localhost,api.figma.com,127.0.0.1,::1");
    }

    #[test]
    fn deduplicates_case_insensitively() {
        let merged = merge_no_proxy(
            Some("LOCALHOST,localhost"),
            &["localhost".to_owned(), "API.FIGMA.COM".to_owned()],
        )
        .expect("merge");
        assert_eq!(merged, "LOCALHOST,API.FIGMA.COM,127.0.0.1,::1");
    }

    #[test]
    fn rejects_entries_with_whitespace() {
        assert_eq!(
            merge_no_proxy(None, &["api figma".to_owned()]).expect_err("must fail"),
            EnvError::InvalidNoProxyEntry("api figma".to_owned())
        );
    }

    #[test]
    fn plans_the_three_proxy_variables_and_snapshots_the_rest() {
        let plan = plan_proxy_env(&current(), &desired()).expect("plan");
        assert_eq!(plan.snapshot, current());
        assert_eq!(
            plan.merged_no_proxy,
            "corp.local,localhost,api.figma.com,127.0.0.1,::1"
        );
        assert_eq!(plan.mutations.len(), 3);
        assert!(matches!(
            &plan.mutations[0],
            EnvMutation::Set { name, value }
                if name == HTTPS_PROXY && value == "http://127.0.0.1:8787"
        ));
        assert!(matches!(
            &plan.mutations[1],
            EnvMutation::Set { name, .. } if name == HTTP_PROXY
        ));
        assert!(matches!(
            &plan.mutations[2],
            EnvMutation::Set { name, .. } if name == NO_PROXY
        ));
    }

    #[test]
    fn restore_returns_exact_previous_values() {
        let plan = plan_proxy_env(&current(), &desired()).expect("plan");
        let restore = plan.restore_mutations();
        assert_eq!(
            restore,
            vec![
                EnvMutation::Set {
                    name: HTTPS_PROXY.to_owned(),
                    value: "http://corp-proxy:8080".to_owned(),
                },
                EnvMutation::Remove {
                    name: HTTP_PROXY.to_owned(),
                },
                EnvMutation::Set {
                    name: NO_PROXY.to_owned(),
                    value: "corp.local, localhost".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn validates_loopback_proxy_urls() {
        for url in [
            "http://127.0.0.1:8787",
            "http://localhost:8787/",
            "http://[::1]:8787",
        ] {
            assert!(
                plan_proxy_env(
                    &EnvSnapshot::default(),
                    &ProxyDesired {
                        proxy_url: url.to_owned(),
                        no_proxy: vec![],
                    }
                )
                .is_ok(),
                "{url}"
            );
        }

        for url in [
            "http://example.com:8787",
            "https://127.0.0.1:8787",
            "http://127.0.0.1",
            "http://127.0.0.1:0",
            "http://127.0.0.1:not-a-port",
        ] {
            assert!(
                plan_proxy_env(
                    &EnvSnapshot::default(),
                    &ProxyDesired {
                        proxy_url: url.to_owned(),
                        no_proxy: vec![],
                    }
                )
                .is_err(),
                "{url}"
            );
        }
    }

    #[test]
    fn snapshot_serializes_in_camel_case() {
        let json = serde_json::to_value(current()).expect("serializable");
        assert_eq!(json["httpsProxy"], "http://corp-proxy:8080");
        assert_eq!(json["noProxy"], "corp.local, localhost");
    }
}
