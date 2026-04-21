// Prevents a second console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod discovery;
mod error;
mod server;
mod state;

use std::sync::Arc;

use commands::*;

fn main() {
    let shared_state = Arc::new(state::AppState::default());
    let state_for_server = shared_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(shared_state)
        .setup(move |app| {
            // Spin up the push-server on app start. It lives on 127.0.0.1
            // only and shares the same `AppState` as the Tauri commands, so
            // whatever DB the user has open is what `sqlv push` will hit.
            server::start(app.handle().clone(), state_for_server.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            open_db,
            close_db,
            list_tables,
            list_views,
            describe_table,
            run_query,
            run_exec,
            run_exec_many,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
