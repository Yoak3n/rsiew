use super::schema::WindowType;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub window_type: WindowType,
    pub inner_size: (f64, f64),
    pub min_inner_size: (f64, f64),
    pub decorations: bool,
    pub transparent: bool,
    pub skip_taskbar: bool,
    pub shadow: bool,
    pub always_on_top: bool,
    pub maximizable: bool,
    pub focused: bool,
    pub center: bool,
}

impl WindowConfig {
    pub fn new(window_type: WindowType) -> Self {
        match window_type {
            WindowType::Main => Self {
                window_type,
                inner_size: (800.0, 600.0),
                min_inner_size: (400.0, 80.0),
                decorations: true,
                transparent: false,
                skip_taskbar: false,
                shadow: false,
                always_on_top: false,
                maximizable: true,
                focused: true,
                center: true,
            },
        }
    }
}
