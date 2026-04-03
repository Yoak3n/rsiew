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
    if args.len() > 1 {
        // 开机自启会加入--autostart参数，意味着不是cli模式，不需要等待
        if !args.contains(&"--autostart".to_string()) {
            let status = child.wait().expect("Failed to wait for rsiew.exe");
            std::process::exit(status.code().unwrap_or(0));
        }
    }
}

#[cfg(not(windows))]
fn main() {}
