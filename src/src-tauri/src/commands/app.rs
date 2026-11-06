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
