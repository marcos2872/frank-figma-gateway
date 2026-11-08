use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Emitter, Manager};

pub fn build(app: &App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir dashboard", true, None::<&str>)?;
    let reconfigure = MenuItem::with_id(app, "reconfigure", "Reconfigurar", true, None::<&str>)?;
    let figma = MenuItem::with_id(app, "figma", "Configurar Figma", true, None::<&str>)?;
    let litellm = MenuItem::with_id(app, "litellm", "Abrir LiteLLM", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, "logs", "Logs", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &reconfigure, &figma, &litellm, &logs, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" | "litellm" | "logs" => show_main(app),
            "reconfigure" => open_wizard(app, "reconfigure"),
            "figma" => open_wizard(app, "figmaOnly"),
            "quit" => app.exit(0),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon((*icon).clone());
    }

    builder.build(app)?;
    Ok(())
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[derive(Clone, Serialize)]
struct WizardOpenRequest {
    mode: &'static str,
}

fn open_wizard(app: &AppHandle, mode: &'static str) {
    let _ = app.emit("wizard://open", WizardOpenRequest { mode });
    show_main(app);
}
