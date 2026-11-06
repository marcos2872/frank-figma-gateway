use ffg_core::error::UiError;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub models: Vec<String>,
    pub proxy_prefix: String,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "frank-figma-gateway".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        models: ffg_core::models::default_models()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        proxy_prefix: ffg_core::rewrite::AI_GATEWAY_PREFIX.to_owned(),
    }
}

#[tauri::command]
pub fn preview_rewrite(path: String) -> Option<String> {
    ffg_core::rewrite::rewrite_ai_path(&path)
}

/// Abre uma URL no navegador do sistema (usado pelo device flow do Copilot).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), UiError> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(UiError::internal("Só é permitido abrir URLs HTTP(S)."));
    }
    opener(&url).spawn().map_err(|error| {
        UiError::internal("Não foi possível abrir o navegador.").with_detail(error.to_string())
    })?;
    Ok(())
}

#[cfg(windows)]
fn opener(url: &str) -> std::process::Command {
    let mut command = std::process::Command::new("cmd");
    command.args(["/C", "start", "", url]);
    command
}

#[cfg(target_os = "macos")]
fn opener(url: &str) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    command.arg(url);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn opener(url: &str) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(url);
    command
}
