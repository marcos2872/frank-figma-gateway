//! LiteLLM config store and management/test client.

pub mod client;
pub mod config_store;
pub mod copilot;
pub mod error;

pub use client::{LiteLlmClient, ProviderTestResult};
pub use config_store::{AppliedConfig, ConfigStore};
pub use copilot::{CopilotAuth, CopilotError, DeviceFlow, PollOutcome};
pub use error::LiteLlmError;
