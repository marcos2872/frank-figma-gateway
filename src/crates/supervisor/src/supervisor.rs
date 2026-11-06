//! Sidecar process supervision with health checks, logs and restart policy.

use crate::backoff::Backoff;
use crate::health::{check_health, HealthCheck, HealthState};
use crate::job::JobHandle;
use crate::logs::{read_stream, LogBuffer, LogLine, LogStream};
use ffg_core::error::{ErrorCode, UiError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_MAX_RESTARTS: u32 = 5;
pub const MONITOR_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceName {
    Postgres,
    Litellm,
    Proxy,
}

impl ServiceName {
    pub const ALL: [ServiceName; 3] = [
        ServiceName::Postgres,
        ServiceName::Litellm,
        ServiceName::Proxy,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ServiceName::Postgres => "postgres",
            ServiceName::Litellm => "litellm",
            ServiceName::Proxy => "proxy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Degraded,
    Failed,
    Stopping,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub name: ServiceName,
    pub state: ServiceState,
    pub detail: Option<String>,
    pub port: Option<u16>,
    pub health: HealthState,
    pub restart_count: u32,
    pub last_error: Option<UiError>,
}

#[derive(Debug, Clone)]
pub struct ServiceSpec {
    pub name: ServiceName,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub port: Option<u16>,
    pub health: HealthCheck,
}

pub struct Supervisor {
    services: Mutex<HashMap<ServiceName, Managed>>,
    client: reqwest::Client,
    backoff: Backoff,
    max_restarts: u32,
}

struct Managed {
    spec: ServiceSpec,
    state: ServiceState,
    child: Option<Child>,
    job: Option<JobHandle>,
    logs: Arc<StdMutex<LogBuffer>>,
    health: HealthState,
    detail: Option<String>,
    restart_count: u32,
    last_error: Option<UiError>,
    desired_running: bool,
    next_restart: Option<Instant>,
}

impl Managed {
    fn new(spec: ServiceSpec) -> Self {
        Self {
            spec,
            state: ServiceState::Stopped,
            child: None,
            job: None,
            logs: Arc::new(StdMutex::new(LogBuffer::default())),
            health: HealthState::Unknown,
            detail: None,
            restart_count: 0,
            last_error: None,
            desired_running: false,
            next_restart: None,
        }
    }

    fn status(&self) -> ServiceStatus {
        ServiceStatus {
            name: self.spec.name,
            state: self.state,
            detail: self.detail.clone(),
            port: self.spec.port.or_else(|| self.spec.health.port()),
            health: self.health,
            restart_count: self.restart_count,
            last_error: self.last_error.clone(),
        }
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new(Backoff::default(), DEFAULT_MAX_RESTARTS)
    }
}

impl Supervisor {
    pub fn new(backoff: Backoff, max_restarts: u32) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("frank-figma-gateway")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            services: Mutex::new(HashMap::new()),
            client,
            backoff,
            max_restarts,
        }
    }

    pub async fn register(&self, spec: ServiceSpec) {
        let mut services = self.services.lock().await;
        match services.get_mut(&spec.name) {
            Some(managed) if managed.child.is_none() => managed.spec = spec,
            Some(_) => {}
            None => {
                services.insert(spec.name, Managed::new(spec));
            }
        }
    }

    pub async fn status_all(&self) -> Vec<ServiceStatus> {
        let services = self.services.lock().await;
        ServiceName::ALL
            .into_iter()
            .map(|name| {
                services
                    .get(&name)
                    .map(Managed::status)
                    .unwrap_or_else(|| default_status(name))
            })
            .collect()
    }

    pub async fn status(&self, name: ServiceName) -> Result<ServiceStatus, SupervisorError> {
        let services = self.services.lock().await;
        services
            .get(&name)
            .map(Managed::status)
            .ok_or(SupervisorError::UnknownService(name))
    }

    pub async fn start(&self, name: ServiceName) -> Result<ServiceStatus, SupervisorError> {
        let mut services = self.services.lock().await;
        let managed = services
            .get_mut(&name)
            .ok_or(SupervisorError::UnknownService(name))?;

        if managed.desired_running && managed.child.is_some() {
            return Ok(managed.status());
        }

        let (child, job, logs) = spawn_child(&managed.spec)?;
        managed.child = Some(child);
        managed.job = Some(job);
        managed.logs = logs;
        managed.state = if matches!(managed.spec.health, HealthCheck::None) {
            ServiceState::Running
        } else {
            ServiceState::Starting
        };
        managed.health = HealthState::Unknown;
        managed.detail = None;
        managed.last_error = None;
        managed.desired_running = true;
        managed.restart_count = 0;
        managed.next_restart = None;
        Ok(managed.status())
    }

    pub async fn start_all(&self) -> Result<Vec<ServiceStatus>, SupervisorError> {
        let names: Vec<ServiceName> = {
            let services = self.services.lock().await;
            ServiceName::ALL
                .into_iter()
                .filter(|name| services.contains_key(name))
                .collect()
        };
        for name in names {
            self.start(name).await?;
        }
        Ok(self.status_all().await)
    }

    pub async fn stop(&self, name: ServiceName) -> Result<ServiceStatus, SupervisorError> {
        let (child, _job) = {
            let mut services = self.services.lock().await;
            let managed = services
                .get_mut(&name)
                .ok_or(SupervisorError::UnknownService(name))?;
            managed.desired_running = false;
            managed.next_restart = None;
            managed.state = ServiceState::Stopping;
            (managed.child.take(), managed.job.take())
        };

        if let Some(mut child) = child {
            let _ = child.start_kill();
            if tokio::time::timeout(Duration::from_secs(10), child.wait())
                .await
                .is_err()
            {
                let _ = child.kill().await;
            }
        }

        let mut services = self.services.lock().await;
        let managed = services
            .get_mut(&name)
            .ok_or(SupervisorError::UnknownService(name))?;
        managed.state = ServiceState::Stopped;
        managed.health = HealthState::Unknown;
        managed.detail = Some("parado pelo usuário".to_owned());
        managed.restart_count = 0;
        Ok(managed.status())
    }

    pub async fn stop_all(&self) -> Result<Vec<ServiceStatus>, SupervisorError> {
        for name in ServiceName::ALL {
            if self.services.lock().await.get(&name).is_some() {
                self.stop(name).await?;
            }
        }
        Ok(self.status_all().await)
    }

    pub async fn restart(&self, name: ServiceName) -> Result<ServiceStatus, SupervisorError> {
        self.stop(name).await?;
        self.start(name).await
    }

    pub async fn logs(
        &self,
        name: ServiceName,
        limit: usize,
    ) -> Result<Vec<LogLine>, SupervisorError> {
        let services = self.services.lock().await;
        let managed = services
            .get(&name)
            .ok_or(SupervisorError::UnknownService(name))?;
        let logs = managed
            .logs
            .lock()
            .map_err(|_| SupervisorError::State("log buffer envenenado".to_owned()))?;
        Ok(logs.tail(limit))
    }

    pub async fn tick(&self) -> Result<(), SupervisorError> {
        let mut restart = Vec::new();
        {
            let mut services = self.services.lock().await;
            for (name, managed) in services.iter_mut() {
                if let Some(child) = managed.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            managed.child = None;
                            managed.job = None;
                            managed.health = HealthState::Unknown;
                            if managed.desired_running && managed.restart_count < self.max_restarts
                            {
                                managed.restart_count += 1;
                                managed.state = ServiceState::Starting;
                                managed.detail = Some(format!("reiniciando após saída {status}"));
                                managed.next_restart = Some(
                                    Instant::now() + self.backoff.delay(managed.restart_count),
                                );
                            } else {
                                managed.state = if managed.desired_running {
                                    ServiceState::Failed
                                } else {
                                    ServiceState::Stopped
                                };
                                managed.desired_running = false;
                                managed.last_error = Some(
                                    UiError::new(
                                        ErrorCode::ProcessExited,
                                        format!("processo terminou com {status}"),
                                    )
                                    .with_detail(managed.spec.name.as_str()),
                                );
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            managed.child = None;
                            managed.job = None;
                            managed.desired_running = false;
                            managed.state = ServiceState::Failed;
                            managed.last_error = Some(
                                UiError::new(
                                    ErrorCode::ProcessExited,
                                    "não foi possível consultar o processo",
                                )
                                .with_detail(error.to_string()),
                            );
                        }
                    }
                }

                if managed.child.is_none()
                    && managed.desired_running
                    && managed.next_restart.is_some_and(|at| Instant::now() >= at)
                {
                    restart.push((*name, managed.spec.clone()));
                }
            }
        }

        for (name, spec) in restart {
            self.spawn_registered(name, spec).await?;
        }

        let checks: Vec<(ServiceName, HealthCheck)> = {
            let services = self.services.lock().await;
            services
                .iter()
                .filter(|(_, managed)| {
                    managed.child.is_some()
                        && managed.desired_running
                        && !matches!(managed.spec.health, HealthCheck::None)
                })
                .map(|(name, managed)| (*name, managed.spec.health.clone()))
                .collect()
        };

        for (name, health) in checks {
            let (state, detail) = check_health(&self.client, &health).await;
            let mut services = self.services.lock().await;
            if let Some(managed) = services.get_mut(&name) {
                if managed.child.is_some() {
                    managed.health = state;
                    if state == HealthState::Ok {
                        managed.state = ServiceState::Running;
                        managed.detail = None;
                        managed.last_error = None;
                    } else {
                        managed.state = ServiceState::Degraded;
                        managed.detail = detail;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn monitor(&self, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(MONITOR_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    if let Err(error) = self.tick().await {
                        log::warn!("supervisor tick falhou: {error}");
                    }
                }
            }
        }
    }

    async fn spawn_registered(
        &self,
        name: ServiceName,
        spec: ServiceSpec,
    ) -> Result<(), SupervisorError> {
        let (child, job, logs) = spawn_child(&spec)?;
        let mut services = self.services.lock().await;
        if let Some(managed) = services.get_mut(&name) {
            managed.child = Some(child);
            managed.job = Some(job);
            managed.logs = logs;
            managed.state = if matches!(managed.spec.health, HealthCheck::None) {
                ServiceState::Running
            } else {
                ServiceState::Starting
            };
            managed.health = HealthState::Unknown;
            managed.detail = None;
            managed.next_restart = None;
        }
        Ok(())
    }
}

fn spawn_child(
    spec: &ServiceSpec,
) -> Result<(Child, JobHandle, Arc<StdMutex<LogBuffer>>), SupervisorError> {
    let job = JobHandle::new().map_err(|error| SupervisorError::Job(error.to_string()))?;

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .envs(&spec.env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }

    let mut child = command.spawn()?;
    job.assign(&child)
        .map_err(|error| SupervisorError::Job(error.to_string()))?;

    let logs = Arc::new(StdMutex::new(LogBuffer::default()));
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(read_stream(stdout, LogStream::Stdout, Arc::clone(&logs)));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(read_stream(stderr, LogStream::Stderr, Arc::clone(&logs)));
    }

    Ok((child, job, logs))
}

