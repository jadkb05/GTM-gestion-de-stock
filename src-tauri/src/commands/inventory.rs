use super::db;
use gtm_db::{
    AlertQuery, CountInput, InventoryCounts, InventoryDetail, InventoryDraft, InventoryDrift,
    InventoryLine, InventoryListItem, ReorderAlert, format_quantity,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryListItemDto {
    id: i64,
    name: Option<String>,
    status: String,
    created_at: String,
    validated_at: Option<String>,
    product_count: i64,
    variance_count: i64,
}

impl From<InventoryListItem> for InventoryListItemDto {
    fn from(item: InventoryListItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            status: item.status,
            created_at: item.created_at,
            validated_at: item.validated_at,
            product_count: item.product_count,
            variance_count: item.variance_count,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryCountsDto {
    counted: i64,
    unchanged: i64,
    surplus: i64,
    shortage: i64,
    uncounted: i64,
    stale: i64,
}

impl From<InventoryCounts> for InventoryCountsDto {
    fn from(counts: InventoryCounts) -> Self {
        Self {
            counted: counts.counted,
            unchanged: counts.unchanged,
            surplus: counts.surplus,
            shortage: counts.shortage,
            uncounted: counts.uncounted,
            stale: counts.stale,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryLineDto {
    id: i64,
    product_id: i64,
    product_name: String,
    product_reference: Option<String>,
    type_name: String,
    theoretical: String,
    counted: Option<String>,
    current: String,
    variance: Option<String>,
    drifted: bool,
}

impl From<InventoryLine> for InventoryLineDto {
    fn from(line: InventoryLine) -> Self {
        Self {
            id: line.id,
            product_id: line.product_id,
            product_name: line.product_name,
            product_reference: line.product_reference,
            type_name: line.type_name,
            theoretical: format_quantity(line.theoretical_milli),
            counted: line.counted_milli.map(format_quantity),
            current: format_quantity(line.current_milli),
            variance: line.variance_milli.map(format_quantity),
            drifted: line.drifted,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryDetailDto {
    id: i64,
    name: Option<String>,
    comment: Option<String>,
    status: String,
    created_at: String,
    validated_at: Option<String>,
    lines: Vec<InventoryLineDto>,
    counts: InventoryCountsDto,
}

impl From<InventoryDetail> for InventoryDetailDto {
    fn from(detail: InventoryDetail) -> Self {
        Self {
            id: detail.id,
            name: detail.name,
            comment: detail.comment,
            status: detail.status,
            created_at: detail.created_at,
            validated_at: detail.validated_at,
            lines: detail
                .lines
                .into_iter()
                .map(InventoryLineDto::from)
                .collect(),
            counts: detail.counts.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryInput {
    name: Option<String>,
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountInputDto {
    line_id: i64,
    quantity: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryDriftDto {
    line_id: i64,
    product_id: i64,
    product_name: String,
    theoretical: String,
    current: String,
}

impl From<InventoryDrift> for InventoryDriftDto {
    fn from(drift: InventoryDrift) -> Self {
        Self {
            line_id: drift.line_id,
            product_id: drift.product_id,
            product_name: drift.product_name,
            theoretical: format_quantity(drift.theoretical_milli),
            current: format_quantity(drift.current_milli),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertQueryDto {
    pub(crate) search: Option<String>,
    pub(crate) type_id: Option<i64>,
    pub(crate) supplier_id: Option<i64>,
    pub(crate) sort: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderAlertDto {
    product_id: i64,
    name: String,
    reference: Option<String>,
    type_id: i64,
    type_name: String,
    supplier_id: Option<i64>,
    supplier_name: Option<String>,
    quantity: String,
    threshold: String,
    gap: String,
}

impl From<ReorderAlert> for ReorderAlertDto {
    fn from(alert: ReorderAlert) -> Self {
        Self {
            product_id: alert.product_id,
            name: alert.name,
            reference: alert.reference,
            type_id: alert.type_id,
            type_name: alert.type_name,
            supplier_id: alert.supplier_id,
            supplier_name: alert.supplier_name,
            quantity: format_quantity(alert.quantity_milli),
            threshold: format_quantity(alert.threshold_milli),
            gap: format_quantity(alert.quantity_milli - alert.threshold_milli),
        }
    }
}

#[tauri::command]
pub fn list_inventories(app: AppHandle) -> Result<Vec<InventoryListItemDto>, String> {
    let conn = db::open(&app)?;
    gtm_db::list_inventories(&conn)
        .map(|items| items.into_iter().map(InventoryListItemDto::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_inventory(
    app: AppHandle,
    inventory: InventoryInput,
) -> Result<InventoryDetailDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::create_inventory(
        &mut conn,
        InventoryDraft {
            name: inventory.name,
            comment: inventory.comment,
        },
    )
    .map(InventoryDetailDto::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_inventory(app: AppHandle, id: i64) -> Result<InventoryDetailDto, String> {
    let conn = db::open(&app)?;
    gtm_db::get_inventory(&conn, id)
        .map(InventoryDetailDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn add_inventory_products(
    app: AppHandle,
    inventory_id: i64,
    product_ids: Vec<i64>,
) -> Result<InventoryDetailDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::add_inventory_products(&mut conn, inventory_id, &product_ids)
        .map(InventoryDetailDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_inventory_counts(
    app: AppHandle,
    inventory_id: i64,
    counts: Vec<CountInputDto>,
) -> Result<InventoryDetailDto, String> {
    let mut conn = db::open(&app)?;
    let counts = counts
        .into_iter()
        .map(|count| CountInput {
            line_id: count.line_id,
            quantity: count.quantity,
        })
        .collect::<Vec<_>>();
    gtm_db::save_inventory_counts(&mut conn, inventory_id, &counts)
        .map(InventoryDetailDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn check_inventory_drift(app: AppHandle, id: i64) -> Result<Vec<InventoryDriftDto>, String> {
    let conn = db::open(&app)?;
    gtm_db::check_inventory_drift(&conn, id)
        .map(|rows| rows.into_iter().map(InventoryDriftDto::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn refresh_inventory_line(
    app: AppHandle,
    line_id: i64,
    reset_count: bool,
) -> Result<InventoryDetailDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::refresh_inventory_line(&mut conn, line_id, reset_count)
        .map(InventoryDetailDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn remove_inventory_line(app: AppHandle, line_id: i64) -> Result<InventoryDetailDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::remove_inventory_line(&mut conn, line_id)
        .map(InventoryDetailDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn validate_inventory(app: AppHandle, id: i64) -> Result<InventoryDetailDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::validate_inventory(&mut conn, id)
        .map(InventoryDetailDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_reorder_alerts(
    app: AppHandle,
    query: AlertQueryDto,
) -> Result<Vec<ReorderAlertDto>, String> {
    let conn = db::open(&app)?;
    gtm_db::list_reorder_alerts(
        &conn,
        &AlertQuery {
            search: query.search,
            type_id: query.type_id,
            supplier_id: query.supplier_id,
            sort: query.sort,
        },
    )
    .map(|rows| rows.into_iter().map(ReorderAlertDto::from).collect())
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_reorder_threshold(
    app: AppHandle,
    product_id: i64,
    threshold: Option<String>,
) -> Result<Option<String>, String> {
    let mut conn = db::open(&app)?;
    gtm_db::set_reorder_threshold(&mut conn, product_id, threshold.as_deref())
        .map(|milli| milli.map(format_quantity))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn count_reorder_alerts(app: AppHandle) -> Result<i64, String> {
    let conn = db::open(&app)?;
    gtm_db::count_reorder_alerts(&conn).map_err(|error| error.to_string())
}
