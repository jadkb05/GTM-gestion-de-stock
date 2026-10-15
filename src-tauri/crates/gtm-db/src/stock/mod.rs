mod error;
mod inventory;
mod quantity;
mod sheet;

pub use error::StockError;
pub use inventory::{
    AlertQuery, CountInput, InventoryCounts, InventoryDetail, InventoryDraft, InventoryDrift,
    InventoryLine, InventoryListItem, ReorderAlert, add_inventory_products, check_inventory_drift,
    count_reorder_alerts, create_inventory, get_inventory, list_inventories, list_reorder_alerts,
    refresh_inventory_line, remove_inventory_line, save_inventory_counts, set_reorder_threshold,
    validate_inventory,
};
pub use quantity::{MAX_MILLI, SCALE, format_quantity};
pub use sheet::{
    MovementDetail, PAGE_SIZE, SheetLine, SheetPage, SheetQuery, StockSheet, get_movement_detail,
    get_stock_sheet, list_sheet_movements,
};

use error::unavailable;
use quantity::parse_quantity;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

#[cfg(test)]
mod tests;

const REASON_MAX: usize = 200;
const DOCUMENT_MAX: usize = 64;
const COMMENT_MAX: usize = 2000;
const LIST_LIMIT: i64 = 200;
const RECENT_LIMIT: i64 = 8;

