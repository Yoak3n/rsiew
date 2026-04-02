use std::env;
use std::process::Command;

fn main() {
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
