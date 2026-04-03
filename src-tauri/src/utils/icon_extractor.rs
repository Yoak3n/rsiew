#[cfg(target_os = "windows")]
use winapi::shared::windef::HICON;

#[cfg(target_os = "windows")]
fn encode_windows_icon_path(value: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
unsafe fn get_windows_associated_icon(path: &str) -> Option<HICON> {
    use std::ptr::null_mut;
    use winapi::shared::minwindef::WORD;
    use winapi::um::shellapi::ExtractAssociatedIconW;

    let mut wide_path = encode_windows_icon_path(path);
    if wide_path.len() < 260 {
        wide_path.resize(260, 0);
    }
    let mut icon_index: WORD = 0;
    let icon = ExtractAssociatedIconW(null_mut(), wide_path.as_mut_ptr(), &mut icon_index);
    if icon.is_null() { None } else { Some(icon) }
}

#[cfg(target_os = "windows")]
unsafe fn render_windows_icon_pixels(icon: HICON) -> Option<(Vec<u8>, u32, u32)> {
    const DI_NORMAL: u32 = 0x0003;
    use std::mem::zeroed;
    use std::ptr::{copy_nonoverlapping, null_mut, write_bytes};
    use winapi::shared::minwindef::UINT;
    use winapi::shared::windef::HGDIOBJ;
    use winapi::um::wingdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetObjectW, SelectObject,
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use winapi::um::winuser::{DrawIconEx, GetDC, GetIconInfo, ReleaseDC, ICONINFO};

    let mut icon_info: ICONINFO = zeroed();
    if GetIconInfo(icon, &mut icon_info) == 0 { return None; }

    let rendered = (|| {
        let source_bitmap = if !icon_info.hbmColor.is_null() { icon_info.hbmColor } else { icon_info.hbmMask };
        if source_bitmap.is_null() { return None; }

        let mut bitmap: BITMAP = zeroed();
        if GetObjectW(source_bitmap as *mut _, std::mem::size_of::<BITMAP>() as i32, &mut bitmap as *mut _ as *mut _) == 0 { return None; }

        let width = bitmap.bmWidth.abs();
        let mut height = bitmap.bmHeight.abs();
        if icon_info.hbmColor.is_null() { height /= 2; }
        if width <= 0 || height <= 0 { return None; }

        let screen_dc = GetDC(null_mut());
        if screen_dc.is_null() { return None; }

        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_null() {
            ReleaseDC(null_mut(), screen_dc);
            return None;
        }

        let mut bitmap_info: BITMAPINFO = zeroed();
        bitmap_info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bitmap_info.bmiHeader.biWidth = width;
        bitmap_info.bmiHeader.biHeight = -height;
        bitmap_info.bmiHeader.biPlanes = 1;
        bitmap_info.bmiHeader.biBitCount = 32;
        bitmap_info.bmiHeader.biCompression = BI_RGB;

        let mut dib_bits = null_mut();
        let dib = CreateDIBSection(screen_dc, &bitmap_info, DIB_RGB_COLORS as UINT, &mut dib_bits, null_mut(), 0);
        if dib.is_null() || dib_bits.is_null() {
            DeleteDC(mem_dc);
            ReleaseDC(null_mut(), screen_dc);
            return None;
        }

        let old_object = SelectObject(mem_dc, dib as HGDIOBJ);
        if old_object.is_null() {
            DeleteObject(dib as HGDIOBJ);
            DeleteDC(mem_dc);
            ReleaseDC(null_mut(), screen_dc);
            return None;
        }

        let pixel_len = width as usize * height as usize * 4;
        write_bytes(dib_bits as *mut u8, 0, pixel_len);

        let draw_result = DrawIconEx(mem_dc, 0, 0, icon, width, height, 0, null_mut(), DI_NORMAL);
        let mut pixels = None;
        if draw_result != 0 {
            let mut buffer = vec![0; pixel_len];
            copy_nonoverlapping(dib_bits as *const u8, buffer.as_mut_ptr(), pixel_len);
            pixels = Some((buffer, width as u32, height as u32));
        }

        SelectObject(mem_dc, old_object);
        DeleteObject(dib as HGDIOBJ);
        DeleteDC(mem_dc);
        ReleaseDC(null_mut(), screen_dc);
        pixels
    })();

    if !icon_info.hbmColor.is_null() { DeleteObject(icon_info.hbmColor as HGDIOBJ); }
    if !icon_info.hbmMask.is_null() { DeleteObject(icon_info.hbmMask as HGDIOBJ); }

    rendered
}

#[cfg(target_os = "windows")]
fn encode_windows_icon_base64(mut pixels: Vec<u8>, width: u32, height: u32) -> Option<String> {
    if width == 0 || height == 0 { return None; }
    // BGRA to RGBA
    for chunk in pixels.chunks_exact_mut(4) { chunk.swap(0, 2); }

    let image = image::RgbaImage::from_raw(width, height, pixels)?;
    let mut dynamic_image = image::DynamicImage::ImageRgba8(image);
    if width > 128 || height > 128 {
        dynamic_image = dynamic_image.resize_exact(128, 128, image::imageops::FilterType::Lanczos3);
    }

    let mut cursor = std::io::Cursor::new(Vec::new());
    dynamic_image.write_to(&mut cursor, image::ImageFormat::Png).ok()?;

    use base64::Engine;
    Some(base64::engine::general_purpose::STANDARD.encode(cursor.into_inner()))
}

#[cfg(target_os = "windows")]
pub fn extract_icon_base64(path: &str) -> String {
    if path.is_empty() { return String::new(); }
    unsafe {
        if let Some(icon) = get_windows_associated_icon(path) {
            if let Some((pixels, width, height)) = render_windows_icon_pixels(icon) {
                use winapi::um::winuser::DestroyIcon;
                DestroyIcon(icon);
                if let Some(encoded) = encode_windows_icon_base64(pixels, width, height) {
                    return format!("{}", encoded);
                }
            }
        }
    }
    String::new()
}

#[cfg(not(target_os = "windows"))]
pub fn extract_icon_base64(_path: &str) -> String {
    String::new()
}