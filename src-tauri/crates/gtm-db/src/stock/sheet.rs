use super::error::{StockError, unavailable};
use super::{StockMovement, like_pattern};
use rusqlite::{Connection, OptionalExtension, params};

pub const PAGE_SIZE: i64 = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockSheet {
    pub product_id: i64,
    pub name: String,
    pub reference: Option<String>,
    pub type_name: String,
    pub supplier_name: Option<String>,
    pub is_active: bool,
    pub quantity_milli: i64,
    pub movement_count: i64,
    pub entries_milli: i64,
    pub exits_milli: i64,
    pub reorder_threshold_milli: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetQuery {
    pub product_id: i64,
    pub direction: Option<String>,
    pub search: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub page: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetLine {
    pub id: i64,
    pub created_at: String,
    pub direction: String,
    pub quantity_milli: i64,
    pub balance_after_milli: i64,
    pub reason: Option<String>,
    pub document_reference: Option<String>,
    pub comment: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPage {
    pub lines: Vec<SheetLine>,
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementDetail {
    pub movement: StockMovement,
    pub balance_after_milli: i64,
}

pub fn get_stock_sheet(conn: &Connection, product_id: i64) -> Result<StockSheet, StockError> {
    let row = conn
        .query_row(
            "SELECT p.id, p.name, p.reference, t.name, s.name, p.is_active, b.quantity_milli,
                    p.reorder_threshold_milli
             FROM products p
             JOIN product_types t ON t.id = p.type_id
             LEFT JOIN suppliers s ON s.id = p.supplier_id
             LEFT JOIN stock_balances b ON b.product_id = p.id
             WHERE p.id = ?1",
            params![product_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )
        .optional()
        .map_err(|_| unavailable())?;
    let Some((id, name, reference, type_name, supplier_name, active, quantity, threshold)) = row
    else {
        return Err(StockError::NotFound("Ce produit est introuvable.".into()));
    };
    let Some(quantity_milli) = quantity else {
        return Err(StockError::NotFound(
            "Le stock de ce produit est introuvable.".into(),
        ));
    };
    let (movement_count, entries_milli, exits_milli) = conn
        .query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN direction = 'in' THEN quantity_milli END), 0),
                COALESCE(SUM(CASE WHEN direction = 'out' THEN quantity_milli END), 0)
             FROM stock_movements
             WHERE product_id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| unavailable())?;
    Ok(StockSheet {
        product_id: id,
        name,
        reference,
        type_name,
        supplier_name,
        is_active: active == 1,
        quantity_milli,
        movement_count,
        entries_milli,
        exits_milli,
        reorder_threshold_milli: threshold,
    })
}

pub fn list_sheet_movements(
    conn: &Connection,
    query: &SheetQuery,
) -> Result<SheetPage, StockError> {
    if query.product_id < 1 {
        return Err(StockError::NotFound("Ce produit est introuvable.".into()));
    }
    let direction = parse_direction(query.direction.as_deref())?;
    let from_date = parse_date(query.from_date.as_deref())?;
    let to_date = parse_date(query.to_date.as_deref())?;
    if let (Some(start), Some(end)) = (&from_date, &to_date) {
        if end < start {
            return Err(StockError::Invalid(
                "La date de fin précède la date de début.".into(),
            ));
        }
    }
    if query.page < 1 || query.page > 10_000 {
        return Err(StockError::Invalid(
            "Le numéro de page est invalide.".into(),
        ));
    }
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(like_pattern);
    let offset = (query.page - 1) * PAGE_SIZE;
    let total: i64 = conn
        .query_row(
            &format!("{LEDGER} SELECT COUNT(*) FROM ledger {FILTER}"),
            params![query.product_id, direction, from_date, to_date, search],
            |row| row.get(0),
        )
        .map_err(|_| unavailable())?;
    let mut statement = conn
        .prepare(&format!(
            "{LEDGER}
             SELECT id, created_at, direction, quantity_milli, balance_after_milli,
                    reason, document_reference, comment, source
             FROM ledger
             {FILTER}
             ORDER BY created_at DESC, id DESC
             LIMIT ?6 OFFSET ?7"
        ))
        .map_err(|_| unavailable())?;
    let lines = statement
        .query_map(
            params![
                query.product_id,
                direction,
                from_date,
                to_date,
                search,
                PAGE_SIZE,
                offset
            ],
            |row| {
                Ok(SheetLine {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    direction: row.get(2)?,
                    quantity_milli: row.get(3)?,
                    balance_after_milli: row.get(4)?,
                    reason: row.get(5)?,
                    document_reference: row.get(6)?,
                    comment: row.get(7)?,
                    source: row.get(8)?,
                })
            },
        )
        .map_err(|_| unavailable())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| unavailable())?;
    Ok(SheetPage {
        lines,
        page: query.page,
        page_size: PAGE_SIZE,
        total,
    })
}

pub fn get_movement_detail(conn: &Connection, id: i64) -> Result<MovementDetail, StockError> {
    conn.query_row(
        "WITH ledger AS (
            SELECT
                id,
                SUM(CASE WHEN direction = 'in' THEN quantity_milli ELSE -quantity_milli END)
                    OVER (
                        PARTITION BY product_id
                        ORDER BY created_at ASC, id ASC
                        ROWS UNBOUNDED PRECEDING
                    ) AS balance_after_milli
            FROM stock_movements
         )
         SELECT
            m.id, m.product_id, p.name, p.reference, m.direction, m.quantity_milli,
            m.reason, m.document_reference, m.comment, m.created_at, ledger.balance_after_milli,
            m.source, m.inventory_id
         FROM stock_movements m
         JOIN products p ON p.id = m.product_id
         JOIN ledger ON ledger.id = m.id
         WHERE m.id = ?1",
        params![id],
        |row| {
            Ok(MovementDetail {
                movement: StockMovement {
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
                    source: row.get(11)?,
                    inventory_id: row.get(12)?,
                },
                balance_after_milli: row.get(10)?,
            })
        },
    )
    .optional()
    .map_err(|_| unavailable())?
    .ok_or_else(|| StockError::NotFound("Ce mouvement est introuvable.".into()))
}

