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

pub(super) fn read_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>, ExchangeError> {
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
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    let headers = reader
        .headers()?
        .iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if headers.iter().all(|cell| cell.trim().is_empty()) {
        return Err(ExchangeError::message(
            "Le fichier ne contient pas d'en-têtes.",
        ));
    }
    rows.push(headers);
    for record in reader.records() {
        let record = record?;
        rows.push(record.iter().map(ToOwned::to_owned).collect());
    }
    Ok(rows)
}
