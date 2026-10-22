pub mod cleanup;
pub mod commands;
pub mod config;
pub mod error;
pub mod platform;
pub mod providers;
pub mod recommend;
pub mod rules;
pub mod scanner;
pub mod state;
pub mod view;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Validates the bundled rules once at startup; an invalid file is a build defect.
    rules::model::Rules::embedded();
    let state = Arc::new(state::AppState::new(config::Config::from_env()));
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::cancel_scan,
            commands::load_last_snapshot,
            commands::get_view,
            commands::reveal_in_finder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
