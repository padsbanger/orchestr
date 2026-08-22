use std::{fs, sync::Mutex};

use orchestr_db::SettingsRepository;
use tauri::{Manager, State};

struct AppState {
    settings: Mutex<SettingsRepository>,
}

#[tauri::command]
fn get_setting(key: String, state: State<'_, AppState>) -> Result<Option<String>, String> {
    state
        .settings
        .lock()
        .map_err(|_| "Local settings store is unavailable.".to_owned())?
        .get(&key)
        .map_err(|error| format!("Unable to read local setting: {error}"))
}

#[tauri::command]
fn set_setting(key: String, value: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .settings
        .lock()
        .map_err(|_| "Local settings store is unavailable.".to_owned())?
        .set(&key, &value)
        .map_err(|error| format!("Unable to save local setting: {error}"))
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("orchestr.db");
            let settings = SettingsRepository::open(&database_path)?;

            app.manage(AppState {
                settings: Mutex::new(settings),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_setting, set_setting])
        .run(tauri::generate_context!())
        .expect("error while running Orchestr desktop application");
}

