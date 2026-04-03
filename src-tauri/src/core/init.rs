use tauri::{Builder, Manager, Wry};

pub fn setup_plugins(builder: Builder<Wry>) -> Builder<Wry> {
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let is_visible = window.is_visible().unwrap_or(false);
                if !is_visible {
                    let _ = window.show();
                }
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())

}

pub fn generate_handlers() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static
{
    use super::cmd::*;
    tauri::generate_handler![get_today_stats, get_app_icon_native, check_window_url]
}

pub fn setup_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let label = window.label();
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            log::info!("CloseRequested: {}", label);
            let _ = window.hide();
            api.prevent_close();

        }
        _ => {}
    }
}
