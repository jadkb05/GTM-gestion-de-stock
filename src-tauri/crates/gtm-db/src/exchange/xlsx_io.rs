use super::{Report, error::ExchangeError, generated_at};
use calamine::{Data, Reader, open_workbook_auto};
use rust_xlsxwriter::{Format, Workbook};
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
    for (row_index, row) in report.rows.iter().enumerate() {
        for (column, cell) in row.iter().enumerate() {
            data.write_string((row_index + 1) as u32, column as u16, cell)?;
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

pub(super) fn read_xlsx(path: &Path) -> Result<Vec<Vec<String>>, ExchangeError> {
    let mut workbook = open_workbook_auto(path)?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| ExchangeError::message("Le fichier Excel ne contient aucune feuille."))?
        .map_err(ExchangeError::from)?;
    let mut rows = Vec::new();
    for row in range.rows() {
        let cells = row.iter().map(cell_text).collect::<Result<Vec<_>, _>>()?;
        if cells.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err(ExchangeError::message("Le fichier Excel est vide."));
    }
    Ok(rows)
}

fn cell_text(cell: &Data) -> Result<String, ExchangeError> {
    match cell {
        Data::Empty => Ok(String::new()),
        Data::String(value) => Ok(value.clone()),
        Data::Int(value) => Ok(value.to_string()),
        Data::Float(value) => float_text(*value),
        Data::Bool(_)
        | Data::DateTime(_)
        | Data::DateTimeIso(_)
        | Data::DurationIso(_)
        | Data::Error(_) => Err(ExchangeError::message(
            "Une cellule contient une valeur qui n'est pas du texte. Convertissez-la en texte.",
        )),
    }
}

fn float_text(value: f64) -> Result<String, ExchangeError> {
    if !value.is_finite() {
        return Err(ExchangeError::message(
            "Une cellule numérique n'est pas lisible.",
        ));
    }
    if value.fract() == 0.0 && value.abs() < 1_000_000_000_000.0 {
        return Ok((value as i64).to_string());
    }
    Err(ExchangeError::message(
        "Une cellule numérique n'est pas acceptée. Saisissez le texte de l'article.",
    ))
}
