// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod user_path;

#[cfg(all(windows, not(debug_assertions)))]
fn attach_console() {
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::{STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    use winapi::um::fileapi::CreateFileW;
    use winapi::um::winnt::{FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};
    use winapi::um::fileapi::OPEN_EXISTING;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);

        // Redirect stdout and stderr to the console
        let con_out_name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        let con_out = CreateFileW(
            con_out_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut()
        );

        if con_out != winapi::um::handleapi::INVALID_HANDLE_VALUE {
            use winapi::um::processenv::SetStdHandle;
            SetStdHandle(STD_OUTPUT_HANDLE, con_out);
            SetStdHandle(STD_ERROR_HANDLE, con_out);
        }
    }
}

#[cfg(not(windows))]
fn attach_console() {}

fn main() {
    #[cfg(all(windows, not(debug_assertions)))]
    user_path::add_to_user_path();
    #[cfg(all(windows, not(debug_assertions)))]
    attach_console();

    rsiew_lib::run()
}