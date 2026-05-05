pub mod attachment;
pub mod character;
pub mod chat;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod llm;
pub mod memory;
pub mod models;
pub mod plugin;
pub mod spontaneous;
pub mod state;
pub mod thought;
pub mod tts;

pub use error::AppError;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
