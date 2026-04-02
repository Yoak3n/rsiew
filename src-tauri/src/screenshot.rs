use std::path::{Path, PathBuf};

pub struct ScreenshotService {
    data_dir: PathBuf,
}

impl ScreenshotService {
    pub fn new(data_dir: &Path) -> Self {
        let screenshots_dir = data_dir.join("screenshots");
        let _ = std::fs::create_dir_all(&screenshots_dir);
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }

    pub fn capture(&self) -> Result<PathBuf, String> {
        let now = chrono::Local::now().timestamp();
        let file_name = format!("capture_{}.png", now);
        let save_path = self.data_dir.join("screenshots").join(file_name);

        self.capture_impl(&save_path)?;
        Ok(save_path)
    }

    #[cfg(target_os = "macos")]
    fn capture_impl(&self, path: &Path) -> Result<(), String> {
        use screenshots::Screen;
        let screens = Screen::all().map_err(|e| e.to_string())?;
        if let Some(screen) = screens.first() {
            let image = screen.capture().map_err(|e| e.to_string())?;
            std::fs::write(path, image.to_png().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        Err("No screens found".to_string())
    }

    #[cfg(target_os = "linux")]
    fn capture_impl(&self, path: &Path) -> Result<(), String> {
        use std::process::Command;
        let output = Command::new("scrot")
            .arg(path.to_str().unwrap())
            .output()
            .map_err(|e| e.to_string())?;
            
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[cfg(target_os = "windows")]
    fn capture_impl(&self, path: &Path) -> Result<(), String> {
        use winapi::um::wingdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
            SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
        };
        use winapi::um::winuser::{
            GetDC, GetDesktopWindow, GetSystemMetrics, ReleaseDC, SM_CXSCREEN, SM_CYSCREEN,
        };

        unsafe {
            // 1. 获取桌面句柄和尺寸
            let hwnd = GetDesktopWindow();
            let hdc_screen = GetDC(hwnd);
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);

            if width == 0 || height == 0 {
                ReleaseDC(hwnd, hdc_screen);
                return Err("Failed to get screen metrics".to_string());
            }

            // 2. 创建兼容的设备上下文和位图
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbm_screen = CreateCompatibleBitmap(hdc_screen, width, height);

            if hbm_screen.is_null() {
                DeleteDC(hdc_mem);
                ReleaseDC(hwnd, hdc_screen);
                return Err("Failed to create compatible bitmap".to_string());
            }

            let hbm_old = SelectObject(hdc_mem, hbm_screen as *mut _);

            // 3. 将屏幕内容拷贝到内存设备上下文中
            if BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY) == 0 {
                SelectObject(hdc_mem, hbm_old);
                DeleteObject(hbm_screen as *mut _);
                DeleteDC(hdc_mem);
                ReleaseDC(hwnd, hdc_screen);
                return Err("BitBlt failed".to_string());
            }

            // 4. 提取位图数据 (DIB)
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width;
            bmi.bmiHeader.biHeight = -height; // 负数表示自顶向下，像素排列与通常的图片一致
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32; // BGRA
            bmi.bmiHeader.biCompression = BI_RGB;

            let pixel_count = (width * height) as usize;
            let mut buffer: Vec<u8> = vec![0; pixel_count * 4];

            let scan_lines = GetDIBits(
                hdc_screen,
                hbm_screen,
                0,
                height as u32,
                buffer.as_mut_ptr() as *mut _,
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // 清理 GDI 资源
            SelectObject(hdc_mem, hbm_old);
            DeleteObject(hbm_screen as *mut _);
            DeleteDC(hdc_mem);
            ReleaseDC(hwnd, hdc_screen);

            if scan_lines == 0 {
                return Err("GetDIBits failed".to_string());
            }

            // 5. BGRA 转 RGBA
            for chunk in buffer.chunks_exact_mut(4) {
                chunk.swap(0, 2); // 交换 B 和 R
                chunk[3] = 255;   // 强制 Alpha 通道不透明，避免截图透明
            }

            // 6. 使用 image 库编码保存为 PNG
            let img = image::RgbaImage::from_raw(width as u32, height as u32, buffer)
                .ok_or("Failed to create RgbaImage from raw pixels")?;
            
            img.save(path).map_err(|e| format!("Failed to save screenshot: {}", e))?;

            Ok(())
        }
    }
}