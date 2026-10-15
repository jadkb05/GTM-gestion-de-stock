use super::catalogue::{ProductQueryDto, to_query};
use super::db;
use super::inventory::AlertQueryDto;
use gtm_db::{AlertQuery, ExportFormat, HistoryExportQuery};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcomeDto {
    rows: usize,
    scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryExportQueryDto {
    search: Option<String>,
    product_id: Option<i64>,
    direction: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReadyDto {
    line: i64,
    name: String,
    reference: Option<String>,
    type_name: String,
    supplier_name: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportIssueDto {
    line: i64,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewDto {
    ready: Vec<ImportReadyDto>,
    issues: Vec<ImportIssueDto>,
    ignored_columns: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultDto {
    created: usize,
}

fn format(value: &str) -> Result<ExportFormat, String> {
    ExportFormat::parse(value).map_err(|error| error.to_string())
}

fn destination(path: String) -> PathBuf {
    PathBuf::from(path)
}

#[tauri::command]
pub fn export_catalogue(
    app: AppHandle,
    path: String,
    format_name: String,
    query: ProductQueryDto,
) -> Result<ExportOutcomeDto, String> {
    let query = to_query(query)?;
    let conn = db::open(&app)?;
    gtm_db::export_catalogue(&conn, &destination(path), format(&format_name)?, &query)
        .map(|outcome| ExportOutcomeDto {
            rows: outcome.rows,
            scope: outcome.scope,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_history(
    app: AppHandle,
    path: String,
    format_name: String,
    query: HistoryExportQueryDto,
) -> Result<ExportOutcomeDto, String> {
    let conn = db::open(&app)?;
    gtm_db::export_history(
        &conn,
        &destination(path),
        format(&format_name)?,
        &HistoryExportQuery {
            search: query.search,
            product_id: query.product_id,
            direction: query.direction,
            from_date: query.from_date,
            to_date: query.to_date,
        },
    )
    .map(|outcome| ExportOutcomeDto {
        rows: outcome.rows,
        scope: outcome.scope,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_inventory(
    app: AppHandle,
    path: String,
    format_name: String,
    inventory_id: i64,
) -> Result<ExportOutcomeDto, String> {
    let conn = db::open(&app)?;
    gtm_db::export_inventory(
        &conn,
        &destination(path),
        format(&format_name)?,
        inventory_id,
    )
    .map(|outcome| ExportOutcomeDto {
        rows: outcome.rows,
        scope: outcome.scope,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_reorder_alerts(
    app: AppHandle,
    path: String,
    format_name: String,
    query: AlertQueryDto,
) -> Result<ExportOutcomeDto, String> {
    let conn = db::open(&app)?;
    gtm_db::export_reorder(
        &conn,
        &destination(path),
        format(&format_name)?,
        &AlertQuery {
            search: query.search,
            type_id: query.type_id,
            supplier_id: query.supplier_id,
            sort: query.sort,
        },
    )
    .map(|outcome| ExportOutcomeDto {
        rows: outcome.rows,
        scope: outcome.scope,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_article_import(app: AppHandle, path: String) -> Result<ImportPreviewDto, String> {
    let conn = db::open(&app)?;
    gtm_db::preview_article_import(&conn, &destination(path))
        .map(|preview| ImportPreviewDto {
            ready: preview
                .ready
                .into_iter()
                .map(|row| ImportReadyDto {
                    line: row.line,
                    name: row.name,
                    reference: row.reference,
                    type_name: row.type_name,
                    supplier_name: row.supplier_name,
                    description: row.description,
                })
                .collect(),
            issues: preview
                .issues
                .into_iter()
                .map(|issue| ImportIssueDto {
                    line: issue.line,
                    message: issue.message,
                })
                .collect(),
            ignored_columns: preview.ignored_columns,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn commit_article_import(app: AppHandle, path: String) -> Result<ImportResultDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::commit_article_import(&mut conn, &destination(path))
        .map(|created| ImportResultDto { created })
        .map_err(|error| error.to_string())
}
