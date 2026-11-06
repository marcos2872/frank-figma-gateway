//! Shared application state managed by Tauri.

use crate::paths::RuntimePaths;
use crate::setup_store::SetupStore;
use std::sync::Mutex;

pub struct AppState {
    pub paths: RuntimePaths,
    pub setup: Mutex<SetupStore>,
}

impl AppState {
    pub fn new(paths: RuntimePaths) -> Self {
        Self {
            setup: Mutex::new(SetupStore::new(paths.clone())),
            paths,
        }
    }
}
