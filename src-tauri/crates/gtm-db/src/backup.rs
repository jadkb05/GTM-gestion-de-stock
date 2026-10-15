//! Sauvegarde cohérente et restauration de la base locale.
//!
//! La copie utilise l'API de sauvegarde SQLite. Elle lit un instantané
//! validé, y compris lorsque le journal WAL est actif, au lieu de copier
//! le fichier principal seul. Le fichier choisi n'est migré qu'après son
//! remplacement contrôlé de la base active.

use crate::migrate::MIGRATIONS;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags, params};
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const FORMAT_VERSION: &str = "1";
const ROLLBACK_NAME: &str = "gtm-stock.rollback.db";
const PREVIOUS_NAME: &str = "gtm-stock.previous.db";
const INCOMING_NAME: &str = "gtm-stock.incoming.db";

#[cfg(test)]
pub(crate) static FAIL_AFTER_MOVING_CURRENT: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReport {
    pub schema_version: i64,
    pub app_name: String,
    pub app_version: Option<String>,
    pub created_at: Option<String>,
    pub format_version: Option<String>,
    pub product_count: i64,
    pub supplier_count: i64,
    pub type_count: i64,
    pub movement_count: i64,
    pub inventory_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupStatus {
    pub created_at: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BackupError {
    message: String,
}

impl BackupError {
    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BackupError {}

pub fn last_backup_status(conn: &Connection) -> Result<BackupStatus, BackupError> {
    Ok(BackupStatus {
        created_at: metadata(conn, "last_backup_at")?,
        path: metadata(conn, "last_backup_path")?,
    })
}

pub fn create_backup(
    source: &Connection,
    destination: &Path,
    app_version: &str,
) -> Result<BackupReport, BackupError> {
    reject_destination(source, destination)?;
    let temporary = temporary_path(destination);
    let report = (|| {
        let mut target = Connection::open(&temporary)
            .map_err(|_| BackupError::message("La sauvegarde n'a pas pu être créée."))?;
        copy_database(source, &mut target)?;
        stamp_backup(&target, app_version)?;
        single_file(&target)?;
        let report = validate_connection(&target)?;
        drop(target);
        Ok(report)
    })();
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if let Err(error) = replace_file(&temporary, destination) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    let created_at = report.created_at.clone().unwrap_or_default();
    write_metadata(source, "last_backup_at", &created_at)?;
    write_metadata(source, "last_backup_path", &destination.to_string_lossy())?;
    Ok(report)
}

pub fn inspect_backup(path: &Path) -> Result<BackupReport, BackupError> {
    let connection = open_readonly(path)?;
    validate_connection(&connection)
}

/// Remet en place la base si le processus s'est arrêté après l'avoir déplacée
/// et avant l'installation du fichier de restauration.
pub(crate) fn recover_interrupted_restore(app_data_dir: &Path) -> Result<(), crate::DbError> {
    let live = crate::database_path(app_data_dir);
    if live.exists() {
        return Ok(());
    }
    let previous = app_data_dir.join(PREVIOUS_NAME);
    if previous.is_file() {
        fs::rename(&previous, &live).map_err(|_| {
            crate::DbError::Inconsistent(
                "Une restauration interrompue n'a pas pu être annulée. La base précédente est toujours dans le dossier de données.".into(),
            )
        })?;
    }
    Ok(())
}

pub fn restore_backup(
    app_data_dir: &Path,
    backup_path: &Path,
) -> Result<BackupReport, BackupError> {
    let live_path = crate::database_path(app_data_dir);
    if same_path(&live_path, backup_path) {
        return Err(BackupError::message(
            "Choisissez une sauvegarde, pas la base actuellement ouverte.",
        ));
    }
    let _report = inspect_backup(backup_path)?;
    let rollback_path = app_data_dir.join(ROLLBACK_NAME);
    let incoming_path = app_data_dir.join(INCOMING_NAME);
    // La sauvegarde choisie est copiée avant d'écrire le point de retour :
    // le fichier choisi peut être ce point de retour lui-même.
    snapshot_into(&open_readonly(backup_path)?, &incoming_path)?;
    let incoming_report = match validate_connection(&open_readonly(&incoming_path)?) {
        Ok(report) => report,
        Err(error) => {
            let _ = fs::remove_file(&incoming_path);
            return Err(error);
        }
    };
    let live = match crate::connect(app_data_dir) {
        Ok(connection) => Some(connection),
        Err(error) if database_is_damaged(&error) => {
            park_damaged_database(&live_path, &rollback_path)?;
            None
        }
        Err(_) => {
            return Err(BackupError::message(
                "La base actuelle n'a pas pu être préparée.".to_string(),
            ));
        }
    };
    if let Some(live) = &live {
        snapshot_into(live, &rollback_path)?;
        match validate_connection(&open_readonly(&rollback_path)?) {
            Ok(_) => {}
            Err(_) => {
                // La copie de retour reste sur le disque. Une base déjà incohérente
                // ne bloque pas l'installation d'une sauvegarde qui a passé les contrôles.
            }
        }
    }
    if let Some(live) = &live {
        checkpoint_truncate(live)?;
    }
    drop(live);
    remove_sidecars(&live_path);
    if let Err(error) = swap_database(&live_path, &incoming_path) {
        let _ = fs::remove_file(&incoming_path);
        return Err(error);
    }
    match crate::connect(app_data_dir) {
        Ok(restored) => {
            let verified = validate_connection(&restored)?;
            if verified.product_count != incoming_report.product_count
                || verified.movement_count != incoming_report.movement_count
                || verified.inventory_count != incoming_report.inventory_count
            {
                drop(restored);
                let previous = app_data_dir.join(PREVIOUS_NAME);
                if previous.is_file() {
                    let _ = fs::rename(&live_path, app_data_dir.join("gtm-stock.failed.db"));
                    let _ = fs::rename(&previous, &live_path);
                    return Err(BackupError::message(
                        "La restauration n'a pas pu être vérifiée. La base précédente a été remise en place.",
                    ));
                }
                return Err(BackupError::message(
                    "La restauration n'a pas pu être vérifiée. Le fichier installé et le point de retour sont conservés.",
                ));
            }
            drop(restored);
            let _ = fs::remove_file(app_data_dir.join(PREVIOUS_NAME));
            let _ = fs::remove_file(&incoming_path);
            Ok(incoming_report)
        }
        Err(_) => {
            let previous = app_data_dir.join(PREVIOUS_NAME);
            if previous.is_file() {
                let _ = fs::rename(&live_path, app_data_dir.join("gtm-stock.failed.db"));
                let _ = fs::rename(&previous, &live_path);
            }
            Err(BackupError::message(
                "La restauration a échoué. La base précédente a été remise en place. Un point de retour reste disponible à côté de la base.",
            ))
        }
    }
}

fn database_is_damaged(error: &crate::DbError) -> bool {
    match error {
        crate::DbError::Open(source)
        | crate::DbError::Configure(source)
        | crate::DbError::Query(source) => sqlite_damage(source),
        crate::DbError::Migration { source, .. } => sqlite_damage(source),
        crate::DbError::CreateDir(_) | crate::DbError::Inconsistent(_) => false,
    }
}

fn sqlite_damage(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(code, _) => matches!(
            code.code,
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
        ),
        _ => false,
    }
}

fn park_damaged_database(live: &Path, rollback: &Path) -> Result<(), BackupError> {
    if !live.exists() {
        return Ok(());
    }
    let destination = if rollback.exists() {
        let damaged = rollback.with_file_name("gtm-stock.damaged.db");
        if damaged.exists() {
            return Err(BackupError::message(
                "La base actuelle est illisible et un fichier de secours occupe déjà son emplacement. Déplacez gtm-stock.damaged.db avant de réessayer.",
            ));
        }
        damaged
    } else {
        rollback.to_path_buf()
    };
    fs::rename(live, &destination)
        .map_err(|_| BackupError::message("La base illisible n'a pas pu être mise de côté."))?;
    move_sidecar(live, &destination, "wal");
    move_sidecar(live, &destination, "shm");
    Ok(())
}

fn move_sidecar(from: &Path, to: &Path, suffix: &str) {
    let (Some(from_name), Some(to_name), Some(parent)) = (
        from.file_name().and_then(|value| value.to_str()),
        to.file_name().and_then(|value| value.to_str()),
        from.parent(),
    ) else {
        return;
    };
    let source = parent.join(format!("{from_name}-{suffix}"));
    let target = to
        .parent()
        .unwrap_or(parent)
        .join(format!("{to_name}-{suffix}"));
    if source.is_file() {
        let _ = fs::rename(source, target);
    }
}

fn snapshot_into(source: &Connection, destination: &Path) -> Result<(), BackupError> {
    let temporary = temporary_path(destination);
    let copied = (|| {
        let mut target = open_writable(&temporary)?;
        copy_database(source, &mut target)?;
        single_file(&target)?;
        drop(target);
        replace_file(&temporary, destination)
    })();
    if copied.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    copied
}

fn copy_database(source: &Connection, target: &mut Connection) -> Result<(), BackupError> {
    let backup = Backup::new(source, target)
        .map_err(|_| BackupError::message("La copie cohérente de la base n'a pas pu commencer."))?;
    let mut busy_tries = 0;
    loop {
        match backup
            .step(-1)
            .map_err(|_| BackupError::message("La copie cohérente de la base a été interrompue."))?
        {
            StepResult::Done => return Ok(()),
            StepResult::More => continue,
            StepResult::Busy | StepResult::Locked => {
                busy_tries += 1;
                if busy_tries > 20 {
                    return Err(BackupError::message(
                        "La base est occupée par une autre opération. Réessayez.",
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
            _ => {
                return Err(BackupError::message(
                    "La copie cohérente de la base a été interrompue.",
                ));
            }
        }
    }
}

/// La copie hérite du mode WAL de la base active. Une copie autonome, sans
/// fichiers -wal et -shm, reste lisible depuis un support en lecture seule.
fn single_file(connection: &Connection) -> Result<(), BackupError> {
    let mode: String = connection
        .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
        .map_err(|_| BackupError::message("La copie de la base n'a pas pu être finalisée."))?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err(BackupError::message(
            "La copie de la base n'a pas pu être finalisée.",
        ));
    }
    Ok(())
}

fn stamp_backup(connection: &Connection, app_version: &str) -> Result<(), BackupError> {
    let schema_version = schema_version(connection)?;
    let created_at = metadata_now(connection)?;
    write_metadata(connection, "backup_format_version", FORMAT_VERSION)?;
    write_metadata(connection, "backup_created_at", &created_at)?;
    write_metadata(
        connection,
        "backup_schema_version",
        &schema_version.to_string(),
    )?;
    write_metadata(connection, "backup_app_version", app_version)?;
    Ok(())
}

fn validate_connection(connection: &Connection) -> Result<BackupReport, BackupError> {
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| BackupError::message("Ce fichier n'est pas une base SQLite lisible."))?;
    if integrity != "ok" {
        return Err(BackupError::message(
            "La sauvegarde échoue au contrôle d'intégrité SQLite.",
        ));
    }
    let foreign_keys: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(|_| BackupError::message("Le contrôle des liens n'a pas pu être exécuté."))?;
    if foreign_keys != 0 {
        return Err(BackupError::message(
            "Cette sauvegarde contient des liens incohérents entre les tables.",
        ));
    }
    let versions = applied_versions(connection)?;
    if versions.is_empty() {
        return Err(BackupError::message(
            "Cette sauvegarde ne contient pas de version de schéma.",
        ));
    }
    if versions.len() > MIGRATIONS.len() {
        return Err(BackupError::message(
            "Cette sauvegarde provient d'une version plus récente de l'application. Elle ne peut pas être restaurée.",
        ));
    }
    for (found, migration) in versions.iter().zip(MIGRATIONS.iter()) {
        if *found != migration.version {
            return Err(BackupError::message(
                "Le schéma de cette sauvegarde n'est pas compatible avec cette version de l'application.",
            ));
        }
    }
    for migration in MIGRATIONS.iter().take(versions.len()) {
        if !table_exists(connection, expected_tables(migration.version))? {
            return Err(BackupError::message(
                "Cette sauvegarde ne contient pas les tables attendues pour sa version de schéma.",
            ));
        }
    }
    let app_name = metadata(connection, "app_name")?.ok_or_else(|| {
        BackupError::message("Cette sauvegarde ne contient pas l'identité de l'application.")
    })?;
    Ok(BackupReport {
        schema_version: *versions.last().expect("version présente"),
        app_name,
        app_version: metadata(connection, "backup_app_version")?,
        created_at: metadata(connection, "backup_created_at")?,
        format_version: metadata(connection, "backup_format_version")?,
        product_count: count_if_present(connection, "products")?,
        supplier_count: count_if_present(connection, "suppliers")?,
        type_count: count_if_present(connection, "product_types")?,
        movement_count: count_if_present(connection, "stock_movements")?,
        inventory_count: count_if_present(connection, "inventories")?,
    })
}

fn expected_tables(version: i64) -> &'static [&'static str] {
    match version {
        1 => &["schema_migrations", "app_metadata"],
        2 => &["product_types", "suppliers", "products"],
        3 => &["stock_balances", "stock_movements"],
        _ => &["inventories", "inventory_lines"],
    }
}

fn table_exists(connection: &Connection, names: &[&str]) -> Result<bool, BackupError> {
    for name in names {
        let found: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|_| BackupError::message("Le schéma de la sauvegarde n'a pas pu être lu."))?;
        if found != 1 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn applied_versions(connection: &Connection) -> Result<Vec<i64>, BackupError> {
    let exists = table_exists(connection, &["schema_migrations"])?;
    if !exists {
        return Err(BackupError::message(
            "Ce fichier ne contient pas le schéma de GTM Stock Manager.",
        ));
    }
    let mut statement = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
        .map_err(|_| BackupError::message("La version du schéma n'a pas pu être lue."))?;
    statement
        .query_map([], |row| row.get(0))
        .map_err(|_| BackupError::message("La version du schéma n'a pas pu être lue."))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| BackupError::message("La version du schéma n'a pas pu être lue."))
}

fn schema_version(connection: &Connection) -> Result<i64, BackupError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|_| BackupError::message("La version du schéma n'a pas pu être lue."))
}

fn count_if_present(connection: &Connection, table: &str) -> Result<i64, BackupError> {
    if !table_exists(connection, &[table])? {
        return Ok(0);
    }
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(|_| BackupError::message("Les données de la sauvegarde n'ont pas pu être lues."))
}

fn checkpoint_truncate(connection: &Connection) -> Result<(), BackupError> {
    let busy: i64 = connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
        .map_err(|_| BackupError::message("Le journal de la base n'a pas pu être refermé."))?;
    if busy != 0 {
        return Err(BackupError::message(
            "Une opération est encore en cours sur la base. Réessayez la restauration.",
        ));
    }
    Ok(())
}

fn swap_database(current: &Path, incoming: &Path) -> Result<(), BackupError> {
    if !incoming.is_file() {
        return Err(BackupError::message(
            "La base préparée pour la restauration est introuvable.",
        ));
    }
    let Some(parent) = current.parent() else {
        return Err(BackupError::message(
            "Le dossier de la base est introuvable.",
        ));
    };
    let previous = parent.join(PREVIOUS_NAME);
    let _ = fs::remove_file(&previous);
    if current.exists() {
        fs::rename(current, &previous)
            .map_err(|_| BackupError::message("La base actuelle n'a pas pu être mise de côté."))?;
    }
    #[cfg(test)]
    if FAIL_AFTER_MOVING_CURRENT.swap(false, Ordering::SeqCst) {
        let _ = fs::rename(&previous, current);
        return Err(BackupError::message(
            "Le remplacement a échoué. La base précédente a été remise en place.",
        ));
    }
    if let Err(error) = fs::rename(incoming, current) {
        let _ = fs::rename(&previous, current);
        let _ = error;
        return Err(BackupError::message(
            "Le remplacement a échoué. La base précédente a été remise en place.",
        ));
    }
    Ok(())
}

fn remove_sidecars(database: &Path) {
    let name = database
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("gtm-stock.db");
    if let Some(parent) = database.parent() {
        let _ = fs::remove_file(parent.join(format!("{name}-wal")));
        let _ = fs::remove_file(parent.join(format!("{name}-shm")));
    }
}

fn reject_destination(source: &Connection, destination: &Path) -> Result<(), BackupError> {
    if destination.as_os_str().is_empty() || destination.file_name().is_none() {
        return Err(BackupError::message("Indiquez un nom de fichier."));
    }
    if destination.is_dir() {
        return Err(BackupError::message("Indiquez un fichier, pas un dossier."));
    }
    let extension = destination
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension != "gtm-backup" {
        return Err(BackupError::message(
            "La sauvegarde doit utiliser l'extension .gtm-backup.",
        ));
    }
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        if !parent.is_dir() {
            return Err(BackupError::message(
                "Le dossier de destination est introuvable.",
            ));
        }
    }
    if let Some(live) = source.path() {
        if same_path(Path::new(live), destination) {
            return Err(BackupError::message(
                "Choisissez un autre fichier que la base locale.",
            ));
        }
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn open_readonly(path: &Path) -> Result<Connection, BackupError> {
    if !path.is_file() {
        return Err(BackupError::message(
            "Le fichier de sauvegarde est introuvable.",
        ));
    }
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| BackupError::message("Ce fichier n'est pas une base SQLite lisible."))
}

fn open_writable(path: &Path) -> Result<Connection, BackupError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(BackupError::message(
                "Le dossier de destination est introuvable.",
            ));
        }
    }
    let _ = fs::remove_file(path);
    Connection::open(path).map_err(|_| BackupError::message("La sauvegarde n'a pas pu être créée."))
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("sauvegarde");
    path.with_file_name(format!(".{name}.gtm-tmp"))
}

fn replace_file(temporary: &Path, destination: &Path) -> Result<(), BackupError> {
    match fs::rename(temporary, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(temporary, destination)
                .map_err(|_| BackupError::message("La sauvegarde n'a pas pu être écrite."))?;
            let _ = fs::remove_file(temporary);
            Ok(())
        }
    }
}

fn metadata(connection: &Connection, key: &str) -> Result<Option<String>, BackupError> {
    if !table_exists(connection, &["app_metadata"])? {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            _ => Err(BackupError::message(
                "Les informations de la sauvegarde n'ont pas pu être lues.",
            )),
        })
}

fn write_metadata(connection: &Connection, key: &str, value: &str) -> Result<(), BackupError> {
    connection
        .execute(
            "INSERT INTO app_metadata (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .map_err(|_| {
            BackupError::message("Les informations de sauvegarde n'ont pas pu être enregistrées.")
        })?;
    Ok(())
}

fn metadata_now(connection: &Connection) -> Result<String, BackupError> {
    connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|_| BackupError::message("La date de sauvegarde n'a pas pu être déterminée."))
}

#[cfg(test)]
mod tests;
