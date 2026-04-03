use anyhow::Result;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};
#[cfg(desktop)]
use tauri_plugin_autostart::{MacosLauncher,ManagerExt};

pub fn create_tray_icon(app: &App) -> Result<()> {
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;
    #[cfg(desktop)]
    {
   
        let _ = app.handle().plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ));
        let auto_i = CheckMenuItem::with_id(
            app,
            "autostart",
            "AutoStart",
            true,
            app.autolaunch().is_enabled().unwrap_or(false),
            None::<&str>,
        )?;
        // menu = Menu::with_items(app, &[&auto_i,&show_i, &quit_i])?;
        menu.insert_items(&[&auto_i], 0)?;
    }
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            #[cfg(desktop)]
            "auto" => {
                let autostart_manager = app.autolaunch();
                let currently_enabled = autostart_manager.is_enabled().unwrap_or(false);
                let new_state = if currently_enabled {
                    autostart_manager.disable().is_ok() && false
                } else {
                    autostart_manager.enable().is_ok()
                };
                if let Some(item) = app.menu().and_then(|m| m.get("autostart")) {
                    if let Some(check_item) = item.as_check_menuitem() {
                        let _ = check_item.set_checked(new_state);
                    }
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let is_visible = window.is_visible().unwrap_or(false);
                    if is_visible {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}
