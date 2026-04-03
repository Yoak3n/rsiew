use std::process::Command;
fn main() {
    let output = Command::new("./test_gui.exe").output().unwrap();
    println!("Captured: {}", String::from_utf8_lossy(&output.stdout));
}
