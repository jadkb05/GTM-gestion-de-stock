//! Inventaire physique et seuils de réapprovisionnement.
//!
//! L'instantané du stock théorique (`theoretical_milli`) est copié depuis
//! `stock_balances` au moment où le produit est ajouté à l'inventaire, et
//! seulement à ce moment-là. Un mouvement ultérieur ne le modifie pas.
//! La validation relit le solde dans la même transaction et refuse tout
//! l'inventaire si un instantané ne correspond plus au solde courant.
//! Réactualiser la référence est une action explicite : elle remplace
//! l'instantané par le solde du moment, sans créer de mouvement.

use super::error::{StockError, unavailable};
use super::like_pattern;
use super::quantity::{MAX_MILLI, parse_non_negative};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

const NAME_MAX: usize = 120;
const COMMENT_MAX: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryDraft {
    pub name: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryListItem {
    pub id: i64,
    pub name: Option<String>,
    pub status: String,
    pub created_at: String,
    pub validated_at: Option<String>,
    pub product_count: i64,
    pub variance_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryLine {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_reference: Option<String>,
    pub type_name: String,
    pub theoretical_milli: i64,
    pub counted_milli: Option<i64>,
    pub current_milli: i64,
    pub variance_milli: Option<i64>,
    pub drifted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventoryCounts {
    pub counted: i64,
    pub unchanged: i64,
    pub surplus: i64,
    pub shortage: i64,
    pub uncounted: i64,
    pub stale: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryDetail {
    pub id: i64,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub status: String,
    pub created_at: String,
    pub validated_at: Option<String>,
    pub lines: Vec<InventoryLine>,
    pub counts: InventoryCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountInput {
    pub line_id: i64,
    pub quantity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryDrift {
    pub line_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub theoretical_milli: i64,
    pub current_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReorderAlert {
    pub product_id: i64,
    pub name: String,
    pub reference: Option<String>,
    pub type_id: i64,
    pub type_name: String,
    pub supplier_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub quantity_milli: i64,
    pub threshold_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertQuery {
    pub search: Option<String>,
    pub type_id: Option<i64>,
    pub supplier_id: Option<i64>,
    pub sort: Option<String>,
}

pub fn list_inventories(conn: &Connection) -> Result<Vec<InventoryListItem>, StockError> {
    let mut statement = conn
        .prepare(
            "SELECT
                i.id, i.name, i.status, i.created_at, i.validated_at,
                (SELECT COUNT(*) FROM inventory_lines l WHERE l.inventory_id = i.id),
                (SELECT COUNT(*)
                 FROM inventory_lines l
                 JOIN stock_balances b ON b.product_id = l.product_id
                 WHERE l.inventory_id = i.id
                   AND l.counted_milli IS NOT NULL
                   AND l.counted_milli <> l.theoretical_milli
                   AND (i.status = 'validated' OR b.quantity_milli = l.theoretical_milli))
             FROM inventories i
             ORDER BY i.created_at DESC, i.id DESC",
        )
        .map_err(|_| failed())?;
    let rows = statement
        .query_map([], |row| {
            Ok(InventoryListItem {
                id: row.get(0)?,
                name: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                validated_at: row.get(4)?,
                product_count: row.get(5)?,
                variance_count: row.get(6)?,
            })
        })
        .map_err(|_| failed())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| failed())?;
    Ok(rows)
}

pub fn create_inventory(
    conn: &mut Connection,
    draft: InventoryDraft,
) -> Result<InventoryDetail, StockError> {
    let name = optional_text(draft.name, NAME_MAX, "Le nom dépasse 120 caractères.")?;
    let comment = optional_text(
        draft.comment,
        COMMENT_MAX,
        "Le commentaire dépasse 2 000 caractères.",
    )?;
    let tx = conn.transaction().map_err(|_| failed())?;
    tx.execute(
        "INSERT INTO inventories (name, comment, status, created_at, validated_at)
         VALUES (?1, ?2, 'draft', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), NULL)",
        params![name, comment],
    )
    .map_err(|_| failed())?;
    let id = tx.last_insert_rowid();
    tx.commit().map_err(|_| failed())?;
    get_inventory(conn, id)
}

pub fn get_inventory(conn: &Connection, id: i64) -> Result<InventoryDetail, StockError> {
    if id < 1 {
        return Err(StockError::NotFound(
            "Cet inventaire est introuvable.".into(),
        ));
    }
    let header = conn
        .query_row(
            "SELECT id, name, comment, status, created_at, validated_at
             FROM inventories WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|_| failed())?
        .ok_or_else(|| StockError::NotFound("Cet inventaire est introuvable.".into()))?;
    // Après validation, l'instantané est l'historique du comptage : le solde
    // courant a été corrigé et ne signale plus une référence périmée.
    let frozen = header.3 == "validated";
    let lines = load_lines(conn, id, frozen)?;
    let counts = summarize(&lines);
    Ok(InventoryDetail {
        id: header.0,
        name: header.1,
        comment: header.2,
        status: header.3,
        created_at: header.4,
        validated_at: header.5,
        lines,
        counts,
    })
}

pub fn add_inventory_products(
    conn: &mut Connection,
    inventory_id: i64,
    product_ids: &[i64],
) -> Result<InventoryDetail, StockError> {
    if product_ids.is_empty() {
        return Err(StockError::Invalid(
            "Sélectionnez au moins un produit.".into(),
        ));
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed())?;
    lock_draft(&tx, inventory_id)?;
    for product_id in product_ids {
        add_one(&tx, inventory_id, *product_id)?;
    }
    tx.commit().map_err(|_| failed())?;
    get_inventory(conn, inventory_id)
}

pub fn save_inventory_counts(
    conn: &mut Connection,
    inventory_id: i64,
    counts: &[CountInput],
) -> Result<InventoryDetail, StockError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed())?;
    lock_draft(&tx, inventory_id)?;
    for count in counts {
        let milli = match count.quantity.as_deref() {
            None => None,
            Some(value) if value.trim().is_empty() => None,
            Some(value) => Some(parse_non_negative(value)?),
        };
        let changed = tx
            .execute(
                "UPDATE inventory_lines
                 SET counted_milli = ?1
                 WHERE id = ?2 AND inventory_id = ?3",
                params![milli, count.line_id, inventory_id],
            )
            .map_err(|_| failed())?;
        if changed != 1 {
            return Err(StockError::NotFound(
                "Cette ligne d'inventaire est introuvable.".into(),
            ));
        }
    }
    tx.commit().map_err(|_| failed())?;
    get_inventory(conn, inventory_id)
}

pub fn check_inventory_drift(
    conn: &Connection,
    inventory_id: i64,
) -> Result<Vec<InventoryDrift>, StockError> {
    let detail = get_inventory(conn, inventory_id)?;
    Ok(detail
        .lines
        .into_iter()
        .filter(|line| line.drifted)
        .map(|line| InventoryDrift {
            line_id: line.id,
            product_id: line.product_id,
            product_name: line.product_name,
            theoretical_milli: line.theoretical_milli,
            current_milli: line.current_milli,
        })
        .collect())
}

/// Remplace l'instantané par le solde courant.
/// `reset_count` efface la quantité comptée pour reprendre le comptage.
pub fn refresh_inventory_line(
    conn: &mut Connection,
    line_id: i64,
    reset_count: bool,
) -> Result<InventoryDetail, StockError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed())?;
    let (inventory_id, product_id) = draft_line(&tx, line_id)?;
    let current = balance_of(&tx, product_id)?;
    let changed = tx
        .execute(
            "UPDATE inventory_lines
             SET theoretical_milli = ?1,
                 snapshot_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 counted_milli = CASE WHEN ?2 THEN NULL ELSE counted_milli END
             WHERE id = ?3",
            params![current, reset_count, line_id],
        )
        .map_err(|_| failed())?;
    if changed != 1 {
        return Err(failed());
    }
    tx.commit().map_err(|_| failed())?;
    get_inventory(conn, inventory_id)
}

pub fn remove_inventory_line(
    conn: &mut Connection,
    line_id: i64,
) -> Result<InventoryDetail, StockError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed())?;
    let (inventory_id, _) = draft_line(&tx, line_id)?;
    let changed = tx
        .execute(
            "DELETE FROM inventory_lines WHERE id = ?1",
            params![line_id],
        )
        .map_err(|_| failed())?;
    if changed != 1 {
        return Err(failed());
    }
    tx.commit().map_err(|_| failed())?;
    get_inventory(conn, inventory_id)
}

