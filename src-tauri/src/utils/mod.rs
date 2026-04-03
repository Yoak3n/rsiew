pub mod icon_extractor;


use std::path::PathBuf;

pub fn get_data_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("rsiew");
    std::fs::create_dir_all(&path).unwrap();
    path
}