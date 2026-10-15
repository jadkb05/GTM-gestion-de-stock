mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::status::get_app_status,
            commands::catalogue::list_products,
            commands::catalogue::get_product,
            commands::catalogue::create_product,
            commands::catalogue::update_product,
            commands::catalogue::set_product_active,
            commands::catalogue::list_product_types,
            commands::catalogue::create_product_type,
            commands::catalogue::update_product_type,
            commands::catalogue::set_product_type_active,
            commands::catalogue::list_suppliers,
            commands::catalogue::create_supplier,
            commands::catalogue::update_supplier,
            commands::catalogue::set_supplier_active,
            commands::stock::list_stock_levels,
            commands::stock::get_product_stock,
            commands::stock::record_stock_entry,
            commands::stock::record_stock_exit,
            commands::stock::list_stock_movements,
            commands::stock::get_stock_movement,
            commands::stock::get_stock_sheet,
            commands::stock::list_stock_sheet_movements,
            commands::stock::get_stock_overview,
            commands::inventory::list_inventories,
            commands::inventory::create_inventory,
            commands::inventory::get_inventory,
            commands::inventory::add_inventory_products,
            commands::inventory::save_inventory_counts,
            commands::inventory::check_inventory_drift,
            commands::inventory::refresh_inventory_line,
            commands::inventory::remove_inventory_line,
            commands::inventory::validate_inventory,
            commands::inventory::list_reorder_alerts,
            commands::inventory::set_reorder_threshold,
            commands::inventory::count_reorder_alerts,
            commands::exchange::export_catalogue,
            commands::exchange::export_history,
            commands::exchange::export_inventory,
            commands::exchange::export_reorder_alerts,
            commands::exchange::preview_article_import,
            commands::exchange::commit_article_import,
            commands::backup::get_backup_status,
            commands::backup::create_backup,
            commands::backup::inspect_backup,
            commands::backup::restore_backup,
        ])
        .run(tauri::generate_context!())
        .expect("échec du démarrage de GTM Stock Manager");
}
