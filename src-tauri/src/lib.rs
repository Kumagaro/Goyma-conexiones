pub mod commands;
pub mod db;
pub mod errors;
pub mod models;
pub mod text;
pub mod timestamps;
pub mod validators;

#[cfg(test)]
pub mod test_utils;

use std::sync::Mutex;

use chrono::Utc;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::db::connection::{initialize_database, install_panic_hook, resolve_app_data_dir};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = resolve_app_data_dir(app.handle())?;
            install_panic_hook(&app_data_dir);
            let connection = match initialize_database(&app_data_dir, Utc::now()) {
                Ok(connection) => connection,
                Err(message) => {
                    app.dialog().message(&message).blocking_show();
                    return Err(std::io::Error::other(message).into());
                }
            };
            app.manage(Mutex::new(connection));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::login,
            commands::auth::logout,
            commands::auth::get_session_info,
            commands::auth::get_current_session,
            commands::auth::touch_session,
            commands::auth::verify_recovery_identity,
            commands::auth::recover_password,
            commands::categories::create_category,
            commands::categories::search_categories,
            commands::users::create_user,
            commands::users::list_users,
            commands::users::get_user_profile,
            commands::users::update_own_profile,
            commands::users::change_own_password,
            commands::users::update_user,
            commands::users::reset_user_password,
            commands::users::delete_user
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
