//! Persistance SQLite locale de GTM Stock Manager.
//!
//! Le frontend n'appelle pas cette crate. La commande Tauri résout le
//! répertoire de données de l'installation, puis appelle [`initialize`].

mod backup;
mod catalogue;
mod error;
mod exchange;
mod migrate;
mod stock;

pub use catalogue::{
    ActivityFilter, CatalogueError, Product, ProductDraft, ProductQuery, ProductType, Supplier,
    create_product, create_product_type, create_supplier, get_product, import_products,
    list_product_types, list_products, list_suppliers, set_product_active, set_product_type_active,
    set_supplier_active, update_product, update_product_type, update_supplier,
};
pub use stock::{
    AlertQuery, CountInput, InventoryCounts, InventoryDetail, InventoryDraft, InventoryDrift,
    InventoryLine, InventoryListItem, MAX_MILLI, MovementDetail, MovementDraft, MovementQuery,
    PAGE_SIZE, ProductStock, ReorderAlert, SCALE, SheetLine, SheetPage, SheetQuery, StockError,
    StockLevel, StockMovement, StockOverview, StockSheet, add_inventory_products,
    check_inventory_drift, count_reorder_alerts, create_inventory, format_quantity, get_inventory,
    get_movement, get_movement_detail, get_product_stock, get_stock_sheet, list_inventories,
    list_movements, list_reorder_alerts, list_sheet_movements, list_stock_levels, record_entry,
    record_exit, refresh_inventory_line, remove_inventory_line, save_inventory_counts,
    set_reorder_threshold, stock_overview, validate_inventory,
};

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use backup::{
    BackupReport, BackupStatus, create_backup, inspect_backup, last_backup_status, restore_backup,
};
pub use error::DbError;
pub use exchange::{
    ExportFormat, ExportOutcome, HistoryExportQuery, ImportIssue, ImportPreview, ImportReady,
    commit_article_import, export_catalogue, export_history, export_inventory, export_reorder,
    preview_article_import,
};
pub use rusqlite::Connection;

const DATABASE_FILE_NAME: &str = "gtm-stock.db";

/// État de la base après initialisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseStatus {
    pub database_path: PathBuf,
    pub schema_version: i64,
    pub app_name: String,
}

/// Chemin du fichier SQLite dans le répertoire de données fourni.
pub fn database_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(DATABASE_FILE_NAME)
}

/// Crée le répertoire si besoin, ouvre la base et applique les migrations.
pub fn connect(app_data_dir: &Path) -> Result<Connection, DbError> {
    fs::create_dir_all(app_data_dir).map_err(DbError::CreateDir)?;
    backup::recover_interrupted_restore(app_data_dir)?;
    let path = database_path(app_data_dir);
    let mut connection = Connection::open(&path).map_err(DbError::Open)?;
    configure_session(&connection)?;
    migrate::apply_migrations(&mut connection, migrate::MIGRATIONS)?;
    Ok(connection)
}

pub fn initialize(app_data_dir: &Path) -> Result<DatabaseStatus, DbError> {
    let connection = connect(app_data_dir)?;
    let schema_version = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(DbError::Query)?;
    let app_name = read_metadata(&connection, "app_name")?.ok_or_else(|| {
        DbError::Inconsistent("La base locale ne contient pas le nom de l'application.".into())
    })?;

    Ok(DatabaseStatus {
        database_path: database_path(app_data_dir),
        schema_version,
        app_name,
    })
}

fn configure_session(connection: &Connection) -> Result<(), DbError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(DbError::Configure)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(DbError::Configure)?;
    Ok(())
}

pub(crate) fn read_metadata(connection: &Connection, key: &str) -> Result<Option<String>, DbError> {
    let mut statement = connection
        .prepare("SELECT value FROM app_metadata WHERE key = ?1")
        .map_err(DbError::Query)?;
    let mut rows = statement
        .query(rusqlite::params![key])
        .map_err(DbError::Query)?;

    if let Some(row) = rows.next().map_err(DbError::Query)? {
        let value: String = row.get(0).map_err(DbError::Query)?;
        return Ok(Some(value));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_app_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("horloge")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gtm-stock-manager-{nanos}-{id}"));
        fs::create_dir_all(&path).expect("répertoire temporaire");
        path
    }

    #[test]
    fn initialise_une_base_dans_le_repertoire_fourni() {
        let dir = temp_app_dir();
        let status = initialize(&dir).expect("initialisation");

        assert_eq!(status.database_path, dir.join("gtm-stock.db"));
        assert!(status.database_path.is_file());
        assert_eq!(status.schema_version, 4);
        assert_eq!(status.app_name, "GTM Stock Manager");

        let connection = Connection::open(&status.database_path).expect("réouverture");
        let phase = read_metadata(&connection, "schema_phase")
            .expect("lecture")
            .expect("phase");
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal");
        assert_eq!(phase, "4");
        assert_eq!(mode, "wal");
        drop(connection);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_seconde_initialisation_ne_duplique_pas_les_migrations() {
        let dir = temp_app_dir();
        initialize(&dir).expect("première");
        let status = initialize(&dir).expect("seconde");
        assert_eq!(status.schema_version, 4);

        let connection = Connection::open(database_path(&dir)).expect("ouverture");
        let migrations: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("compte");
        let metadata: i64 = connection
            .query_row("SELECT COUNT(*) FROM app_metadata", [], |row| row.get(0))
            .expect("compte");
        assert_eq!(migrations, 4);
        assert_eq!(metadata, 2);
        drop(connection);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn signale_un_repertoire_impossible_a_creer() {
        let dir = temp_app_dir();
        let blocking_file = dir.join("pas-un-dossier");
        fs::write(&blocking_file, b"occupe").expect("fichier");

        let error = initialize(&blocking_file).expect_err("chemin invalide");
        assert!(matches!(error, DbError::CreateDir(_)));
        assert!(error.to_string().contains("répertoire de données"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn une_base_plus_recente_conserve_son_journal() {
        let dir = temp_app_dir();
        let path = database_path(&dir);
        let connection = Connection::open(&path).expect("création");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );
                INSERT INTO schema_migrations (version, name, applied_at)
                VALUES (1, 'app_metadata', '2026-01-01T00:00:00Z');
                INSERT INTO schema_migrations (version, name, applied_at)
                VALUES (2, 'catalogue', '2026-01-01T00:00:00Z');
                INSERT INTO schema_migrations (version, name, applied_at)
                VALUES (3, 'stock', '2026-01-01T00:00:00Z');
                INSERT INTO schema_migrations (version, name, applied_at)
                VALUES (4, 'inventory', '2026-01-01T00:00:00Z');
                INSERT INTO schema_migrations (version, name, applied_at)
                VALUES (5, 'future', '2026-01-01T00:00:00Z');
                CREATE TABLE app_metadata (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL
                );
                INSERT INTO app_metadata (key, value)
                VALUES ('app_name', 'GTM Stock Manager');",
            )
            .expect("schéma futur");
        connection
            .pragma_update(None, "journal_mode", "DELETE")
            .expect("journal");
        drop(connection);

        let error = initialize(&dir).expect_err("version future");
        assert!(error.to_string().contains("plus récente"));

        let connection = Connection::open(&path).expect("relecture");
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("mode");
        let versions: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("versions");
        assert_eq!(mode, "delete");
        assert_eq!(versions, 5);
        assert!(!dir.join("gtm-stock.db-wal").exists());
        drop(connection);
        let _ = fs::remove_dir_all(&dir);
    }
}