fn default_status(name: ServiceName) -> ServiceStatus {
    ServiceStatus {
        name,
        state: ServiceState::Stopped,
        detail: match name {
            ServiceName::Postgres => Some("necessário no perfil Copilot".to_owned()),
            ServiceName::Litellm => None,
            ServiceName::Proxy => Some("processo interno".to_owned()),
        },
        port: None,
        health: HealthState::Unknown,
        restart_count: 0,
        last_error: None,
    }
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("serviço desconhecido: {0:?}")]
    UnknownService(ServiceName),

    #[error("falha ao iniciar processo: {0}")]
    Spawn(#[from] std::io::Error),

    #[error("job object: {0}")]
    Job(String),

    #[error("estado do supervisor: {0}")]
    State(String),
}

impl SupervisorError {
    pub fn to_ui_error(&self) -> UiError {
        match self {
            SupervisorError::UnknownService(name) => UiError::new(
                ErrorCode::Internal,
                format!("Serviço desconhecido: {}", name.as_str()),
            ),
            SupervisorError::Spawn(error) => UiError::new(
                ErrorCode::ProcessSpawn,
                "Não foi possível iniciar o serviço.",
            )
            .with_detail(error.to_string()),
            SupervisorError::Job(detail) => UiError::new(
                ErrorCode::ProcessSpawn,
                "Não foi possível isolar o processo filho.",
            )
            .with_detail(detail.clone()),
            SupervisorError::State(detail) => {
                UiError::new(ErrorCode::Internal, "Estado do supervisor inválido.")
                    .with_detail(detail.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: ServiceName, script: &str) -> ServiceSpec {
        ServiceSpec {
            name,
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_owned(), script.to_owned()],
            env: BTreeMap::new(),
            cwd: None,
            port: None,
            health: HealthCheck::None,
        }
    }

    #[tokio::test]
    async fn status_all_includes_defaults() {
        let supervisor = Supervisor::default();
        let statuses = supervisor.status_all().await;
        assert_eq!(statuses.len(), 3);
        assert!(statuses
            .iter()
            .all(|status| status.state == ServiceState::Stopped));
        assert_eq!(statuses[2].name, ServiceName::Proxy);
    }

    #[tokio::test]
    async fn unknown_service_is_rejected() {
        let supervisor = Supervisor::default();
        let error = supervisor
            .start(ServiceName::Litellm)
            .await
            .expect_err("must fail");
        assert!(matches!(error, SupervisorError::UnknownService(_)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn starts_and_stops_a_process() {
        let supervisor = Supervisor::default();
        supervisor
            .register(spec(ServiceName::Litellm, "sleep 5"))
            .await;

        let status = supervisor.start(ServiceName::Litellm).await.expect("start");
        assert_eq!(status.state, ServiceState::Running);
        assert!(
            supervisor
                .status(ServiceName::Litellm)
                .await
                .expect("status")
                .state
                == ServiceState::Running
        );

        let status = supervisor.stop(ServiceName::Litellm).await.expect("stop");
        assert_eq!(status.state, ServiceState::Stopped);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn captures_process_logs() {
        let supervisor = Supervisor::default();
        supervisor
            .register(spec(ServiceName::Litellm, "echo hello-from-child"))
            .await;
        supervisor.start(ServiceName::Litellm).await.expect("start");
        tokio::time::sleep(Duration::from_millis(200)).await;
        let logs = supervisor
            .logs(ServiceName::Litellm, 10)
            .await
            .expect("logs");
        assert!(logs
            .iter()
            .any(|line| line.line.contains("hello-from-child")));
        let _ = supervisor.stop(ServiceName::Litellm).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn monitor_restarts_a_crashed_process() {
        let supervisor = Supervisor::new(
            Backoff {
                initial: Duration::from_millis(10),
                max: Duration::from_millis(10),
                factor: 1,
            },
            1,
        );
        supervisor
            .register(spec(ServiceName::Litellm, "exit 1"))
            .await;
        supervisor.start(ServiceName::Litellm).await.expect("start");
        tokio::time::sleep(Duration::from_millis(100)).await;
        supervisor.tick().await.expect("tick");
        assert!(
            supervisor
                .status(ServiceName::Litellm)
                .await
                .expect("status")
                .restart_count
                >= 1
        );
    }
}
