use super::import::{SourceRow, SourceTable};
use super::{Report, error::ExchangeError, guard_formula};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub(super) fn write_csv(path: &Path, report: &Report) -> Result<(), ExchangeError> {
    let mut file = File::create(path)?;
    file.write_all(&[0xEF, 0xBB, 0xBF])?;
    let mut writer = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
    writer.write_record(report.headers.iter().map(|cell| guard_formula(cell)))?;
    for row in &report.rows {
        writer.write_record(row.iter().map(|cell| guard_formula(cell)))?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
pub(super) fn read_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>, ExchangeError> {
    Ok(read_csv_rows(bytes)?
        .rows
        .into_iter()
        .map(|row| row.cells)
        .collect())
}

/// Lit le CSV en conservant le numéro de ligne du fichier de chaque
/// enregistrement : les lignes vides sautées ne décalent pas les messages.
pub(super) fn read_csv_rows(bytes: &[u8]) -> Result<SourceTable, ExchangeError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ExchangeError::message("Le fichier CSV doit être encodé en UTF-8."))?;
    if text.trim().is_empty() {
        return Err(ExchangeError::message("Le fichier est vide."));
    }
    let first = text.lines().next().unwrap_or("");
    let delimiter =
        if first.matches(';').count() >= first.matches(',').count() && first.contains(';') {
            b';'
        } else {
            b','
        };
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(false)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    let mut counter = LineCounter::default();
    for record in reader.records() {
        let record = record?;
        let line = record
            .position()
            .map(|position| counter.line_at(text, position.byte() as usize))
            .unwrap_or(rows.len() as i64 + 1);
        rows.push(SourceRow {
            line,
            cells: record.iter().map(ToOwned::to_owned).collect(),
        });
    }
    if rows
        .first()
        .is_none_or(|header| header.cells.iter().all(|cell| cell.trim().is_empty()))
    {
        return Err(ExchangeError::message(
            "Le fichier ne contient pas d'en-têtes.",
        ));
    }
    Ok(SourceTable {
        rows,
        unreadable: Vec::new(),
    })
}

/// Numéro de ligne du premier caractère utile à partir d'un octet. La position
/// fournie par le lecteur précède les lignes vides qu'il a sautées. Le compte
/// reprend là où il s'était arrêté : les positions arrivent dans l'ordre.
#[derive(Default)]
struct LineCounter {
    byte: usize,
    newlines: i64,
}

impl LineCounter {
    fn line_at(&mut self, text: &str, byte: usize) -> i64 {
        let bytes = text.as_bytes();
        let mut start = byte.min(bytes.len());
        while start < bytes.len() && matches!(bytes[start], b'\n' | b'\r') {
            start += 1;
        }
        if start < self.byte {
            self.byte = 0;
            self.newlines = 0;
        }
        self.newlines += bytes[self.byte..start]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count() as i64;
        self.byte = start;
        1 + self.newlines
    }
}
