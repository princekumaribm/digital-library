// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod error;
mod routes;
mod util;

use error::ApiError;
use routes::AppState;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LIBRARY_DATA_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    // macOS: ~/Library/Application Support/DigitalLibrary (same as the Python version); Windows: %APPDATA%\DigitalLibrary.
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("DigitalLibrary")
}

/// Single entry point for the UI; runs off the main thread because hashing and dialogs block.
#[tauri::command]
async fn api(app: AppHandle, method: String, path: String, body: Option<Value>) -> Result<Value, ApiError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        routes::dispatch(Some(&app), &state, &method, &path, body.unwrap_or_else(|| json!({})))
    })
    .await
    .map_err(|_| ApiError::new(500, "Unexpected error."))?
}

fn main() {
    let state = AppState::new(data_dir());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = app.state::<AppState>();
            db::init(&state.db_path).map_err(|e| format!("Could not open the library database: {}", e.error))?;
            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![api])
        .run(tauri::generate_context!())
        .expect("error while running Digital Library");
}
