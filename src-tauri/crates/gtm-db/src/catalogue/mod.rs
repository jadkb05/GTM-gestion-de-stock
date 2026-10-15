mod error;
mod validate;

pub use error::CatalogueError;
use error::unavailable;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use validate::{NormalizedProduct, normalize_label, normalize_product, same_name};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductType {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supplier {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub reference: Option<String>,
    pub type_id: i64,
    pub type_name: String,
    pub supplier_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDraft {
    pub name: String,
    pub reference: Option<String>,
    pub type_id: i64,
    pub supplier_id: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityFilter {
    Active,
    Inactive,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductQuery {
    pub search: Option<String>,
    pub type_id: Option<i64>,
    pub supplier_id: Option<i64>,
    pub status: ActivityFilter,
}

const PRODUCT_SQL: &str = "
    SELECT
        p.id, p.name, p.reference, p.type_id, t.name,
        p.supplier_id, s.name, p.description, p.is_active,
        p.created_at, p.updated_at
    FROM products p
    JOIN product_types t ON t.id = p.type_id
    LEFT JOIN suppliers s ON s.id = p.supplier_id
";

pub fn list_product_types(
    conn: &Connection,
    include_inactive: bool,
) -> Result<Vec<ProductType>, CatalogueError> {
    let sql = if include_inactive {
        "SELECT id, name, is_active, created_at, updated_at
         FROM product_types ORDER BY name COLLATE NOCASE, id"
    } else {
        "SELECT id, name, is_active, created_at, updated_at
         FROM product_types WHERE is_active = 1 ORDER BY name COLLATE NOCASE, id"
    };
    let mut statement = conn.prepare(sql).map_err(unavailable)?;
    let rows = statement
        .query_map([], map_type)
        .map_err(unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    Ok(rows)
}

pub fn create_product_type(
    conn: &mut Connection,
    name: &str,
) -> Result<ProductType, CatalogueError> {
    let name = normalize_label(name, "Indiquez le nom du type.")?;
    let tx = conn.transaction().map_err(unavailable)?;
    ensure_type_name_free(&tx, &name, None)?;
    insert_named(&tx, "product_types", &name)?;
    let id = tx.last_insert_rowid();
    let record = load_type(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(record)
}

pub fn update_product_type(
    conn: &mut Connection,
    id: i64,
    name: &str,
) -> Result<ProductType, CatalogueError> {
    let name = normalize_label(name, "Indiquez le nom du type.")?;
    let tx = conn.transaction().map_err(unavailable)?;
    load_type(&tx, id)?;
    ensure_type_name_free(&tx, &name, Some(id))?;
    rename(&tx, "product_types", id, &name)?;
    let record = load_type(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(record)
}

pub fn set_product_type_active(
    conn: &mut Connection,
    id: i64,
    is_active: bool,
) -> Result<ProductType, CatalogueError> {
    let tx = conn.transaction().map_err(unavailable)?;
    set_flag(
        &tx,
        "product_types",
        id,
        is_active,
        "Ce type de produit est introuvable.",
    )?;
    let record = load_type(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(record)
}

pub fn list_suppliers(
    conn: &Connection,
    include_inactive: bool,
) -> Result<Vec<Supplier>, CatalogueError> {
    let sql = if include_inactive {
        "SELECT id, name, is_active, created_at, updated_at
         FROM suppliers ORDER BY name COLLATE NOCASE, id"
    } else {
        "SELECT id, name, is_active, created_at, updated_at
         FROM suppliers WHERE is_active = 1 ORDER BY name COLLATE NOCASE, id"
    };
    let mut statement = conn.prepare(sql).map_err(unavailable)?;
    let rows = statement
        .query_map([], map_supplier)
        .map_err(unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    Ok(rows)
}

pub fn create_supplier(conn: &mut Connection, name: &str) -> Result<Supplier, CatalogueError> {
    let name = normalize_label(name, "Indiquez le nom du fournisseur.")?;
    let tx = conn.transaction().map_err(unavailable)?;
    ensure_supplier_name_free(&tx, &name, None)?;
    insert_named(&tx, "suppliers", &name)?;
    let id = tx.last_insert_rowid();
    let record = load_supplier(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(record)
}

pub fn update_supplier(
    conn: &mut Connection,
    id: i64,
    name: &str,
) -> Result<Supplier, CatalogueError> {
    let name = normalize_label(name, "Indiquez le nom du fournisseur.")?;
    let tx = conn.transaction().map_err(unavailable)?;
    load_supplier(&tx, id)?;
    ensure_supplier_name_free(&tx, &name, Some(id))?;
    rename(&tx, "suppliers", id, &name)?;
    let record = load_supplier(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(record)
}

pub fn set_supplier_active(
    conn: &mut Connection,
    id: i64,
    is_active: bool,
) -> Result<Supplier, CatalogueError> {
    let tx = conn.transaction().map_err(unavailable)?;
    set_flag(
        &tx,
        "suppliers",
        id,
        is_active,
        "Ce fournisseur est introuvable.",
    )?;
    let record = load_supplier(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(record)
}

pub fn list_products(
    conn: &Connection,
    query: &ProductQuery,
) -> Result<Vec<Product>, CatalogueError> {
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(like_pattern);
    let status = match query.status {
        ActivityFilter::Active => "active",
        ActivityFilter::Inactive => "inactive",
        ActivityFilter::All => "all",
    };
    let sql = format!(
        "{PRODUCT_SQL}
         WHERE (?1 IS NULL OR p.type_id = ?1)
           AND (?2 IS NULL OR p.supplier_id = ?2)
           AND (
                ?3 = 'all'
                OR (?3 = 'active' AND p.is_active = 1)
                OR (?3 = 'inactive' AND p.is_active = 0)
           )
           AND (
                ?4 IS NULL
                OR p.name LIKE ?4 ESCAPE '\\'
                OR IFNULL(p.reference, '') LIKE ?4 ESCAPE '\\'
           )
         ORDER BY p.name COLLATE NOCASE, p.id"
    );
    let mut statement = conn.prepare(&sql).map_err(unavailable)?;
    let rows = statement
        .query_map(
            params![query.type_id, query.supplier_id, status, search],
            map_product,
        )
        .map_err(unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    Ok(rows)
}

pub fn get_product(conn: &Connection, id: i64) -> Result<Product, CatalogueError> {
    load_product(conn, id)
}

/// Insère tous les articles dans une seule transaction immédiate.
/// Une erreur annule l'ensemble : aucune ligne partielle n'est conservée.
/// Le stock n'est pas modifié : le déclencheur crée seulement un solde à zéro.
pub fn import_products(
    conn: &mut Connection,
    drafts: &[ProductDraft],
) -> Result<usize, CatalogueError> {
    if drafts.is_empty() {
        return Err(CatalogueError::Invalid("Aucune ligne à importer.".into()));
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    for draft in drafts {
        let input = normalize_product(draft)?;
        ensure_type_assignable(&tx, input.type_id, None)?;
        ensure_supplier_assignable(&tx, input.supplier_id, None)?;
        ensure_reference_free(&tx, input.reference.as_deref(), None)?;
        insert_product(&tx, &input)?;
    }
    let count = drafts.len();
    tx.commit().map_err(unavailable)?;
    Ok(count)
}

pub fn create_product(
    conn: &mut Connection,
    draft: ProductDraft,
) -> Result<Product, CatalogueError> {
    let input = normalize_product(&draft)?;
    let tx = conn.transaction().map_err(unavailable)?;
    ensure_type_assignable(&tx, input.type_id, None)?;
    ensure_supplier_assignable(&tx, input.supplier_id, None)?;
    ensure_reference_free(&tx, input.reference.as_deref(), None)?;
    insert_product(&tx, &input)?;
    let id = tx.last_insert_rowid();
    let product = load_product(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(product)
}

pub fn update_product(
    conn: &mut Connection,
    id: i64,
    draft: ProductDraft,
) -> Result<Product, CatalogueError> {
    let input = normalize_product(&draft)?;
    let tx = conn.transaction().map_err(unavailable)?;
    let current = load_product(&tx, id)?;
    ensure_type_assignable(&tx, input.type_id, Some(current.type_id))?;
    ensure_supplier_assignable(&tx, input.supplier_id, current.supplier_id)?;
    ensure_reference_free(&tx, input.reference.as_deref(), Some(id))?;
    tx.execute(
        "UPDATE products
         SET name = ?1, reference = ?2, type_id = ?3, supplier_id = ?4, description = ?5,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?6",
        params![
            input.name,
            input.reference,
            input.type_id,
            input.supplier_id,
            input.description,
            id
        ],
    )
    .map_err(unavailable)?;
    let product = load_product(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(product)
}

pub fn set_product_active(
    conn: &mut Connection,
    id: i64,
    is_active: bool,
) -> Result<Product, CatalogueError> {
    let tx = conn.transaction().map_err(unavailable)?;
    let changed = tx
        .execute(
            "UPDATE products
             SET is_active = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
            params![i64::from(is_active), id],
        )
        .map_err(unavailable)?;
    if changed == 0 {
        return Err(CatalogueError::NotFound(
            "Ce produit est introuvable.".into(),
        ));
    }
    let product = load_product(&tx, id)?;
    tx.commit().map_err(unavailable)?;
    Ok(product)
}

fn insert_product(tx: &Transaction<'_>, input: &NormalizedProduct) -> Result<(), CatalogueError> {
    tx.execute(
        "INSERT INTO products (
            name, reference, type_id, supplier_id, description, is_active, created_at, updated_at
         )
         SELECT ?1, ?2, ?3, ?4, ?5, 1, ts, ts
         FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts)",
        params![
            input.name,
            input.reference,
            input.type_id,
            input.supplier_id,
            input.description
        ],
    )
    .map_err(|err| {
        if error::is_constraint(&err) {
            CatalogueError::Conflict("Cette référence est déjà utilisée.".into())
        } else {
            unavailable(err)
        }
    })?;
    Ok(())
}

fn ensure_type_assignable(
    conn: &Connection,
    type_id: i64,
    current_type_id: Option<i64>,
) -> Result<(), CatalogueError> {
    if current_type_id == Some(type_id) {
        return Ok(());
    }
    let active = flag(
        conn,
        "SELECT is_active FROM product_types WHERE id = ?1",
        type_id,
    )?;
    match active {
        None => Err(CatalogueError::Invalid(
            "Ce type de produit n'existe pas.".into(),
        )),
        Some(false) => Err(CatalogueError::Invalid(
            "Ce type de produit est désactivé.".into(),
        )),
        Some(true) => Ok(()),
    }
}

fn ensure_supplier_assignable(
    conn: &Connection,
    supplier_id: Option<i64>,
    current_supplier_id: Option<i64>,
) -> Result<(), CatalogueError> {
    let Some(supplier_id) = supplier_id else {
        return Ok(());
    };
    if current_supplier_id == Some(supplier_id) {
        return Ok(());
    }
    let active = flag(
        conn,
        "SELECT is_active FROM suppliers WHERE id = ?1",
        supplier_id,
    )?;
    match active {
        None => Err(CatalogueError::Invalid(
            "Ce fournisseur n'existe pas.".into(),
        )),
        Some(false) => Err(CatalogueError::Invalid(
            "Ce fournisseur est désactivé.".into(),
        )),
        Some(true) => Ok(()),
    }
}

fn ensure_reference_free(
    conn: &Connection,
    reference: Option<&str>,
    except_id: Option<i64>,
) -> Result<(), CatalogueError> {
    let Some(reference) = reference else {
        return Ok(());
    };
    let mut statement = conn
        .prepare("SELECT id, reference FROM products WHERE reference IS NOT NULL")
        .map_err(unavailable)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(unavailable)?;
    for row in rows {
        let (id, existing) = row.map_err(unavailable)?;
        if except_id == Some(id) {
            continue;
        }
        if same_name(&existing, reference) {
            return Err(CatalogueError::Conflict(
                "Cette référence est déjà utilisée.".into(),
            ));
        }
    }
    Ok(())
}

fn ensure_type_name_free(
    conn: &Connection,
    name: &str,
    except_id: Option<i64>,
) -> Result<(), CatalogueError> {
    ensure_name_free(
        conn,
        "SELECT id, name FROM product_types",
        name,
        except_id,
        "Un type portant ce nom existe déjà.",
    )
}

fn ensure_supplier_name_free(
    conn: &Connection,
    name: &str,
    except_id: Option<i64>,
) -> Result<(), CatalogueError> {
    ensure_name_free(
        conn,
        "SELECT id, name FROM suppliers",
        name,
        except_id,
        "Un fournisseur portant ce nom existe déjà.",
    )
}

fn ensure_name_free(
    conn: &Connection,
    sql: &str,
    name: &str,
    except_id: Option<i64>,
    message: &str,
) -> Result<(), CatalogueError> {
    let mut statement = conn.prepare(sql).map_err(unavailable)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(unavailable)?;
    for row in rows {
        let (id, existing) = row.map_err(unavailable)?;
        if except_id == Some(id) {
            continue;
        }
        if same_name(&existing, name) {
            return Err(CatalogueError::Conflict(message.to_string()));
        }
    }
    Ok(())
}

fn insert_named(tx: &Transaction<'_>, table: &str, name: &str) -> Result<(), CatalogueError> {
    let sql = match table {
        "product_types" => {
            "INSERT INTO product_types (name, is_active, created_at, updated_at)
             SELECT ?1, 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts)"
        }
        "suppliers" => {
            "INSERT INTO suppliers (name, is_active, created_at, updated_at)
             SELECT ?1, 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts)"
        }
        _ => return Err(unavailable_static()),
    };
    tx.execute(sql, params![name]).map_err(|err| {
        if error::is_constraint(&err) {
            CatalogueError::Conflict(if table == "product_types" {
                "Un type portant ce nom existe déjà.".into()
            } else {
                "Un fournisseur portant ce nom existe déjà.".into()
            })
        } else {
            unavailable(err)
        }
    })?;
    Ok(())
}

fn rename(tx: &Transaction<'_>, table: &str, id: i64, name: &str) -> Result<(), CatalogueError> {
    let sql = match table {
        "product_types" => {
            "UPDATE product_types
             SET name = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2"
        }
        "suppliers" => {
            "UPDATE suppliers
             SET name = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2"
        }
        _ => return Err(unavailable_static()),
    };
    tx.execute(sql, params![name, id]).map_err(|err| {
        if error::is_constraint(&err) {
            CatalogueError::Conflict(if table == "product_types" {
                "Un type portant ce nom existe déjà.".into()
            } else {
                "Un fournisseur portant ce nom existe déjà.".into()
            })
        } else {
            unavailable(err)
        }
    })?;
    Ok(())
}

fn set_flag(
    tx: &Transaction<'_>,
    table: &str,
    id: i64,
    is_active: bool,
    missing: &str,
) -> Result<(), CatalogueError> {
    let sql = match table {
        "product_types" => {
            "UPDATE product_types
             SET is_active = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2"
        }
        "suppliers" => {
            "UPDATE suppliers
             SET is_active = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2"
        }
        _ => return Err(unavailable_static()),
    };
    let changed = tx
        .execute(sql, params![i64::from(is_active), id])
        .map_err(unavailable)?;
    if changed == 0 {
        return Err(CatalogueError::NotFound(missing.to_string()));
    }
    Ok(())
}

fn flag(conn: &Connection, sql: &str, id: i64) -> Result<Option<bool>, CatalogueError> {
    let value = conn
        .query_row(sql, params![id], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(unavailable)?;
    Ok(value.map(|flag| flag != 0))
}

fn load_type(conn: &Connection, id: i64) -> Result<ProductType, CatalogueError> {
    conn.query_row(
        "SELECT id, name, is_active, created_at, updated_at FROM product_types WHERE id = ?1",
        params![id],
        map_type,
    )
    .optional()
    .map_err(unavailable)?
    .ok_or_else(|| CatalogueError::NotFound("Ce type de produit est introuvable.".into()))
}

fn load_supplier(conn: &Connection, id: i64) -> Result<Supplier, CatalogueError> {
    conn.query_row(
        "SELECT id, name, is_active, created_at, updated_at FROM suppliers WHERE id = ?1",
        params![id],
        map_supplier,
    )
    .optional()
    .map_err(unavailable)?
    .ok_or_else(|| CatalogueError::NotFound("Ce fournisseur est introuvable.".into()))
}

fn load_product(conn: &Connection, id: i64) -> Result<Product, CatalogueError> {
    let sql = format!("{PRODUCT_SQL} WHERE p.id = ?1");
    conn.query_row(&sql, params![id], map_product)
        .optional()
        .map_err(unavailable)?
        .ok_or_else(|| CatalogueError::NotFound("Ce produit est introuvable.".into()))
}

fn map_type(row: &Row<'_>) -> rusqlite::Result<ProductType> {
    Ok(ProductType {
        id: row.get(0)?,
        name: row.get(1)?,
        is_active: row.get::<_, i64>(2)? != 0,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_supplier(row: &Row<'_>) -> rusqlite::Result<Supplier> {
    Ok(Supplier {
        id: row.get(0)?,
        name: row.get(1)?,
        is_active: row.get::<_, i64>(2)? != 0,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_product(row: &Row<'_>) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get(0)?,
        name: row.get(1)?,
        reference: row.get(2)?,
        type_id: row.get(3)?,
        type_name: row.get(4)?,
        supplier_id: row.get(5)?,
        supplier_name: row.get(6)?,
        description: row.get(7)?,
        is_active: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn like_pattern(search: &str) -> String {
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn unavailable_static() -> CatalogueError {
    CatalogueError::Unavailable("L'opération n'a pas pu être terminée.".into())
}