pub fn validate_inventory(conn: &mut Connection, id: i64) -> Result<InventoryDetail, StockError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed())?;
    validate_in_transaction(&tx, id)?;
    tx.commit().map_err(|_| failed())?;
    get_inventory(conn, id)
}

pub fn list_reorder_alerts(
    conn: &Connection,
    query: &AlertQuery,
) -> Result<Vec<ReorderAlert>, StockError> {
    let sort = match query.sort.as_deref().map(str::trim) {
        None | Some("") | Some("asc") => "ASC",
        Some("desc") => "DESC",
        Some(_) => {
            return Err(StockError::Invalid(
                "Le tri demandé n'est pas reconnu.".into(),
            ));
        }
    };
    if let Some(type_id) = query.type_id {
        if type_id < 1 {
            return Err(StockError::Invalid("Le type demandé est invalide.".into()));
        }
    }
    if let Some(supplier_id) = query.supplier_id {
        if supplier_id < 1 {
            return Err(StockError::Invalid(
                "Le fournisseur demandé est invalide.".into(),
            ));
        }
    }
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(like_pattern);
    let sql = format!(
        "SELECT
            p.id, p.name, p.reference, t.id, t.name, s.id, s.name,
            b.quantity_milli, p.reorder_threshold_milli
         FROM products p
         JOIN product_types t ON t.id = p.type_id
         LEFT JOIN suppliers s ON s.id = p.supplier_id
         JOIN stock_balances b ON b.product_id = p.id
         WHERE p.is_active = 1
           AND p.reorder_threshold_milli IS NOT NULL
           AND b.quantity_milli <= p.reorder_threshold_milli
           AND (?1 IS NULL OR p.type_id = ?1)
           AND (?2 IS NULL OR p.supplier_id = ?2)
           AND (
                ?3 IS NULL
                OR p.name LIKE ?3 ESCAPE '\\'
                OR IFNULL(p.reference, '') LIKE ?3 ESCAPE '\\'
           )
         ORDER BY b.quantity_milli {sort}, p.name COLLATE NOCASE, p.id"
    );
    let mut statement = conn.prepare(&sql).map_err(|_| failed())?;
    let rows = statement
        .query_map(params![query.type_id, query.supplier_id, search], |row| {
            Ok(ReorderAlert {
                product_id: row.get(0)?,
                name: row.get(1)?,
                reference: row.get(2)?,
                type_id: row.get(3)?,
                type_name: row.get(4)?,
                supplier_id: row.get(5)?,
                supplier_name: row.get(6)?,
                quantity_milli: row.get(7)?,
                threshold_milli: row.get(8)?,
            })
        })
        .map_err(|_| failed())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| failed())?;
    Ok(rows)
}

