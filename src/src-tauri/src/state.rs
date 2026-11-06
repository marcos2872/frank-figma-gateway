//! Shared application state managed by Tauri.

use crate::ca::CaManager;
use crate::paths::RuntimePaths;
use crate::platform::{EnvStore, PlatformCertStore, PlatformEnvStore, PlatformFileProtector};
use crate::secrets::PlatformSecretStore;
use crate::setup_store::SetupStore;
use ffg_core::manifest::{RuntimePolicy, TrustedKey};
use ffg_core::secrets::SecretStore;
use ffg_litellm::ConfigStore;
use ffg_proxy::proxy::ProxyHandle;
use ffg_runtime::RuntimeManager;
use ffg_supervisor::Supervisor;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

pub struct AppState {
    pub paths: RuntimePaths,
    pub setup: Mutex<SetupStore>,
    pub runtime: RuntimeManager,
    pub supervisor: Arc<Supervisor>,
    pub secrets: Arc<dyn SecretStore>,
    pub litellm: ConfigStore,
    pub copilot: ffg_litellm::CopilotAuth,
    pub ca: Arc<CaManager>,
    pub env: Arc<dyn EnvStore>,
    pub proxy: Mutex<Option<ProxyHandle>>,
    pub operations: Mutex<HashMap<String, CancellationToken>>,
}

impl AppState {
    #[allow(clippy::default_constructed_unit_structs)]
    pub fn new(paths: RuntimePaths) -> Result<Self, Box<dyn std::error::Error>> {
        let keys: Vec<TrustedKey> =
            serde_json::from_str(include_str!("../resources/keys/manifest-keys.json"))?;
        let policy: RuntimePolicy =
            serde_json::from_str(include_str!("../resources/runtime-policy.json"))?;
        let runtime = RuntimeManager::new(
            paths.root().to_path_buf(),
            keys,
            policy,
            env!("CARGO_PKG_VERSION").to_owned(),
        )?;

        Ok(Self {
            setup: Mutex::new(SetupStore::new(paths.clone())),
            paths: paths.clone(),
            runtime,
            supervisor: Arc::new(Supervisor::default()),
            secrets: Arc::new(PlatformSecretStore::default()),
            litellm: ConfigStore::new(paths.litellm_config_dir())?,
            copilot: ffg_litellm::CopilotAuth::from_env(),
            ca: Arc::new(CaManager::new(
                paths.clone(),
                Arc::new(PlatformCertStore::default()),
                Arc::new(PlatformFileProtector::default()),
            )),
            env: Arc::new(PlatformEnvStore::default()),
            proxy: Mutex::new(None),
            operations: Mutex::new(HashMap::new()),
        })
    }
}