const LEDGER: &str = "
    WITH ledger AS (
        SELECT
            id, created_at, direction, quantity_milli, reason, document_reference, comment, source,
            SUM(CASE WHEN direction = 'in' THEN quantity_milli ELSE -quantity_milli END)
                OVER (ORDER BY created_at ASC, id ASC ROWS UNBOUNDED PRECEDING)
                AS balance_after_milli
        FROM stock_movements
        WHERE product_id = ?1
    )
";

const FILTER: &str = "
    WHERE (?2 IS NULL OR direction = ?2)
      AND (?3 IS NULL OR date(created_at, 'localtime') >= ?3)
      AND (?4 IS NULL OR date(created_at, 'localtime') <= ?4)
      AND (
           ?5 IS NULL
           OR IFNULL(reason, '') LIKE ?5 ESCAPE '\\'
           OR IFNULL(document_reference, '') LIKE ?5 ESCAPE '\\'
      )
";

fn parse_direction(value: Option<&str>) -> Result<Option<&str>, StockError> {
    match value.map(str::trim) {
        None | Some("") | Some("all") => Ok(None),
        Some("in") => Ok(Some("in")),
        Some("out") => Ok(Some("out")),
        Some(_) => Err(StockError::Invalid(
            "Le filtre de mouvement est invalide.".into(),
        )),
    }
}

fn parse_date(value: Option<&str>) -> Result<Option<String>, StockError> {
    let Some(raw) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    let mut parts = raw.split('-');
    let year = parts.next().and_then(|part| part.parse::<i32>().ok());
    let month = parts.next().and_then(|part| part.parse::<u32>().ok());
    let day = parts.next().and_then(|part| part.parse::<u32>().ok());
    let (Some(year), Some(month), Some(day), None) = (year, month, day, parts.next()) else {
        return Err(StockError::Invalid("La date est invalide.".into()));
    };
    if raw.len() != 10 || !(1..=9999).contains(&year) || !valid_day(year, month, day) {
        return Err(StockError::Invalid("La date est invalide.".into()));
    }
    Ok(Some(raw.to_string()))
}

fn valid_day(year: i32, month: u32, day: u32) -> bool {
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    (1..=max).contains(&day)
}

#[cfg(test)]
#[path = "sheet_tests.rs"]
mod tests;