pub fn count_reorder_alerts(conn: &Connection) -> Result<i64, StockError> {
    conn.query_row(
        "SELECT COUNT(*)
         FROM products p
         JOIN stock_balances b ON b.product_id = p.id
         WHERE p.is_active = 1
           AND p.reorder_threshold_milli IS NOT NULL
           AND b.quantity_milli <= p.reorder_threshold_milli",
        [],
        |row| row.get(0),
    )
    .map_err(|_| failed())
}

/// `threshold` absent retire le seuil. Un seuil nul est conservé.
pub fn set_reorder_threshold(
    conn: &mut Connection,
    product_id: i64,
    threshold: Option<&str>,
) -> Result<Option<i64>, StockError> {
    if product_id < 1 || !product_exists(conn, product_id)? {
        return Err(StockError::NotFound("Ce produit est introuvable.".into()));
    }
    let milli = match threshold {
        None => None,
        Some(value) => Some(parse_threshold(value)?),
    };
    let tx = conn.transaction().map_err(|_| failed())?;
    let changed = tx
        .execute(
            "UPDATE products
             SET reorder_threshold_milli = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
            params![milli, product_id],
        )
        .map_err(|_| failed())?;
    if changed != 1 {
        return Err(StockError::NotFound("Ce produit est introuvable.".into()));
    }
    tx.commit().map_err(|_| failed())?;
    Ok(milli)
}