/// Indicateurs du tableau de bord.
///
/// Une rupture est un produit actif dont le stock disponible est égal à zéro.
/// Une alerte est un produit actif dont le seuil est défini et dont le stock
/// est inférieur ou égal à ce seuil. Les deux ensembles ne se confondent pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockOverview {
    pub active_products: i64,
    pub out_of_stock: i64,
    pub reorder_alerts: i64,
    pub movements_last_7_days: i64,
    pub recent: Vec<StockMovement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockLevel {
    pub product_id: i64,
    pub quantity_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductStock {
    pub product_id: i64,
    pub product_name: String,
    pub is_active: bool,
    pub quantity_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementDraft {
    pub product_id: i64,
    pub quantity: String,
    pub reason: Option<String>,
    pub document_reference: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockMovement {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_reference: Option<String>,
    pub direction: String,
    pub quantity_milli: i64,
    pub reason: Option<String>,
    pub document_reference: Option<String>,
    pub comment: Option<String>,
    pub created_at: String,
    pub source: String,
    pub inventory_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementQuery {
    pub search: Option<String>,
    pub product_id: Option<i64>,
    pub direction: Option<String>,
}

struct PreparedMovement {
    product_id: i64,
    quantity_milli: i64,
    reason: Option<String>,
    document_reference: Option<String>,
    comment: Option<String>,
}

pub fn list_stock_levels(conn: &Connection) -> Result<Vec<StockLevel>, StockError> {
    let mut statement = conn
        .prepare("SELECT product_id, quantity_milli FROM stock_balances ORDER BY product_id")
        .map_err(|_| unavailable())?;
    let rows = statement
        .query_map([], |row| {
            Ok(StockLevel {
                product_id: row.get(0)?,
                quantity_milli: row.get(1)?,
            })
        })
        .map_err(|_| unavailable())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| unavailable())?;
    Ok(rows)
}

pub fn get_product_stock(conn: &Connection, product_id: i64) -> Result<ProductStock, StockError> {
    conn.query_row(
        "SELECT p.id, p.name, p.is_active, b.quantity_milli
         FROM products p
         JOIN stock_balances b ON b.product_id = p.id
         WHERE p.id = ?1",
        params![product_id],
        |row| {
            Ok(ProductStock {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                is_active: row.get::<_, i64>(2)? == 1,
                quantity_milli: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|_| unavailable())?
    .ok_or_else(|| StockError::NotFound("Ce produit est introuvable.".into()))
}

pub fn record_entry(
    conn: &mut Connection,
    draft: MovementDraft,
) -> Result<StockMovement, StockError> {
    record(conn, "in", draft)
}

pub fn record_exit(
    conn: &mut Connection,
    draft: MovementDraft,
) -> Result<StockMovement, StockError> {
    record(conn, "out", draft)
}

pub fn stock_overview(conn: &Connection) -> Result<StockOverview, StockError> {
    let (active_products, out_of_stock, reorder_alerts, movements_last_7_days) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM products WHERE is_active = 1),
                (SELECT COUNT(*)
                 FROM products p
                 JOIN stock_balances b ON b.product_id = p.id
                 WHERE p.is_active = 1 AND b.quantity_milli = 0),
                (SELECT COUNT(*)
                 FROM products p
                 JOIN stock_balances b ON b.product_id = p.id
                 WHERE p.is_active = 1
                   AND p.reorder_threshold_milli IS NOT NULL
                   AND b.quantity_milli <= p.reorder_threshold_milli),
                (SELECT COUNT(*)
                 FROM stock_movements
                 WHERE created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-7 days'))",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| unavailable())?;
    let mut statement = conn
        .prepare(
            "SELECT
                m.id, m.product_id, p.name, p.reference, m.direction, m.quantity_milli,
                m.reason, m.document_reference, m.comment, m.created_at, m.source, m.inventory_id
             FROM stock_movements m
             JOIN products p ON p.id = m.product_id
             ORDER BY m.created_at DESC, m.id DESC
             LIMIT ?1",
        )
        .map_err(|_| unavailable())?;
    let recent = statement
        .query_map(params![RECENT_LIMIT], map_movement)
        .map_err(|_| unavailable())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| unavailable())?;
    Ok(StockOverview {
        active_products,
        out_of_stock,
        reorder_alerts,
        movements_last_7_days,
        recent,
    })
}

pub fn list_movements(
    conn: &Connection,
    query: &MovementQuery,
) -> Result<Vec<StockMovement>, StockError> {
    let direction = match query.direction.as_deref().map(str::trim) {
        None | Some("") | Some("all") => None,
        Some("in") => Some("in"),
        Some("out") => Some("out"),
        Some(_) => {
            return Err(StockError::Invalid(
                "Le filtre de mouvement est invalide.".into(),
            ));
        }
    };
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(like_pattern);
    let mut statement = conn
        .prepare(
            "SELECT
                m.id, m.product_id, p.name, p.reference, m.direction, m.quantity_milli,
                m.reason, m.document_reference, m.comment, m.created_at, m.source, m.inventory_id
             FROM stock_movements m
             JOIN products p ON p.id = m.product_id
             WHERE (?1 IS NULL OR m.product_id = ?1)
               AND (?2 IS NULL OR m.direction = ?2)
               AND (
                    ?3 IS NULL
                    OR p.name LIKE ?3 ESCAPE '\\'
                    OR IFNULL(p.reference, '') LIKE ?3 ESCAPE '\\'
                    OR IFNULL(m.reason, '') LIKE ?3 ESCAPE '\\'
                    OR IFNULL(m.document_reference, '') LIKE ?3 ESCAPE '\\'
               )
             ORDER BY m.created_at DESC, m.id DESC
             LIMIT ?4",
        )
        .map_err(|_| unavailable())?;
    let rows = statement
        .query_map(
            params![query.product_id, direction, search, LIST_LIMIT],
            map_movement,
        )
        .map_err(|_| unavailable())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| unavailable())?;
    Ok(rows)
}

pub fn get_movement(conn: &Connection, id: i64) -> Result<StockMovement, StockError> {
    conn.query_row(
        "SELECT
            m.id, m.product_id, p.name, p.reference, m.direction, m.quantity_milli,
            m.reason, m.document_reference, m.comment, m.created_at, m.source, m.inventory_id
         FROM stock_movements m
         JOIN products p ON p.id = m.product_id
         WHERE m.id = ?1",
        params![id],
        map_movement,
    )
    .optional()
    .map_err(|_| unavailable())?
    .ok_or_else(|| StockError::NotFound("Ce mouvement est introuvable.".into()))
}

fn record(
    conn: &mut Connection,
    direction: &str,
    draft: MovementDraft,
) -> Result<StockMovement, StockError> {
    let prepared = prepare_draft(draft)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| unavailable())?;
    ensure_active_product(&tx, prepared.product_id)?;
    let current = balance_of(&tx, prepared.product_id)?;
    let next = match direction {
        "in" => current
            .checked_add(prepared.quantity_milli)
            .ok_or_else(|| {
                StockError::Invalid("La quantité dépasse la limite autorisée.".into())
            })?,
        "out" => {
            if current < prepared.quantity_milli {
                return Err(StockError::Insufficient(
                    "Le stock disponible est insuffisant.".into(),
                ));
            }
            current - prepared.quantity_milli
        }
        _ => return Err(unavailable()),
    };
    if next > MAX_MILLI {
        return Err(StockError::Invalid(
            "La quantité dépasse la limite autorisée.".into(),
        ));
    }

    let changed = if direction == "out" {
        tx.execute(
            "UPDATE stock_balances
             SET quantity_milli = quantity_milli - ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE product_id = ?2 AND quantity_milli >= ?1",
            params![prepared.quantity_milli, prepared.product_id],
        )
        .map_err(|_| unavailable())?
    } else {
        tx.execute(
            "UPDATE stock_balances
             SET quantity_milli = quantity_milli + ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE product_id = ?2 AND quantity_milli + ?1 <= ?3",
            params![prepared.quantity_milli, prepared.product_id, MAX_MILLI],
        )
        .map_err(|_| unavailable())?
    };
    if changed != 1 {
        return Err(if direction == "out" {
            StockError::Insufficient("Le stock disponible est insuffisant.".into())
        } else {
            StockError::Invalid("La quantité dépasse la limite autorisée.".into())
        });
    }

    tx.execute(
        "INSERT INTO stock_movements (
            product_id, direction, quantity_milli, reason, document_reference, comment, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![
            prepared.product_id,
            direction,
            prepared.quantity_milli,
            prepared.reason,
            prepared.document_reference,
            prepared.comment
        ],
    )
    .map_err(|_| unavailable())?;
    let id = tx.last_insert_rowid();
    let movement = get_movement(&tx, id)?;
    tx.commit().map_err(|_| unavailable())?;
    Ok(movement)
}

fn prepare_draft(draft: MovementDraft) -> Result<PreparedMovement, StockError> {
    if draft.product_id < 1 {
        return Err(StockError::NotFound("Ce produit est introuvable.".into()));
    }
    Ok(PreparedMovement {
        product_id: draft.product_id,
        quantity_milli: parse_quantity(&draft.quantity)?,
        reason: optional_text(
            draft.reason.as_deref(),
            REASON_MAX,
            "Le motif dépasse 200 caractères.",
        )?,
        document_reference: optional_text(
            draft.document_reference.as_deref(),
            DOCUMENT_MAX,
            "La référence documentaire dépasse 64 caractères.",
        )?,
        comment: optional_text(
            draft.comment.as_deref(),
            COMMENT_MAX,
            "Le commentaire dépasse 2 000 caractères.",
        )?,
    })
}

fn optional_text(
    value: Option<&str>,
    max: usize,
    message: &str,
) -> Result<Option<String>, StockError> {
    match value.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(None),
        Some(text) if text.chars().count() > max => Err(StockError::Invalid(message.into())),
        Some(text) => Ok(Some(text.to_string())),
    }
}

fn ensure_active_product(conn: &Connection, product_id: i64) -> Result<(), StockError> {
    let active: Option<i64> = conn
        .query_row(
            "SELECT is_active FROM products WHERE id = ?1",
            params![product_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| unavailable())?;
    match active {
        None => Err(StockError::NotFound("Ce produit est introuvable.".into())),
        Some(0) => Err(StockError::Inactive("Ce produit est désactivé.".into())),
        Some(_) => Ok(()),
    }
}

fn balance_of(conn: &Connection, product_id: i64) -> Result<i64, StockError> {
    conn.query_row(
        "SELECT quantity_milli FROM stock_balances WHERE product_id = ?1",
        params![product_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|_| unavailable())?
    .ok_or_else(|| StockError::NotFound("Le stock de ce produit est introuvable.".into()))
}

fn map_movement(row: &Row<'_>) -> rusqlite::Result<StockMovement> {
    Ok(StockMovement {
        id: row.get(0)?,
        product_id: row.get(1)?,
        product_name: row.get(2)?,
        product_reference: row.get(3)?,
        direction: row.get(4)?,
        quantity_milli: row.get(5)?,
        reason: row.get(6)?,
        document_reference: row.get(7)?,
        comment: row.get(8)?,
        created_at: row.get(9)?,
        source: row.get(10)?,
        inventory_id: row.get(11)?,
    })
}

pub(super) fn like_pattern(search: &str) -> String {
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}
