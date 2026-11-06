//! Pure, testable logic for the desktop app.
//!
//! This crate deliberately has no Tauri/Windows dependency so it can be tested on any platform.

pub mod checks;
pub mod copilot;
pub mod download;
pub mod env_plan;
pub mod error;
pub mod install;
pub mod integration;
pub mod litellm_config;
pub mod manifest;
pub mod models;
pub mod providers;
pub mod redaction;
pub mod rewrite;
pub mod secrets;
pub mod setup;
