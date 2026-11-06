use log::LevelFilter;
use tauri_plugin_log::{Target, TargetKind};

/// Builds the Tauri log plugin. Level comes from `FFG_LOG_LEVEL` (default: info).
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let level = std::env::var("FFG_LOG_LEVEL")
        .ok()
        .and_then(|value| value.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info);

    tauri_plugin_log::Builder::new()
        .level(level)
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some("frank-figma-gateway".to_owned()),
            }),
        ])
        .build()
}