fn validate_in_transaction(tx: &Transaction<'_>, id: i64) -> Result<(), StockError> {
    let status: Option<String> = tx
        .query_row(
            "SELECT status FROM inventories WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| failed())?;
    match status.as_deref() {
        Some("validated") => {
            return Err(StockError::Invalid(
                "Cet inventaire est déjà validé.".into(),
            ));
        }
        Some("draft") => {}
        Some(_) => return Err(failed()),
        None => {
            return Err(StockError::NotFound(
                "Cet inventaire est introuvable.".into(),
            ));
        }
    }
    let name: Option<String> = tx
        .query_row(
            "SELECT name FROM inventories WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| failed())?;
    let lines = load_lines(tx, id, false)?;
    if lines.is_empty() {
        return Err(StockError::Invalid(
            "Ajoutez au moins un produit à inventorier.".into(),
        ));
    }
    for line in &lines {
        if line.counted_milli.is_none() {
            return Err(StockError::Invalid(
                "Indiquez la quantité comptée pour chaque produit avant de valider.".into(),
            ));
        }
        if line.drifted {
            return Err(StockError::Invalid(format!(
                "Le stock de « {} » a changé depuis le comptage. Réactualisez la référence avant de valider.",
                line.product_name
            )));
        }
        let active: i64 = tx
            .query_row(
                "SELECT is_active FROM products WHERE id = ?1",
                params![line.product_id],
                |row| row.get(0),
            )
            .map_err(|_| failed())?;
        if active != 1 {
            return Err(StockError::Inactive(format!(
                "Le produit « {} » est désactivé. Réactivez-le ou retirez-le de l'inventaire avant de valider.",
                line.product_name
            )));
        }
    }
    let document = document_reference(&name, id);
    for line in &lines {
        let counted = line.counted_milli.expect("comptage vérifié");
        let variance = counted - line.theoretical_milli;
        if variance == 0 {
            continue;
        }
        apply_correction(tx, id, line, variance, &document)?;
    }
    let changed = tx
        .execute(
            "UPDATE inventories
             SET status = 'validated',
                 validated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status = 'draft'",
            params![id],
        )
        .map_err(|_| failed())?;
    if changed != 1 {
        return Err(StockError::Invalid(
            "Cet inventaire est déjà validé.".into(),
        ));
    }
    Ok(())
}

fn apply_correction(
    tx: &Transaction<'_>,
    inventory_id: i64,
    line: &InventoryLine,
    variance: i64,
    document: &str,
) -> Result<(), StockError> {
    let (direction, quantity) = if variance > 0 {
        ("in", variance)
    } else {
        ("out", -variance)
    };
    if quantity > MAX_MILLI {
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
            params![quantity, line.product_id],
        )
        .map_err(|_| failed())?
    } else {
        tx.execute(
            "UPDATE stock_balances
             SET quantity_milli = quantity_milli + ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE product_id = ?2 AND quantity_milli + ?1 <= ?3",
            params![quantity, line.product_id, MAX_MILLI],
        )
        .map_err(|_| failed())?
    };
    if changed != 1 {
        return Err(StockError::Invalid(
            "La correction de stock n'a pas pu être appliquée.".into(),
        ));
    }
    tx.execute(
        "INSERT INTO stock_movements (
            product_id, direction, quantity_milli, reason, document_reference, comment, created_at,
            source, inventory_id, inventory_line_id, variance_milli
         ) VALUES (
            ?1, ?2, ?3, 'Correction d''inventaire', ?4, NULL,
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            'inventory', ?5, ?6, ?7
         )",
        params![
            line.product_id,
            direction,
            quantity,
            document,
            inventory_id,
            line.id,
            variance
        ],
    )
    .map_err(|_| failed())?;
    Ok(())
}

fn add_one(tx: &Transaction<'_>, inventory_id: i64, product_id: i64) -> Result<(), StockError> {
    if product_id < 1 {
        return Err(StockError::NotFound("Ce produit est introuvable.".into()));
    }
    let product: Option<(i64, i64)> = tx
        .query_row(
            "SELECT p.is_active, b.quantity_milli
             FROM products p
             JOIN stock_balances b ON b.product_id = p.id
             WHERE p.id = ?1",
            params![product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| failed())?;
    let Some((active, quantity)) = product else {
        return Err(StockError::NotFound("Ce produit est introuvable.".into()));
    };
    if active != 1 {
        return Err(StockError::Inactive("Ce produit est désactivé.".into()));
    }
    let exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM inventory_lines WHERE inventory_id = ?1 AND product_id = ?2",
            params![inventory_id, product_id],
            |row| row.get(0),
        )
        .map_err(|_| failed())?;
    if exists != 0 {
        return Err(StockError::Invalid(
            "Ce produit est déjà dans l'inventaire.".into(),
        ));
    }
    tx.execute(
        "INSERT INTO inventory_lines (
            inventory_id, product_id, theoretical_milli, counted_milli, snapshot_at
         ) VALUES (?1, ?2, ?3, NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![inventory_id, product_id, quantity],
    )
    .map_err(|_| failed())?;
    Ok(())
}

