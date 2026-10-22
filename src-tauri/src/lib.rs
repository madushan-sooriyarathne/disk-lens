pub mod commands;
pub mod config;
pub mod error;
pub mod scanner;
pub mod state;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(state::AppState::new(config::Config::from_env()));
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::cancel_scan,
            commands::load_last_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
