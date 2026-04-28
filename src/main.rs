mod app;
mod commands;
mod date_utils;
mod dev_utils;
mod dto;
mod projects;
mod single_dev;
mod single_efforts;
mod workers;

use chrono::Utc;
use std::sync::Mutex;

use crate::app::{App, SAVE_PATH};
use crate::date_utils::dates::{local_to_days, primo_giorno_settimana_corrente};

pub struct TauriState {
    pub app: Mutex<App>,
    pub current_file: Mutex<String>,
    pub changed: Mutex<bool>,
    pub filter: Mutex<String>,
    pub this_week: usize,
}

fn main() {
    let mut app = App::load(SAVE_PATH).unwrap_or_else(|_| App::new());
    app.compute_sovra();

    let this_week = local_to_days(&primo_giorno_settimana_corrente(
        &Utc::now().date_naive(),
    )) as usize;

    let state = TauriState {
        app: Mutex::new(app),
        current_file: Mutex::new(SAVE_PATH.to_string()),
        changed: Mutex::new(false),
        filter: Mutex::new(String::new()),
        this_week,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_cells,
            commands::set_dev_effort,
            commands::set_note,
            commands::set_dev_note,
            commands::move_effort,
            commands::add_row,
            commands::del_row,
            commands::new_project,
            commands::rename_project,
            commands::add_dev_to_project,
            commands::add_worker,
            commands::add_dev,
            commands::set_worker_max_hours,
            commands::set_worker_week_override,
            commands::search_worker,
            commands::save_file,
            commands::open_file,
            commands::find_completions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
