#[cfg(windows)]
fn main() {
    use std::env;
    use std::process::Command;

    let args: Vec<String> = env::args().collect();
    
    let mut exe_path = env::current_exe().expect("Failed to get current exe path");
    // Change our name (rsiew.com or rsiew-cli.exe) to the actual GUI app rsiew.exe
    exe_path.set_file_name("rsiew.exe");
    
    // Spawn the GUI app with the same arguments
    let mut child = Command::new(&exe_path)
        .args(&args[1..])
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("Failed to start rsiew.exe at {:?}: {}", exe_path, e);
            std::process::exit(1);
        });
        
    // If arguments were provided (like "stats" or "--help"), wait for the command to finish.
    // This makes the terminal block and capture the output properly.
    // If no arguments were provided, the user is just launching the GUI, so we exit immediately
    // and let the GUI run detached, freeing up the terminal.
    if args.len() > 1 {
        let status = child.wait().expect("Failed to wait for rsiew.exe");
        std::process::exit(status.code().unwrap_or(0));
    }
}

// 在非 Windows 平台（如 macOS/Linux），这个文件不需要做任何事，
// 提供一个空的 main 函数让 Cargo 能顺利编译通过（虽然实际上我们不会用到它）。
#[cfg(not(windows))]
fn main() {}
