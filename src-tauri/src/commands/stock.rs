use super::db;
use gtm_db::{
    MovementDraft, MovementQuery, ProductStock, SheetQuery, StockLevel, StockMovement,
    StockOverview, StockSheet, format_quantity,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockLevelDto {
    product_id: i64,
    quantity: String,
}

impl From<StockLevel> for StockLevelDto {
    fn from(level: StockLevel) -> Self {
        Self {
            product_id: level.product_id,
            quantity: format_quantity(level.quantity_milli),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductStockDto {
    product_id: i64,
    product_name: String,
    is_active: bool,
    quantity: String,
}

impl From<ProductStock> for ProductStockDto {
    fn from(stock: ProductStock) -> Self {
        Self {
            product_id: stock.product_id,
            product_name: stock.product_name,
            is_active: stock.is_active,
            quantity: format_quantity(stock.quantity_milli),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovementDto {
    id: i64,
    product_id: i64,
    product_name: String,
    product_reference: Option<String>,
    direction: String,
    quantity: String,
    reason: Option<String>,
    document_reference: Option<String>,
    comment: Option<String>,
    created_at: String,
    source: String,
    inventory_id: Option<i64>,
}

impl From<StockMovement> for MovementDto {
    fn from(movement: StockMovement) -> Self {
        Self {
            id: movement.id,
            product_id: movement.product_id,
            product_name: movement.product_name,
            product_reference: movement.product_reference,
            direction: movement.direction,
            quantity: format_quantity(movement.quantity_milli),
            reason: movement.reason,
            document_reference: movement.document_reference,
            comment: movement.comment,
            created_at: movement.created_at,
            source: movement.source,
            inventory_id: movement.inventory_id,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovementInput {
    product_id: i64,
    quantity: String,
    reason: Option<String>,
    document_reference: Option<String>,
    comment: Option<String>,
}

impl From<MovementInput> for MovementDraft {
    fn from(input: MovementInput) -> Self {
        Self {
            product_id: input.product_id,
            quantity: input.quantity,
            reason: input.reason,
            document_reference: input.document_reference,
            comment: input.comment,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovementQueryDto {
    search: Option<String>,
    product_id: Option<i64>,
    direction: Option<String>,
}

#[tauri::command]
pub fn list_stock_levels(app: AppHandle) -> Result<Vec<StockLevelDto>, String> {
    let conn = db::open(&app)?;
    gtm_db::list_stock_levels(&conn)
        .map(|rows| rows.into_iter().map(StockLevelDto::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_product_stock(app: AppHandle, product_id: i64) -> Result<ProductStockDto, String> {
    let conn = db::open(&app)?;
    gtm_db::get_product_stock(&conn, product_id)
        .map(ProductStockDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn record_stock_entry(app: AppHandle, movement: MovementInput) -> Result<MovementDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::record_entry(&mut conn, movement.into())
        .map(MovementDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn record_stock_exit(app: AppHandle, movement: MovementInput) -> Result<MovementDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::record_exit(&mut conn, movement.into())
        .map(MovementDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_stock_movements(
    app: AppHandle,
    query: MovementQueryDto,
) -> Result<Vec<MovementDto>, String> {
    let conn = db::open(&app)?;
    gtm_db::list_movements(
        &conn,
        &MovementQuery {
            search: query.search,
            product_id: query.product_id,
            direction: query.direction,
        },
    )
    .map(|rows| rows.into_iter().map(MovementDto::from).collect())
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_stock_movement(app: AppHandle, id: i64) -> Result<MovementDetailDto, String> {
    let conn = db::open(&app)?;
    gtm_db::get_movement_detail(&conn, id)
        .map(MovementDetailDto::from)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovementDetailDto {
    id: i64,
    product_id: i64,
    product_name: String,
    product_reference: Option<String>,
    direction: String,
    quantity: String,
    reason: Option<String>,
    document_reference: Option<String>,
    comment: Option<String>,
    created_at: String,
    source: String,
    inventory_id: Option<i64>,
    balance_after: String,
}

impl From<gtm_db::MovementDetail> for MovementDetailDto {
    fn from(detail: gtm_db::MovementDetail) -> Self {
        let movement = detail.movement;
        Self {
            id: movement.id,
            product_id: movement.product_id,
            product_name: movement.product_name,
            product_reference: movement.product_reference,
            direction: movement.direction,
            quantity: format_quantity(movement.quantity_milli),
            reason: movement.reason,
            document_reference: movement.document_reference,
            comment: movement.comment,
            created_at: movement.created_at,
            source: movement.source,
            inventory_id: movement.inventory_id,
            balance_after: format_quantity(detail.balance_after_milli),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockSheetDto {
    product_id: i64,
    name: String,
    reference: Option<String>,
    type_name: String,
    supplier_name: Option<String>,
    is_active: bool,
    quantity: String,
    movement_count: i64,
    entries_total: String,
    exits_total: String,
    reorder_threshold: Option<String>,
}

impl From<StockSheet> for StockSheetDto {
    fn from(sheet: StockSheet) -> Self {
        Self {
            product_id: sheet.product_id,
            name: sheet.name,
            reference: sheet.reference,
            type_name: sheet.type_name,
            supplier_name: sheet.supplier_name,
            is_active: sheet.is_active,
            quantity: format_quantity(sheet.quantity_milli),
            movement_count: sheet.movement_count,
            entries_total: format_quantity(sheet.entries_milli),
            exits_total: format_quantity(sheet.exits_milli),
            reorder_threshold: sheet.reorder_threshold_milli.map(format_quantity),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetQueryDto {
    product_id: i64,
    direction: Option<String>,
    search: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
    page: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetLineDto {
    id: i64,
    created_at: String,
    direction: String,
    quantity: String,
    balance_after: String,
    reason: Option<String>,
    document_reference: Option<String>,
    comment: Option<String>,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetPageDto {
    lines: Vec<SheetLineDto>,
    page: i64,
    page_size: i64,
    total: i64,
}

#[tauri::command]
pub fn get_stock_sheet(app: AppHandle, product_id: i64) -> Result<StockSheetDto, String> {
    let conn = db::open(&app)?;
    gtm_db::get_stock_sheet(&conn, product_id)
        .map(StockSheetDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_stock_sheet_movements(
    app: AppHandle,
    query: SheetQueryDto,
) -> Result<SheetPageDto, String> {
    let conn = db::open(&app)?;
    gtm_db::list_sheet_movements(
        &conn,
        &SheetQuery {
            product_id: query.product_id,
            direction: query.direction,
            search: query.search,
            from_date: query.from_date,
            to_date: query.to_date,
            page: query.page,
        },
    )
    .map(|page| SheetPageDto {
        lines: page
            .lines
            .into_iter()
            .map(|line| SheetLineDto {
                id: line.id,
                created_at: line.created_at,
                direction: line.direction,
                quantity: format_quantity(line.quantity_milli),
                balance_after: format_quantity(line.balance_after_milli),
                reason: line.reason,
                document_reference: line.document_reference,
                comment: line.comment,
                source: line.source,
            })
            .collect(),
        page: page.page,
        page_size: page.page_size,
        total: page.total,
    })
    .map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockOverviewDto {
    active_products: i64,
    out_of_stock: i64,
    reorder_alerts: i64,
    movements_last_7_days: i64,
    recent_movements: Vec<MovementDto>,
}

impl From<StockOverview> for StockOverviewDto {
    fn from(overview: StockOverview) -> Self {
        Self {
            active_products: overview.active_products,
            out_of_stock: overview.out_of_stock,
            reorder_alerts: overview.reorder_alerts,
            movements_last_7_days: overview.movements_last_7_days,
            recent_movements: overview.recent.into_iter().map(MovementDto::from).collect(),
        }
    }
}

#[tauri::command]
pub fn get_stock_overview(app: AppHandle) -> Result<StockOverviewDto, String> {
    let conn = db::open(&app)?;
    gtm_db::stock_overview(&conn)
        .map(StockOverviewDto::from)
        .map_err(|error| error.to_string())
}
