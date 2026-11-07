//! Environment checks for the onboarding welcome step.

use crate::figma;
use crate::state::AppState;
use ffg_core::checks::{
    disk_requirement_with_margin, is_supported_platform, summarize, CheckId, CheckStatus,
    EnvironmentCheck, EnvironmentReport, FigmaDetection, ProxyEnvironment,
};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tauri::State;

#[tauri::command]
pub fn environment_report(state: State<'_, AppState>) -> EnvironmentReport {
    let os = std::env::consts::OS.to_owned();
    let arch = std::env::consts::ARCH.to_owned();
    let supported = is_supported_platform(&os, &arch);
    let data_dir = state.paths.root().display().to_string();
    let writable = std::fs::write(state.paths.state_dir().join(".write-test"), b"ok")
        .and_then(|_| std::fs::remove_file(state.paths.state_dir().join(".write-test")))
        .is_ok();
    let free_disk = fs2::available_space(state.paths.root()).unwrap_or(0);
    let figma = figma::detect();
    let proxy = state.env.snapshot().unwrap_or_default();

    let mut checks = Vec::new();
    checks.push(check(
        CheckId::Platform,
        if supported {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        if supported {
            "Windows x64 detectado"
        } else {
            "Esta versão exige Windows x64"
        },
    ));
    checks.push(check(
        CheckId::DataDir,
        if writable {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        if writable {
            "Pasta de dados gravável"
        } else {
            "Sem permissão na pasta de dados"
        },
    ));
    checks.push(check(
        CheckId::Disk,
        if free_disk > disk_requirement_with_margin(0) {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        "Espaço em disco disponível",
    ));
    checks.push(check(
        CheckId::Internet,
        if can_connect() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        "Conectividade com a internet",
    ));
    checks.push(check(
        CheckId::Figma,
        if figma.detected {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        if figma.detected {
            "Figma instalado"
        } else {
            "Figma não encontrado"
        },
    ));
    if !proxy.is_empty() {
        checks.push(check(
            CheckId::ExistingProxy,
            CheckStatus::Warn,
            "Já existe um proxy configurado no ambiente",
        ));
    }
    let (blocking, warnings) = summarize(&checks);

    EnvironmentReport {
        os,
        arch,
        supported,
        data_dir,
        free_disk_bytes: free_disk,
        required_disk_bytes: 0,
        checks,
        figma: FigmaDetection {
            found: figma.detected,
            path: figma.path,
            version: figma.version,
            running: figma.running,
        },
        existing_proxy: ProxyEnvironment {
            https_proxy: proxy.https_proxy,
            http_proxy: proxy.http_proxy,
            no_proxy: proxy.no_proxy,
        },
        ports: Vec::new(),
        blocking,
        warnings,
    }
}

fn check(id: CheckId, status: CheckStatus, message: &str) -> EnvironmentCheck {
    EnvironmentCheck::new(id, status, message)
}

fn can_connect() -> bool {
    let Ok(mut addresses) = "github.com:443".to_socket_addrs() else {
        return false;
    };
    addresses.any(|address| TcpStream::connect_timeout(&address, Duration::from_secs(2)).is_ok())
}
