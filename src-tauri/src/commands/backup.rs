use super::db;
use gtm_db::BackupReport;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupReportDto {
    schema_version: i64,
    app_name: String,
    app_version: Option<String>,
    created_at: Option<String>,
    format_version: Option<String>,
    product_count: i64,
    supplier_count: i64,
    type_count: i64,
    movement_count: i64,
    inventory_count: i64,
}

impl From<BackupReport> for BackupReportDto {
    fn from(report: BackupReport) -> Self {
        Self {
            schema_version: report.schema_version,
            app_name: report.app_name,
            app_version: report.app_version,
            created_at: report.created_at,
            format_version: report.format_version,
            product_count: report.product_count,
            supplier_count: report.supplier_count,
            type_count: report.type_count,
            movement_count: report.movement_count,
            inventory_count: report.inventory_count,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatusDto {
    created_at: Option<String>,
    path: Option<String>,
}

fn destination(path: String) -> PathBuf {
    PathBuf::from(path)
}

#[tauri::command]
pub fn get_backup_status(app: AppHandle) -> Result<BackupStatusDto, String> {
    let conn = db::open(&app)?;
    gtm_db::last_backup_status(&conn)
        .map(|status| BackupStatusDto {
            created_at: status.created_at,
            path: status.path,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_backup(app: AppHandle, path: String) -> Result<BackupReportDto, String> {
    let conn = db::open(&app)?;
    gtm_db::create_backup(&conn, &destination(path), APP_VERSION)
        .map(BackupReportDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inspect_backup(_app: AppHandle, path: String) -> Result<BackupReportDto, String> {
    gtm_db::inspect_backup(&destination(path))
        .map(BackupReportDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn restore_backup(app: AppHandle, path: String) -> Result<BackupReportDto, String> {
    let _guard = db::gate();
    let dir = app.path().app_data_dir().map_err(|_| {
        "Impossible de déterminer le répertoire de données de l'application.".to_string()
    })?;
    gtm_db::restore_backup(&dir, &destination(path))
        .map(BackupReportDto::from)
        .map_err(|error| error.to_string())
}
