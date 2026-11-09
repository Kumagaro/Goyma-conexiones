pub mod activity;
pub mod attachments;
pub mod automation;
pub mod backup;
pub mod commands;
pub mod csv_io;
pub mod db;
pub mod errors;
pub mod export;
pub mod inventory;
pub mod invoice_counter;
pub mod invoice_print;
pub mod invoice_snapshot;
pub mod models;
pub mod pricing;
pub mod search;
pub mod state;
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
            commands::categories::update_category,
            commands::categories::get_category,
            commands::categories::delete_category,
            commands::categories::list_categories,
            commands::categories::search_categories,
            commands::clients::create_client,
            commands::clients::update_client,
            commands::clients::delete_client,
            commands::clients::get_client_detail,
            commands::clients::list_clients,
            commands::clients::search_clients,
            commands::suppliers::create_supplier,
            commands::suppliers::update_supplier,
            commands::suppliers::delete_supplier,
            commands::suppliers::get_supplier_detail,
            commands::suppliers::list_suppliers,
            commands::suppliers::search_suppliers,
            commands::units::create_unit,
            commands::units::update_unit,
            commands::units::delete_unit,
            commands::units::list_units,
            commands::units::search_units,
            commands::conversions::list_conversions,
            commands::conversions::create_conversion,
            commands::conversions::update_conversion_factor,
            commands::conversions::delete_conversion,
            commands::conversions::convert_quantity,
            commands::products::create_product,
            commands::products::list_products,
            commands::products::search_products_for_invoice,
            commands::products::get_product_associated_invoices,
            commands::products::list_out_of_stock_and_discontinued,
            commands::products::get_product,
            commands::products::update_product,
            commands::products::preview_selling_price,
            commands::products::discontinue_product,
            commands::products::activate_product,
            commands::products::delete_product,
            commands::products::list_unit_options_for_product,
            commands::history::get_product_history,
            commands::history::list_history_products,
            commands::history::export_history_excel,
            commands::history::export_history_pdf,
            activity::list_activity_log,
            backup::create_backup,
            backup::list_automatic_backups,
            backup::restore_backup,
            csv_io::export_csv,
            csv_io::import_csv,
            commands::drafts::save_draft,
            commands::drafts::get_draft,
            commands::drafts::list_drafts,
            commands::drafts::delete_draft,
            commands::dashboard::get_dashboard_kpis,
            commands::dashboard::get_dashboard_summary,
            commands::invoices::list_invoices,
            commands::invoices::get_invoice,
            commands::invoices::create_entry_invoice,
            commands::invoices::update_entry_invoice,
            commands::invoices::create_sale_invoice,
            commands::invoices::create_service_invoice,
            commands::invoices::create_guarantee_invoice,
            commands::invoices::create_installation_invoice,
            commands::invoices::create_loss_invoice,
            commands::invoices::update_sale_invoice,
            commands::invoices::update_service_invoice,
            commands::invoices::update_guarantee_invoice,
            commands::invoices::update_installation_invoice,
            commands::invoices::save_invoice_attachment,
            commands::invoices::download_invoice_attachment,
            commands::invoices::change_invoice_status,
            commands::invoices::record_deliveries,
            commands::services::list_services,
            commands::services::get_service,
            commands::services::create_service,
            commands::services::update_service,
            commands::services::delete_service,
            commands::invoices::delete_invoice,
            commands::invoices::get_invoice_print_model,
            commands::invoices::export_invoice_pdf,
            commands::notifications::list_notifications,
            commands::notifications::count_unread_notifications,
            commands::notifications::list_notifications_since,
            commands::notifications::mark_notification_read,
            commands::notifications::mark_all_notifications_read,
            commands::notifications::delete_notification,
            commands::notifications::delete_notifications,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_user_preferences,
            commands::settings::update_user_preferences,
            commands::settings::list_exchange_rates,
            commands::settings::update_exchange_rate,
            commands::settings::list_exchange_rate_history,
            commands::settings::get_company_profile,
            commands::settings::update_company_profile,
            commands::settings::save_company_logo,
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
