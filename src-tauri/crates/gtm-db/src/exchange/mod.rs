//! Exports CSV, Excel et PDF, et import contrôlé d'articles.
//!
//! Les exports ne font que lire. L'import n'écrit qu'après validation complète,
//! dans une seule transaction, et ne crée ni mouvement de stock, ni type, ni fournisseur.

mod csv_io;
mod error;
mod import;
mod pdf_io;
mod query;
mod xlsx_io;

#[cfg(test)]
mod tests;

pub use error::ExchangeError;
pub use import::{
    ImportIssue, ImportPreview, ImportReady, commit_article_import, preview_article_import,
};
pub use query::{
    ExportFormat, ExportOutcome, HistoryExportQuery, export_catalogue, export_history,
    export_inventory, export_reorder,
};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_EXPORT_ROWS: usize = 100_000;
const MAX_PDF_ROWS: usize = 2_000;

struct Report {
    title: String,
    scope: String,
    sheet_name: String,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Date de génération affichée dans le PDF, à l'heure locale du poste comme
/// les horodatages du document. L'heure UTC n'est qu'un repli explicite.
fn generated_at() -> String {
    let local = rusqlite::Connection::open_in_memory().and_then(|conn| {
        conn.query_row(
            "SELECT strftime('%d/%m/%Y %H:%M', 'now', 'localtime')",
            [],
            |row| row.get::<_, String>(0),
        )
    });
    if let Ok(text) = local {
        return text;
    }
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let time = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = time / 3_600;
    let minute = (time % 3_600) / 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (y + if month <= 2 { 1 } else { 0 }) as i32;
    (year, month, day)
}

fn display_milli(milli: i64) -> String {
    crate::format_quantity(milli).replace('.', ",")
}

/// Présente un horodatage déjà converti en heure locale par SQLite
/// (`datetime(..., 'localtime')`) au format français JJ/MM/AAAA HH:MM:SS.
fn display_stamp(value: &str) -> String {
    let text = value.trim_end_matches('Z').replace('T', " ");
    let (Some(year), Some(month), Some(day), Some(time)) = (
        text.get(0..4),
        text.get(5..7),
        text.get(8..10),
        text.get(11..19),
    ) else {
        return text;
    };
    format!("{day}/{month}/{year} {time}")
}

fn guard_formula(value: &str) -> String {
    let trimmed = value.trim_start_matches([' ', '\t']);
    if trimmed.is_empty() || is_plain_number(trimmed) {
        return value.to_string();
    }
    if trimmed.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

fn is_plain_number(value: &str) -> bool {
    let rest = value.strip_prefix('-').unwrap_or(value);
    if rest.is_empty() || rest.starts_with('-') {
        return false;
    }
    let mut separator = false;
    for char in rest.chars() {
        if char.is_ascii_digit() {
            continue;
        }
        if (char == ',' || char == '.') && !separator {
            separator = true;
            continue;
        }
        return false;
    }
    true
}

fn check_output(
    conn: &rusqlite::Connection,
    path: &Path,
    format: ExportFormat,
) -> Result<(), ExchangeError> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(ExchangeError::message("Indiquez un nom de fichier."));
    }
    if path.is_dir() {
        return Err(ExchangeError::message(
            "Indiquez un fichier, pas un dossier.",
        ));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension != format.extension() {
        return Err(ExchangeError::message(
            "L'extension du fichier ne correspond pas au format choisi.",
        ));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        if !parent.is_dir() {
            return Err(ExchangeError::message(
                "Le dossier de destination est introuvable.",
            ));
        }
    }
    if let Some(database) = conn.path() {
        if path == Path::new(database) {
            return Err(ExchangeError::message(
                "Ce chemin est celui de la base locale. Choisissez un autre fichier.",
            ));
        }
    }
    Ok(())
}

fn write_report(
    conn: &rusqlite::Connection,
    path: &Path,
    format: ExportFormat,
    report: &Report,
) -> Result<(), ExchangeError> {
    check_output(conn, path, format)?;
    if report.rows.len() > MAX_EXPORT_ROWS {
        return Err(ExchangeError::message(
            "L'export dépasse 100 000 lignes. Réduisez le périmètre.",
        ));
    }
    if format == ExportFormat::Pdf && report.rows.len() > MAX_PDF_ROWS {
        return Err(ExchangeError::message(
            "Ce rapport PDF dépasse 2 000 lignes. Utilisez le format CSV ou Excel.",
        ));
    }
    let temporary = temporary_path(path);
    let write_result = match format {
        ExportFormat::Csv => csv_io::write_csv(&temporary, report),
        ExportFormat::Xlsx => xlsx_io::write_xlsx(&temporary, report),
        ExportFormat::Pdf => pdf_io::write_pdf(&temporary, report),
    };
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = replace_file(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(ExchangeError::from(error));
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("export");
    path.with_file_name(format!(".{name}.gtm-tmp"))
}

fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    match fs::rename(temporary, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(temporary, destination)?;
            let _ = fs::remove_file(temporary);
            Ok(())
        }
    }
}

fn outcome(report: &Report) -> ExportOutcome {
    ExportOutcome {
        rows: report.rows.len(),
        scope: report.scope.clone(),
    }
}
