use gtm_db::{DatabaseStatus, DbError};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    database_path: String,
    schema_version: i64,
    app_name: String,
}

impl From<DatabaseStatus> for AppStatus {
    fn from(status: DatabaseStatus) -> Self {
        Self {
            database_path: status.database_path.display().to_string(),
            schema_version: status.schema_version,
            app_name: status.app_name,
        }
    }
}

#[tauri::command]
pub fn get_app_status(app: AppHandle) -> Result<AppStatus, String> {
    let dir = app.path().app_data_dir().map_err(|_| {
        "Impossible de déterminer le répertoire de données de l'application.".to_string()
    })?;
    gtm_db::initialize(&dir)
        .map(AppStatus::from)
        .map_err(|error| match error {
            DbError::Inconsistent(message) => message,
            other => other.to_string(),
        })
}
