//! Shared application state managed by Tauri.

use crate::paths::RuntimePaths;
use crate::setup_store::SetupStore;
use ffg_core::manifest::{RuntimePolicy, TrustedKey};
use ffg_runtime::RuntimeManager;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct AppState {
    pub paths: RuntimePaths,
    pub setup: Mutex<SetupStore>,
    pub runtime: RuntimeManager,
    pub operations: Mutex<HashMap<String, CancellationToken>>,
}

impl AppState {
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
            paths,
            runtime,
            operations: Mutex::new(HashMap::new()),
        })
    }
}
