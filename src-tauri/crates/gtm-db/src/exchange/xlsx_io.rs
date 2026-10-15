use super::import::{SourceRow, SourceTable, UnreadableCell};
use super::{Report, error::ExchangeError, generated_at};
use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use rust_xlsxwriter::{Format, Workbook};
use std::io::Cursor;
use std::path::Path;

pub(super) fn write_xlsx(path: &Path, report: &Report) -> Result<(), ExchangeError> {
    let mut workbook = Workbook::new();
    let bold = Format::new().set_bold();
    let data = workbook.add_worksheet();
    data.set_name(&sheet_name(&report.sheet_name))?;
    for (column, header) in report.headers.iter().enumerate() {
        data.write_string_with_format(0, column as u16, header, &bold)?;
        data.set_column_width(column as u16, 18)?;
    }
    let whole = Format::new().set_num_format("#,##0");
    let decimal = Format::new().set_num_format("#,##0.###");
    for (row_index, row) in report.rows.iter().enumerate() {
        let line = (row_index + 1) as u32;
        for (column, cell) in row.iter().enumerate() {
            let quantity = report
                .quantity_columns
                .contains(&column)
                .then(|| quantity_value(cell))
                .flatten();
            match quantity {
                Some((value, is_whole)) => {
                    let format = if is_whole { &whole } else { &decimal };
                    data.write_number_with_format(line, column as u16, value, format)?;
                }
                None if cell.is_empty() => {}
                None => {
                    data.write_string(line, column as u16, cell)?;
                }
            }
        }
    }
    let scope = workbook.add_worksheet();
    scope.set_name("Périmètre")?;
    scope.write_string_with_format(0, 0, "Libellé", &bold)?;
    scope.write_string_with_format(0, 1, "Valeur", &bold)?;
    scope.write_string(1, 0, "Titre")?;
    scope.write_string(1, 1, &report.title)?;
    scope.write_string(2, 0, "Généré le")?;
    scope.write_string(2, 1, &generated_at())?;
    scope.write_string(3, 0, "Périmètre")?;
    scope.write_string(3, 1, &report.scope)?;
    scope.write_string(4, 0, "Lignes")?;
    scope.write_string(4, 1, &report.rows.len().to_string())?;
    scope.set_column_width(0, 18)?;
    scope.set_column_width(1, 80)?;
    workbook.save(path)?;
    Ok(())
}

/// Quantité affichée (« -12 345,678 » au plus trois décimales) relue en nombre.
/// Les quantités ont au plus 14 chiffres significatifs : un f64 les conserve
/// exactement, comme Excel. Renvoie aussi si la valeur est entière.
fn quantity_value(cell: &str) -> Option<(f64, bool)> {
    let text = cell.trim();
    let (integer, fraction) = text.split_once(',').unwrap_or((text, ""));
    let digits = integer.strip_prefix('-').unwrap_or(integer);
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 3
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let value = text.replace(',', ".").parse::<f64>().ok()?;
    Some((value, fraction.bytes().all(|byte| byte == b'0')))
}

fn sheet_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|char| match char {
            ':' | '\\' | '/' | '?' | '*' | '[' | ']' => ' ',
            _ => char,
        })
        .take(31)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Données"
    } else {
        trimmed
    }
    .to_string()
}

#[cfg(test)]
pub(super) fn read_xlsx(path: &Path) -> Result<Vec<Vec<String>>, ExchangeError> {
    Ok(read_xlsx_rows(&std::fs::read(path)?)?
        .rows
        .into_iter()
        .map(|row| row.cells)
        .collect())
}

/// Lit la première feuille avec le numéro de ligne Excel de chaque rangée.
/// Une cellule non textuelle n'interrompt pas la lecture : elle est notée,
/// et l'import ne la signale que si sa colonne est réellement utilisée.
pub(super) fn read_xlsx_rows(bytes: &[u8]) -> Result<SourceTable, ExchangeError> {
    let mut workbook: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes))?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| ExchangeError::message("Le fichier Excel ne contient aucune feuille."))?
        .map_err(ExchangeError::from)?;
    let first_row = range.start().map(|(row, _)| row as i64).unwrap_or(0);
    let mut table = SourceTable {
        rows: Vec::new(),
        unreadable: Vec::new(),
    };
    for (offset, row) in range.rows().enumerate() {
        let line = first_row + offset as i64 + 1;
        let mut cells = Vec::with_capacity(row.len());
        for (column, cell) in row.iter().enumerate() {
            match cell_text(cell) {
                Ok(text) => cells.push(text),
                Err(message) => {
                    table.unreadable.push(UnreadableCell {
                        line,
                        column,
                        message,
                    });
                    cells.push(String::new());
                }
            }
        }
        if cells.iter().all(|cell| cell.trim().is_empty())
            && !table.unreadable.iter().any(|cell| cell.line == line)
        {
            continue;
        }
        table.rows.push(SourceRow { line, cells });
    }
    if table.rows.is_empty() {
        return Err(ExchangeError::message("Le fichier Excel est vide."));
    }
    Ok(table)
}

fn cell_text(cell: &Data) -> Result<String, &'static str> {
    match cell {
        Data::Empty => Ok(String::new()),
        Data::String(value) => Ok(value.clone()),
        Data::Int(value) => Ok(value.to_string()),
        Data::Float(value) => float_text(*value),
        Data::Bool(_)
        | Data::DateTime(_)
        | Data::DateTimeIso(_)
        | Data::DurationIso(_)
        | Data::Error(_) => {
            Err("la cellule contient une valeur qui n'est pas du texte. Convertissez-la en texte.")
        }
    }
}

fn float_text(value: f64) -> Result<String, &'static str> {
    if !value.is_finite() {
        return Err("la cellule numérique n'est pas lisible.");
    }
    if value.fract() == 0.0 && value.abs() < 1_000_000_000_000.0 {
        return Ok((value as i64).to_string());
    }
    Err("la cellule numérique n'est pas acceptée. Saisissez le texte de l'article.")
}
