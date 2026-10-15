use super::{csv_io, error::ExchangeError, xlsx_io};
use crate::catalogue::{ProductDraft, import_products};
use rusqlite::{Connection, OptionalExtension, params};
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

pub fn commit_article_import(conn: &mut Connection, path: &Path) -> Result<usize, ExchangeError> {
    let prepared = prepare(conn, path)?;
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
    let table = read_table(path)?;
    if table.is_empty() {
        return Err(ExchangeError::message(
            "Le fichier ne contient aucun article.",
        ));
    }
    let header = &table[0];
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
    let mut ready = Vec::new();
    let mut issues = Vec::new();
    let mut drafts = Vec::new();
    let mut seen_references = Vec::new();
    for (offset, row) in table.iter().enumerate().skip(1) {
        let line = (offset + 1) as i64;
        if row.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        for index in &stock_indexes {
            if row.get(*index).is_some_and(|cell| !cell.trim().is_empty()) {
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
            let key = ascii_lower(reference);
            if let Some(previous) = seen_references.iter().find(|(seen, _)| seen == &key) {
                issues.push(ImportIssue {
                    line,
                    message: format!(
                        "La référence « {reference} » est en double dans le fichier (ligne {}).",
                        previous.1
                    ),
                });
            } else {
                seen_references.push((key, line));
            }
            if reference.chars().count() > 64 {
                issues.push(ImportIssue {
                    line,
                    message: "La référence dépasse 64 caractères.".into(),
                });
            }
            if reference_exists(conn, reference)? {
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
            match lookup_named(conn, "product_types", &type_name, "type")? {
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
            match lookup_named(conn, "suppliers", supplier_name, "fournisseur")? {
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
        },
        drafts,
    })
}

fn read_table(path: &Path) -> Result<Vec<Vec<String>>, ExchangeError> {
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
    if metadata.len() > MAX_IMPORT_BYTES {
        return Err(ExchangeError::message(
            "Le fichier dépasse 8 Mo. Découpez-le avant de l'importer.",
        ));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "csv" => {
            let bytes = std::fs::read(path)?;
            csv_io::read_csv(&bytes)
        }
        "xlsx" => xlsx_io::read_xlsx(path),
        _ => Err(ExchangeError::message(
            "Importez un fichier CSV ou Excel (.xlsx).",
        )),
    }
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

fn ascii_lower(value: &str) -> String {
    value
        .chars()
        .map(|char| {
            if char.is_ascii_uppercase() {
                char.to_ascii_lowercase()
            } else {
                char
            }
        })
        .collect()
}

enum NameLookup {
    Active(i64),
    Inactive,
    Missing,
}

fn lookup_named(
    conn: &Connection,
    table: &str,
    name: &str,
    _kind: &str,
) -> Result<NameLookup, ExchangeError> {
    if table != "product_types" && table != "suppliers" {
        return Err(ExchangeError::message("L'import n'a pas pu être préparé."));
    }
    let sql = format!("SELECT id, is_active FROM {table} WHERE name = ?1 COLLATE NOCASE");
    let found = conn
        .query_row(&sql, params![name], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)? == 1))
        })
        .optional()
        .map_err(|_| ExchangeError::message("L'import n'a pas pu être préparé."))?;
    Ok(match found {
        Some((id, true)) => NameLookup::Active(id),
        Some((_, false)) => NameLookup::Inactive,
        None => NameLookup::Missing,
    })
}

fn reference_exists(conn: &Connection, reference: &str) -> Result<bool, ExchangeError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM products WHERE reference = ?1 COLLATE NOCASE",
            params![reference],
            |row| row.get(0),
        )
        .map_err(|_| ExchangeError::message("L'import n'a pas pu être préparé."))?;
    Ok(count > 0)
}
