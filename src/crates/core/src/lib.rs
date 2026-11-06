//! Pure, testable logic for the desktop app.
//!
//! This crate deliberately has no Tauri/Windows dependency so it can be tested on any platform.

pub mod checks;
pub mod error;
pub mod manifest;
pub mod models;
pub mod redaction;
pub mod rewrite;
pub mod secrets;
pub mod setup;
