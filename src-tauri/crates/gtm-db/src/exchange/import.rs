use super::{csv_io, error::ExchangeError, xlsx_io};
use crate::catalogue::{ProductDraft, import_products};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;

const MAX_IMPORT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReady {
    pub line: i64,
    pub name: String,
    pub reference: Option<String>,
    pub type_name: String,
    pub supplier_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportIssue {
    pub line: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    pub ready: Vec<ImportReady>,
    pub issues: Vec<ImportIssue>,
    pub ignored_columns: Vec<String>,
    /// Empreinte des octets lus pour cet aperçu. L'import la redemande : un
    /// fichier modifié entre l'aperçu et l'import est refusé.
    pub fingerprint: String,
}

/// Rangée lue dans le fichier, avec son numéro de ligne d'origine.
pub(super) struct SourceRow {
    pub line: i64,
    pub cells: Vec<String>,
}

/// Cellule Excel non textuelle, remplacée par une chaîne vide à la lecture.
pub(super) struct UnreadableCell {
    pub line: i64,
    pub column: usize,
    pub message: &'static str,
}

pub(super) struct SourceTable {
    pub rows: Vec<SourceRow>,
    pub unreadable: Vec<UnreadableCell>,
}

struct Prepared {
    preview: ImportPreview,
    drafts: Vec<ProductDraft>,
}

pub fn preview_article_import(
    conn: &Connection,
    path: &Path,
) -> Result<ImportPreview, ExchangeError> {
    Ok(prepare(conn, path)?.preview)
}

pub fn commit_article_import(
    conn: &mut Connection,
    path: &Path,
    fingerprint: &str,
) -> Result<usize, ExchangeError> {
    let prepared = prepare(conn, path)?;
    if prepared.preview.fingerprint != fingerprint {
        return Err(ExchangeError::message(
            "Le fichier a été modifié depuis l'aperçu. Aucun article n'a été enregistré. Relancez l'import pour vérifier son nouveau contenu.",
        ));
    }
    if !prepared.preview.issues.is_empty() {
        let details = prepared
            .preview
            .issues
            .iter()
            .map(|issue| {
                if issue.line == 0 {
                    issue.message.clone()
                } else {
                    format!("Ligne {} : {}", issue.line, issue.message)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        return Err(ExchangeError::message(format!(
            "L'import a été annulé. Aucun article n'a été enregistré. {details}"
        )));
    }
    import_products(conn, &prepared.drafts).map_err(|error| {
        ExchangeError::message(format!(
            "L'import a été annulé. Aucun article n'a été enregistré. {error}"
        ))
    })
}

fn prepare(conn: &Connection, path: &Path) -> Result<Prepared, ExchangeError> {
    // Le fichier est lu une seule fois : l'empreinte et l'analyse portent
    // sur les mêmes octets.
    let (bytes, kind) = read_source(path)?;
    let fingerprint = fingerprint(&bytes);
    let source = match kind {
        SourceKind::Csv => csv_io::read_csv_rows(&bytes)?,
        SourceKind::Xlsx => xlsx_io::read_xlsx_rows(&bytes)?,
    };
    let table = &source.rows;
    if table.is_empty() {
        return Err(ExchangeError::message(
            "Le fichier ne contient aucun article.",
        ));
    }
    let header = &table[0].cells;
    let columns = header
        .iter()
        .map(|cell| canonical(cell))
        .collect::<Vec<_>>();
    let name_index = require_column(&columns, &["nom", "name", "nom du produit"], "Nom")?;
    let type_index = require_column(&columns, &["type", "type de produit"], "Type")?;
    let reference_index = optional_column(&columns, &["reference", "référence", "ref"]);
    let supplier_index = optional_column(&columns, &["fournisseur", "supplier"]);
    let description_index = optional_column(&columns, &["description"]);
    let stock_indexes = columns
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            matches!(
                name.as_str(),
                "stock"
                    | "stock actuel"
                    | "quantite"
                    | "quantité"
                    | "seuil"
                    | "seuil de reapprovisionnement"
            )
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let known = [
        name_index,
        type_index,
        reference_index.unwrap_or(usize::MAX),
        supplier_index.unwrap_or(usize::MAX),
        description_index.unwrap_or(usize::MAX),
    ];
    let mut ignored_columns = Vec::new();
    for (index, label) in header.iter().enumerate() {
        if label.trim().is_empty() || known.contains(&index) || stock_indexes.contains(&index) {
            continue;
        }
        ignored_columns.push(label.trim().to_string());
    }
    if table.len() - 1 > MAX_IMPORT_ROWS {
        return Err(ExchangeError::message(
            "Le fichier dépasse 5 000 articles. Découpez-le avant de l'importer.",
        ));
    }
    let used_columns = [
        (Some(name_index), "Nom"),
        (Some(type_index), "Type"),
        (reference_index, "Référence"),
        (supplier_index, "Fournisseur"),
        (description_index, "Description"),
    ];
    let existing_references = folded_references(conn)?;
    let types = folded_names(conn, "product_types")?;
    let suppliers = folded_names(conn, "suppliers")?;
    let mut ready = Vec::new();
    let mut issues = Vec::new();
    let mut drafts = Vec::new();
    let mut seen_references: Vec<(String, i64)> = Vec::new();
    for source_row in table.iter().skip(1) {
        let line = source_row.line;
        let row = &source_row.cells;
        let unreadable = source
            .unreadable
            .iter()
            .filter(|cell| cell.line == line)
            .collect::<Vec<_>>();
        let unreadable_used = unreadable.iter().any(|cell| {
            stock_indexes.contains(&cell.column)
                || used_columns
                    .iter()
                    .any(|(index, _)| *index == Some(cell.column))
        });
        if row.iter().all(|cell| cell.trim().is_empty()) && !unreadable_used {
            continue;
        }
        for cell in &unreadable {
            if stock_indexes.contains(&cell.column) {
                continue;
            }
            if let Some((_, label)) = used_columns
                .iter()
                .find(|(index, _)| *index == Some(cell.column))
            {
                issues.push(ImportIssue {
                    line,
                    message: format!("Colonne {label} : {}", cell.message),
                });
            }
        }
        for index in &stock_indexes {
            let filled = row.get(*index).is_some_and(|cell| !cell.trim().is_empty())
                || unreadable.iter().any(|cell| cell.column == *index);
            if filled {
                issues.push(ImportIssue {
                    line,
                    message: "Le stock et le seuil ne sont pas importés. Saisissez une entrée de stock pour tracer le mouvement.".into(),
                });
            }
        }
        let name = cell(row, name_index);
        let type_name = cell(row, type_index);
        let reference = reference_index
            .map(|index| cell(row, index))
            .filter(|value| !value.is_empty());
        let supplier_name = supplier_index
            .map(|index| cell(row, index))
            .filter(|value| !value.is_empty());
        let description = description_index
            .map(|index| cell(row, index))
            .filter(|value| !value.is_empty());
        if name.is_empty() {
            issues.push(ImportIssue {
                line,
                message: "Indiquez le nom du produit.".into(),
            });
        } else if name.chars().count() > 120 {
            issues.push(ImportIssue {
                line,
                message: "Le nom dépasse 120 caractères.".into(),
            });
        }
        if type_name.is_empty() {
            issues.push(ImportIssue {
                line,
                message: "Indiquez le type de produit.".into(),
            });
        }
        if let Some(reference) = &reference {
            let key = fold(reference);
            if let Some(previous) = seen_references.iter().find(|(seen, _)| seen == &key) {
                issues.push(ImportIssue {
                    line,
                    message: format!(
                        "La référence « {reference} » est en double dans le fichier (ligne {}).",
                        previous.1
                    ),
                });
            } else {
                seen_references.push((key.clone(), line));
            }
            if reference.chars().count() > 64 {
                issues.push(ImportIssue {
                    line,
                    message: "La référence dépasse 64 caractères.".into(),
                });
            }
            if existing_references.contains(&key) {
                issues.push(ImportIssue {
                    line,
                    message: format!("La référence « {reference} » est déjà utilisée."),
                });
            }
        }
        if description
            .as_ref()
            .is_some_and(|value| value.chars().count() > 2000)
        {
            issues.push(ImportIssue {
                line,
                message: "La description dépasse 2 000 caractères.".into(),
            });
        }
        let type_id = if type_name.is_empty() {
            None
        } else {
            match lookup_named(&types, &type_name) {
                NameLookup::Missing => {
                    issues.push(ImportIssue {
                        line,
                        message: format!(
                            "Le type « {type_name} » n'existe pas. Il ne sera pas créé."
                        ),
                    });
                    None
                }
                NameLookup::Inactive => {
                    issues.push(ImportIssue {
                        line,
                        message: format!("Le type « {type_name} » est désactivé."),
                    });
                    None
                }
                NameLookup::Active(id) => Some(id),
            }
        };
        let supplier_id = if let Some(supplier_name) = &supplier_name {
            match lookup_named(&suppliers, supplier_name) {
                NameLookup::Missing => {
                    issues.push(ImportIssue {
                        line,
                        message: format!(
                            "Le fournisseur « {supplier_name} » n'existe pas. Il ne sera pas créé."
                        ),
                    });
                    None
                }
                NameLookup::Inactive => {
                    issues.push(ImportIssue {
                        line,
                        message: format!("Le fournisseur « {supplier_name} » est désactivé."),
                    });
                    None
                }
                NameLookup::Active(id) => Some(id),
            }
        } else {
            None
        };
        if name.is_empty() || type_id.is_none() || issues.iter().any(|issue| issue.line == line) {
            continue;
        }
        drafts.push(ProductDraft {
            name: name.clone(),
            reference: reference.clone(),
            type_id: type_id.expect("type contrôlé"),
            supplier_id,
            description: description.clone(),
        });
        ready.push(ImportReady {
            line,
            name,
            reference,
            type_name,
            supplier_name,
            description,
        });
    }
    if ready.is_empty() && issues.is_empty() {
        issues.push(ImportIssue {
            line: 0,
            message: "Le fichier ne contient aucun article.".into(),
        });
    }
    Ok(Prepared {
        preview: ImportPreview {
            ready,
            issues,
            ignored_columns,
            fingerprint,
        },
        drafts,
    })
}

enum SourceKind {
    Csv,
    Xlsx,
}

fn read_source(path: &Path) -> Result<(Vec<u8>, SourceKind), ExchangeError> {
    if path.as_os_str().is_empty() {
        return Err(ExchangeError::message("Indiquez un fichier à importer."));
    }
    let metadata = std::fs::metadata(path).map_err(|_| {
        ExchangeError::message("Le fichier à importer est introuvable ou illisible.")
    })?;
    if metadata.is_dir() {
        return Err(ExchangeError::message(
            "Indiquez un fichier, pas un dossier.",
        ));
    }
    let too_large =
        || ExchangeError::message("Le fichier dépasse 8 Mo. Découpez-le avant de l'importer.");
    if metadata.len() > MAX_IMPORT_BYTES {
        return Err(too_large());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = match extension.as_str() {
        "csv" => SourceKind::Csv,
        "xlsx" => SourceKind::Xlsx,
        _ => {
            return Err(ExchangeError::message(
                "Importez un fichier CSV ou Excel (.xlsx).",
            ));
        }
    };
    let bytes = std::fs::read(path)?;
    if bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err(too_large());
    }
    Ok((bytes, kind))
}

/// Taille et FNV-1a 64 bits du contenu : détecte une modification du
/// fichier entre l'aperçu et l'import (pas une protection cryptographique).
fn fingerprint(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{}-{hash:016x}", bytes.len())
}

fn require_column(columns: &[String], names: &[&str], label: &str) -> Result<usize, ExchangeError> {
    optional_column(columns, names)
        .ok_or_else(|| ExchangeError::message(format!("La colonne {label} est obligatoire.")))
}

fn optional_column(columns: &[String], names: &[&str]) -> Option<usize> {
    columns
        .iter()
        .position(|column| names.iter().any(|name| column == name))
}

fn cell(row: &[String], index: usize) -> String {
    row.get(index)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn canonical(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|char| match char {
            'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'À' | 'Â' => 'a',
            'à' | 'â' => 'a',
            'Ù' | 'Û' => 'u',
            'ù' | 'û' => 'u',
            'Ô' => 'o',
            'ô' => 'o',
            'Î' | 'Ï' => 'i',
            'î' | 'ï' => 'i',
            'Ç' => 'c',
            'ç' => 'c',
            other => other,
        })
        .collect::<String>()
        .to_lowercase()
}

/// Même comparaison que le catalogue (`same_name`) : sans tenir compte
/// de la casse, accents compris. L'aperçu annonce ainsi ce que l'import fera.
fn fold(value: &str) -> String {
    value.to_lowercase()
}

enum NameLookup {
    Active(i64),
    Inactive,
    Missing,
}

fn lookup_named(names: &[(String, i64, bool)], name: &str) -> NameLookup {
    let key = fold(name);
    match names.iter().find(|(folded, _, _)| *folded == key) {
        Some((_, id, true)) => NameLookup::Active(*id),
        Some((_, _, false)) => NameLookup::Inactive,
        None => NameLookup::Missing,
    }
}

fn folded_names(conn: &Connection, table: &str) -> Result<Vec<(String, i64, bool)>, ExchangeError> {
    let sql = match table {
        "product_types" => "SELECT id, name, is_active FROM product_types ORDER BY id",
        "suppliers" => "SELECT id, name, is_active FROM suppliers ORDER BY id",
        _ => return Err(ExchangeError::message("L'import n'a pas pu être préparé.")),
    };
    let failed = |_| ExchangeError::message("L'import n'a pas pu être préparé.");
    let mut statement = conn.prepare(sql).map_err(failed)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                fold(&row.get::<_, String>(1)?),
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(2)? == 1,
            ))
        })
        .map_err(failed)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failed)?;
    Ok(rows)
}

fn folded_references(conn: &Connection) -> Result<HashSet<String>, ExchangeError> {
    let failed = |_| ExchangeError::message("L'import n'a pas pu être préparé.");
    let mut statement = conn
        .prepare("SELECT reference FROM products WHERE reference IS NOT NULL")
        .map_err(failed)?;
    let references = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(failed)?
        .map(|reference| reference.map(|value| fold(&value)))
        .collect::<Result<HashSet<_>, _>>()
        .map_err(failed)?;
    Ok(references)
}
