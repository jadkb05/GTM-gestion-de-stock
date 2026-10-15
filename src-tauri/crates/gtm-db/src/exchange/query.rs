use super::{Report, display_milli, display_stamp, error::ExchangeError, outcome, write_report};
use crate::catalogue::{ActivityFilter, ProductQuery};
use crate::stock::{AlertQuery, list_reorder_alerts};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Xlsx,
    Pdf,
}

impl ExportFormat {
    pub fn parse(value: &str) -> Result<Self, ExchangeError> {
        match value.trim() {
            "csv" => Ok(Self::Csv),
            "xlsx" => Ok(Self::Xlsx),
            "pdf" => Ok(Self::Pdf),
            _ => Err(ExchangeError::message("Le format d'export est inconnu.")),
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOutcome {
    pub rows: usize,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryExportQuery {
    pub search: Option<String>,
    pub product_id: Option<i64>,
    pub direction: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

pub fn export_catalogue(
    conn: &Connection,
    path: &std::path::Path,
    format: ExportFormat,
    query: &ProductQuery,
) -> Result<ExportOutcome, ExchangeError> {
    let report = catalogue_report(conn, query)?;
    let result = outcome(&report);
    write_report(conn, path, format, &report)?;
    Ok(result)
}

pub fn export_history(
    conn: &Connection,
    path: &std::path::Path,
    format: ExportFormat,
    query: &HistoryExportQuery,
) -> Result<ExportOutcome, ExchangeError> {
    let report = history_report(conn, query)?;
    let result = outcome(&report);
    write_report(conn, path, format, &report)?;
    Ok(result)
}

pub fn export_inventory(
    conn: &Connection,
    path: &std::path::Path,
    format: ExportFormat,
    inventory_id: i64,
) -> Result<ExportOutcome, ExchangeError> {
    let report = inventory_report(conn, inventory_id)?;
    let result = outcome(&report);
    write_report(conn, path, format, &report)?;
    Ok(result)
}

pub fn export_reorder(
    conn: &Connection,
    path: &std::path::Path,
    format: ExportFormat,
    query: &AlertQuery,
) -> Result<ExportOutcome, ExchangeError> {
    let alerts = list_reorder_alerts(conn, query).map_err(stock_error)?;
    let filtered = query
        .search
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || query.type_id.is_some()
        || query.supplier_id.is_some();
    let scope = if filtered {
        "Produits actifs en alerte correspondant aux filtres. Stock lu au moment de l'export."
    } else {
        "Produits actifs dont le stock courant est inférieur ou égal au seuil."
    };
    let mut rows = Vec::new();
    for alert in alerts {
        rows.push(vec![
            alert.name,
            alert.reference.unwrap_or_default(),
            alert.type_name,
            alert.supplier_name.unwrap_or_default(),
            display_milli(alert.quantity_milli),
            display_milli(alert.threshold_milli),
        ]);
    }
    let report = Report {
        title: "Produits à réapprovisionner".into(),
        scope: scope.into(),
        sheet_name: "Réapprovisionnement".into(),
        headers: vec![
            "Produit".into(),
            "Référence".into(),
            "Type".into(),
            "Fournisseur".into(),
            "Stock actuel".into(),
            "Seuil".into(),
        ],
        rows,
    };
    let result = outcome(&report);
    write_report(conn, path, format, &report)?;
    Ok(result)
}

fn catalogue_report(conn: &Connection, query: &ProductQuery) -> Result<Report, ExchangeError> {
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
    let mut statement = conn
        .prepare(
            "SELECT
                p.id, p.reference, p.name, t.name, s.name,
                b.quantity_milli, p.reorder_threshold_milli, p.is_active,
                datetime(p.created_at, 'localtime'), datetime(p.updated_at, 'localtime')
             FROM products p
             JOIN product_types t ON t.id = p.type_id
             LEFT JOIN suppliers s ON s.id = p.supplier_id
             JOIN stock_balances b ON b.product_id = p.id
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
             ORDER BY p.name COLLATE NOCASE, p.id",
        )
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?;
    let rows = statement
        .query_map(
            params![query.type_id, query.supplier_id, status, search],
            |row| {
                let threshold: Option<i64> = row.get(6)?;
                let active: i64 = row.get(7)?;
                Ok(vec![
                    row.get::<_, i64>(0)?.to_string(),
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    row.get(2)?,
                    row.get(3)?,
                    row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    display_milli(row.get(5)?),
                    threshold.map(display_milli).unwrap_or_default(),
                    if active == 1 { "Actif" } else { "Inactif" }.to_string(),
                    display_stamp(&row.get::<_, String>(8)?),
                    display_stamp(&row.get::<_, String>(9)?),
                ])
            },
        )
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?;
    let narrowed = search.is_some() || query.type_id.is_some() || query.supplier_id.is_some();
    let population = match query.status {
        ActivityFilter::Active => "produits actifs",
        ActivityFilter::Inactive => "produits inactifs",
        ActivityFilter::All => "produits actifs et inactifs",
    };
    let scope = if narrowed {
        format!("Périmètre filtré : {population}. Stock lu au moment de l'export.")
    } else {
        format!("Périmètre : {population}. Stock lu au moment de l'export.")
    };
    Ok(Report {
        title: "Catalogue et stocks".into(),
        scope,
        sheet_name: "Catalogue".into(),
        headers: vec![
            "Identifiant".into(),
            "Référence".into(),
            "Nom".into(),
            "Type".into(),
            "Fournisseur".into(),
            "Stock actuel".into(),
            "Seuil".into(),
            "État".into(),
            "Créé le".into(),
            "Modifié le".into(),
        ],
        rows,
    })
}

fn history_report(conn: &Connection, query: &HistoryExportQuery) -> Result<Report, ExchangeError> {
    let direction = match query.direction.as_deref().map(str::trim) {
        None | Some("") | Some("all") => None,
        Some("in") => Some("in"),
        Some("out") => Some("out"),
        Some(_) => {
            return Err(ExchangeError::message(
                "Le filtre de mouvement est invalide.",
            ));
        }
    };
    let from_date = clean_day(query.from_date.as_deref())?;
    let to_date = clean_day(query.to_date.as_deref())?;
    if let (Some(from_date), Some(to_date)) = (&from_date, &to_date) {
        if to_date < from_date {
            return Err(ExchangeError::message(
                "La date de fin précède la date de début.",
            ));
        }
    }
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(like_pattern);
    let mut statement = conn
        .prepare(
            "SELECT
                datetime(m.created_at, 'localtime'), p.name, p.reference, m.direction, m.quantity_milli,
                m.reason, m.document_reference, m.comment, m.source, m.inventory_id
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
               AND (?4 IS NULL OR date(m.created_at, 'localtime') >= ?4)
               AND (?5 IS NULL OR date(m.created_at, 'localtime') <= ?5)
             ORDER BY m.created_at ASC, m.id ASC",
        )
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?;
    let rows = statement
        .query_map(
            params![query.product_id, direction, search, from_date, to_date],
            |row| {
                let direction: String = row.get(3)?;
                let source: String = row.get(8)?;
                let inventory_id: Option<i64> = row.get(9)?;
                Ok(vec![
                    display_stamp(&row.get::<_, String>(0)?),
                    row.get(1)?,
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    movement_label(&direction),
                    display_milli(row.get(4)?),
                    row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    source_label(&source),
                    inventory_id.map(|id| id.to_string()).unwrap_or_default(),
                ])
            },
        )
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?;
    let filtered = search.is_some()
        || query.product_id.is_some()
        || direction.is_some()
        || from_date.is_some()
        || to_date.is_some();
    let scope = if filtered {
        "Mouvements correspondant aux filtres et à la période, sans la limite d'affichage de 200 lignes."
    } else {
        "Historique complet, sans la limite d'affichage de 200 lignes."
    };
    Ok(Report {
        title: "Historique des mouvements".into(),
        scope: scope.into(),
        sheet_name: "Mouvements".into(),
        headers: vec![
            "Date".into(),
            "Produit".into(),
            "Référence".into(),
            "Mouvement".into(),
            "Quantité".into(),
            "Motif".into(),
            "Référence documentaire".into(),
            "Commentaire".into(),
            "Source".into(),
            "Inventaire".into(),
        ],
        rows,
    })
}

fn inventory_report(conn: &Connection, inventory_id: i64) -> Result<Report, ExchangeError> {
    let header = conn
        .query_row(
            "SELECT name, status, datetime(created_at, 'localtime'), datetime(validated_at, 'localtime')
             FROM inventories WHERE id = ?1",
            params![inventory_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?
        .ok_or_else(|| ExchangeError::message("Cet inventaire est introuvable."))?;
    let (name, status_code, created_at, validated_at) = header;
    let status = if status_code == "validated" {
        "Validé"
    } else {
        "Brouillon"
    };
    let name = name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("Inventaire {inventory_id}"));
    let mut statement = conn
        .prepare(
            "SELECT p.name, p.reference, l.theoretical_milli, l.counted_milli,
                    datetime(l.snapshot_at, 'localtime')
             FROM inventory_lines l
             JOIN products p ON p.id = l.product_id
             WHERE l.inventory_id = ?1
             ORDER BY p.name COLLATE NOCASE, l.id",
        )
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?;
    let rows = statement
        .query_map(params![inventory_id], |row| {
            let theoretical: i64 = row.get(2)?;
            let counted: Option<i64> = row.get(3)?;
            let variance = counted.map(|value| display_milli(value - theoretical));
            Ok(vec![
                name.clone(),
                display_stamp(&created_at),
                status.to_string(),
                validated_at
                    .as_deref()
                    .map(display_stamp)
                    .unwrap_or_default(),
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                display_milli(theoretical),
                counted.map(display_milli).unwrap_or_default(),
                variance.unwrap_or_default(),
                display_stamp(&row.get::<_, String>(4)?),
            ])
        })
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ExchangeError::message("L'export n'a pas pu être préparé."))?;
    Ok(Report {
        title: format!("Inventaire — {name}"),
        scope: format!(
            "{status}. Écart = quantité comptée − stock de référence figé. Une cellule vide n'est pas un zéro."
        ),
        sheet_name: "Inventaire".into(),
        headers: vec![
            "Inventaire".into(),
            "Date".into(),
            "Statut".into(),
            "Validé le".into(),
            "Produit".into(),
            "Référence".into(),
            "Stock de référence".into(),
            "Quantité comptée".into(),
            "Écart".into(),
            "Date de référence".into(),
        ],
        rows,
    })
}

fn movement_label(direction: &str) -> String {
    match direction {
        "in" => "Entrée",
        "out" => "Sortie",
        other => other,
    }
    .to_string()
}

fn source_label(source: &str) -> String {
    match source {
        "inventory" => "Inventaire",
        _ => "Manuel",
    }
    .to_string()
}

fn clean_day(value: Option<&str>) -> Result<Option<String>, ExchangeError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let bytes = value.as_bytes();
    let shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !shape {
        return Err(ExchangeError::message(
            "La période doit utiliser le format AAAA-MM-JJ.",
        ));
    }
    let month: u32 = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..10].parse().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(ExchangeError::message(
            "La période doit utiliser le format AAAA-MM-JJ.",
        ));
    }
    Ok(Some(value.to_string()))
}

fn like_pattern(search: &str) -> String {
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn stock_error(error: crate::StockError) -> ExchangeError {
    ExchangeError::message(error.to_string())
}
