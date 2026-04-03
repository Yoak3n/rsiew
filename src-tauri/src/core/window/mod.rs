mod config;
mod schema;

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    Error, Manager as TauriManager, Url,
    WebviewWindow, WebviewWindowBuilder, Wry
};

use schema::{WindowState, WindowType};

use crate::core::handle;

pub struct Manager {
    configs: HashMap<schema::WindowType, config::WindowConfig>,
    states: Arc<Mutex<HashMap<schema::WindowType, schema::WindowState>>>,
}

impl Manager {
    fn new() -> Self {
        Self {
            configs: HashMap::from([(
                WindowType::Main,
                config::WindowConfig::new(WindowType::Main),
            )]),
            states: Arc::new(Mutex::new(HashMap::from([(
                WindowType::Main,
                WindowState::NotExist,
            )]))),
        }
    }

    pub fn global() -> &'static Self {
        static INSTANCE: OnceCell<Manager> = OnceCell::new();
        INSTANCE.get_or_init(Self::new)
    }

    // 获取窗口实例
    pub fn get_window(&self, window_type: WindowType) -> Option<WebviewWindow<Wry>> {
        handle::Handle::global().app_handle().and_then(|app| app.get_webview_window(window_type.label()))
    }

    pub fn update_window_state(&self, window_type: WindowType, state: WindowState) {
        self.states.lock().unwrap().insert(window_type, state);
    }

    pub fn get_cached_window_state(&self, window_type: WindowType) -> WindowState {
        self.states
            .lock()
            .unwrap()
            .get(&window_type)
            .copied()
            .unwrap_or(WindowState::NotExist)
    }

    fn create_window_inner(&self, window_type: WindowType, url_with_args: Option<&str>) -> Result<WebviewWindow<Wry>, Error> {
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let config = self.configs.get(&window_type).ok_or_else(|| Error::FailedToReceiveMessage)?;

                // 检查是否已存在窗口
        if let Some(existing_window) = self.get_window(window_type) {
            if let Some(f) = url_with_args{
                let current_url = existing_window.url().map(|u| u.to_string()).unwrap_or("".to_string());
                
                if current_url != f.to_string() {
                    let _ = existing_window.navigate(Url::parse(f).unwrap());
                }
            }

            


            if existing_window.is_minimized().unwrap_or(false) {
                let _ = existing_window.unminimize();
            }
            let _ = existing_window.show();
            let _ = existing_window.set_focus();

            // 更新缓存状态为可见且有焦点
            self.update_window_state(window_type, WindowState::VisibleFocused);

            return Ok(existing_window);
        }


        let builder = WebviewWindowBuilder::new(
            &app_handle, 
            window_type.label().to_string(),
            tauri::WebviewUrl::App(
                url_with_args.map(|u| u.parse().unwrap_or(window_type.url().parse().unwrap_or_default()))
                    .unwrap_or_else(|| window_type.url().into()),
            ));
        builder.build().map_err(|e| Error::from(e))


    }


    fn destroy_window(&self, window_type: WindowType) -> bool {
        match self.get_window(window_type) {
            Some(window) => {
                if let Err(e) = window.close() {
                    log::error!("窗口销毁失败: {:?}", e);
                    return false
                }
                self.update_window_state(window_type, WindowState::NotExist);
                true
            }
            None => {
                self.update_window_state(window_type, WindowState::NotExist);
                true
            },
        }
    }
}