fn load_lines(
    conn: &Connection,
    inventory_id: i64,
    frozen: bool,
) -> Result<Vec<InventoryLine>, StockError> {
    let mut statement = conn
        .prepare(
            "SELECT
                l.id, l.product_id, p.name, p.reference, t.name,
                l.theoretical_milli, l.counted_milli, b.quantity_milli
             FROM inventory_lines l
             JOIN products p ON p.id = l.product_id
             JOIN product_types t ON t.id = p.type_id
             JOIN stock_balances b ON b.product_id = l.product_id
             WHERE l.inventory_id = ?1
             ORDER BY p.name COLLATE NOCASE, l.id",
        )
        .map_err(|_| failed())?;
    let rows = statement
        .query_map(params![inventory_id], |row| {
            let theoretical: i64 = row.get(5)?;
            let counted: Option<i64> = row.get(6)?;
            let current: i64 = row.get(7)?;
            Ok(InventoryLine {
                id: row.get(0)?,
                product_id: row.get(1)?,
                product_name: row.get(2)?,
                product_reference: row.get(3)?,
                type_name: row.get(4)?,
                theoretical_milli: theoretical,
                counted_milli: counted,
                current_milli: current,
                variance_milli: counted.map(|value| value - theoretical),
                drifted: !frozen && current != theoretical,
            })
        })
        .map_err(|_| failed())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| failed())?;
    Ok(rows)
}

fn summarize(lines: &[InventoryLine]) -> InventoryCounts {
    let mut counts = InventoryCounts::default();
    for line in lines {
        match line.counted_milli {
            None => counts.uncounted += 1,
            Some(counted) => {
                counts.counted += 1;
                if line.drifted {
                    counts.stale += 1;
                } else if counted == line.theoretical_milli {
                    counts.unchanged += 1;
                } else if counted > line.theoretical_milli {
                    counts.surplus += 1;
                } else {
                    counts.shortage += 1;
                }
            }
        }
    }
    counts
}

fn lock_draft(tx: &Transaction<'_>, id: i64) -> Result<(), StockError> {
    let status: Option<String> = tx
        .query_row(
            "SELECT status FROM inventories WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| failed())?;
    match status.as_deref() {
        Some("draft") => Ok(()),
        Some("validated") => Err(StockError::Invalid(
            "Un inventaire validé ne peut plus être modifié.".into(),
        )),
        Some(_) => Err(failed()),
        None => Err(StockError::NotFound(
            "Cet inventaire est introuvable.".into(),
        )),
    }
}

fn draft_line(tx: &Transaction<'_>, line_id: i64) -> Result<(i64, i64), StockError> {
    let row: Option<(i64, String, i64)> = tx
        .query_row(
            "SELECT i.id, i.status, l.product_id
             FROM inventory_lines l
             JOIN inventories i ON i.id = l.inventory_id
             WHERE l.id = ?1",
            params![line_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|_| failed())?;
    let Some((inventory_id, status, product_id)) = row else {
        return Err(StockError::NotFound(
            "Cette ligne d'inventaire est introuvable.".into(),
        ));
    };
    if status != "draft" {
        return Err(StockError::Invalid(
            "Un inventaire validé ne peut plus être modifié.".into(),
        ));
    }
    Ok((inventory_id, product_id))
}

fn balance_of(conn: &Connection, product_id: i64) -> Result<i64, StockError> {
    conn.query_row(
        "SELECT quantity_milli FROM stock_balances WHERE product_id = ?1",
        params![product_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|_| failed())?
    .ok_or_else(|| StockError::NotFound("Le stock de ce produit est introuvable.".into()))
}

fn product_exists(conn: &Connection, product_id: i64) -> Result<bool, StockError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM products WHERE id = ?1",
            params![product_id],
            |row| row.get(0),
        )
        .map_err(|_| failed())?;
    Ok(count == 1)
}

fn optional_text(
    value: Option<String>,
    max: usize,
    too_long: &str,
) -> Result<Option<String>, StockError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > max {
        return Err(StockError::Invalid(too_long.into()));
    }
    Ok(Some(trimmed.to_string()))
}

fn document_reference(name: &Option<String>, id: i64) -> String {
    match name {
        Some(name) if name.chars().count() <= 64 => name.clone(),
        _ => format!("Inventaire {id}"),
    }
}

fn parse_threshold(raw: &str) -> Result<i64, StockError> {
    parse_non_negative(raw).map_err(|error| {
        let message = match error.to_string().as_str() {
            "Indiquez une quantité." => "Indiquez un seuil.".to_string(),
            "La quantité doit être positive ou nulle." => {
                "Le seuil doit être positif ou nul.".to_string()
            }
            "La quantité n'est pas un nombre valide." => {
                "Le seuil n'est pas un nombre valide.".to_string()
            }
            "La quantité dépasse 3 décimales." => "Le seuil dépasse 3 décimales.".to_string(),
            "La quantité dépasse la limite autorisée." => {
                "Le seuil dépasse la limite autorisée.".to_string()
            }
            other => other.to_string(),
        };
        StockError::Invalid(message)
    })
}

fn failed() -> StockError {
    unavailable()
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
