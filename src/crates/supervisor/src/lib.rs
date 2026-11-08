//! Sidecar supervision, health checks, ports and logs.

pub mod backoff;
pub mod health;
mod job;
pub mod logs;
pub mod ports;
pub mod supervisor;

pub use backoff::Backoff;
pub use health::{HealthCheck, HealthState};
pub use logs::{LogBuffer, LogLine, LogStream};
pub use ports::allocate as allocate_port;
pub use supervisor::{
    ServiceName, ServiceSpec, ServiceState, ServiceStatus, Supervisor, SupervisorError,
};
