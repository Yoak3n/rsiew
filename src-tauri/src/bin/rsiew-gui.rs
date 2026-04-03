#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if !rsiew_lib::cli::parse_as_cli() {
        rsiew_lib::run();
    }
}