use rusqlite::{Connection, params};

use crate::error::DbError;

/// Migration SQL versionnée, embarquée dans l'application.
pub(crate) struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "app_metadata",
        sql: include_str!("../migrations/0001_app_metadata.sql"),
    },
    Migration {
        version: 2,
        name: "catalogue",
        sql: include_str!("../migrations/0002_catalogue.sql"),
    },
    Migration {
        version: 3,
        name: "stock",
        sql: include_str!("../migrations/0003_stock.sql"),
    },
    Migration {
        version: 4,
        name: "inventory",
        sql: include_str!("../migrations/0004_inventory.sql"),
    },
];

/// Applique les migrations manquantes. Chaque migration est une transaction.
pub(crate) fn apply_migrations(
    conn: &mut Connection,
    migrations: &[Migration],
) -> Result<i64, DbError> {
    validate_migration_list(migrations)?;
    ensure_migrations_table(conn)?;
    let applied = applied_versions(conn)?;
    ensure_compatible(&applied, migrations)?;
    // WAL persiste dans le fichier. On ne l'active qu'après avoir accepté la version.
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(DbError::Configure)?;

    for migration in migrations.iter().skip(applied.len()) {
        apply_one(conn, migration)?;
    }

    current_version(conn)
}

fn validate_migration_list(migrations: &[Migration]) -> Result<(), DbError> {
    let mut previous = 0;
    for migration in migrations {
        if migration.version <= previous {
            return Err(DbError::Inconsistent(format!(
                "Les migrations doivent être strictement croissantes (version {}).",
                migration.version
            )));
        }
        if migration.name.is_empty() || migration.sql.trim().is_empty() {
            return Err(DbError::Inconsistent(format!(
                "La migration {} est vide.",
                migration.version
            )));
        }
        previous = migration.version;
    }
    Ok(())
}

fn ensure_migrations_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )
    .map_err(DbError::Query)
}

fn applied_versions(conn: &Connection) -> Result<Vec<i64>, DbError> {
    let mut statement = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
        .map_err(DbError::Query)?;
    let versions = statement
        .query_map([], |row| row.get(0))
        .map_err(DbError::Query)?
        .collect::<Result<Vec<i64>, _>>()
        .map_err(DbError::Query)?;
    Ok(versions)
}

fn ensure_compatible(applied: &[i64], migrations: &[Migration]) -> Result<(), DbError> {
    if applied.len() > migrations.len() {
        return Err(DbError::Inconsistent(
            "La base locale est plus récente que cette version de l'application.".into(),
        ));
    }

    for (applied_version, migration) in applied.iter().zip(migrations.iter()) {
        if *applied_version != migration.version {
            return Err(DbError::Inconsistent(format!(
                "La base locale contient la migration {applied_version}, alors que l'application attend {}.",
                migration.version
            )));
        }
    }

    Ok(())
}

fn apply_one(conn: &mut Connection, migration: &Migration) -> Result<(), DbError> {
    let transaction = conn.transaction().map_err(|source| DbError::Migration {
        version: migration.version,
        name: migration.name,
        source,
    })?;

    transaction
        .execute_batch(migration.sql)
        .map_err(|source| DbError::Migration {
            version: migration.version,
            name: migration.name,
            source,
        })?;

    transaction
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![migration.version, migration.name],
        )
        .map_err(|source| DbError::Migration {
            version: migration.version,
            name: migration.name,
            source,
        })?;

    transaction.commit().map_err(|source| DbError::Migration {
        version: migration.version,
        name: migration.name,
        source,
    })?;

    Ok(())
}

fn current_version(conn: &Connection) -> Result<i64, DbError> {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .map_err(DbError::Query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn connection() -> Connection {
        Connection::open_in_memory().expect("base mémoire")
    }

    #[test]
    fn migration_echouee_annule_la_transaction() {
        let mut conn = connection();
        let migrations = [
            Migration {
                version: 1,
                name: "premiere",
                sql: "CREATE TABLE premiere (id INTEGER PRIMARY KEY);",
            },
            Migration {
                version: 2,
                name: "invalide",
                sql: "CREATE TABLE deuxieme (id INTEGER PRIMARY KEY); CECI N'EST PAS DU SQL;",
            },
        ];

        let error =
            apply_migrations(&mut conn, &migrations).expect_err("la seconde migration échoue");
        assert!(error.to_string().contains("migration 2"));

        let versions = applied_versions(&conn).expect("versions");
        assert_eq!(versions, vec![1]);

        let deuxieme_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params!["deuxieme"],
                |row| row.get(0),
            )
            .expect("sqlite_master");
        assert_eq!(deuxieme_exists, 0);

        let premiere_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params!["premiere"],
                |row| row.get(0),
            )
            .expect("sqlite_master");
        assert_eq!(premiere_exists, 1);
    }

    #[test]
    fn refuse_une_base_plus_recente() {
        let mut conn = connection();
        apply_migrations(&mut conn, MIGRATIONS).expect("migration initiale");
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![5, "future", "2026-01-01T00:00:00.000Z"],
        )
        .expect("insertion");

        let error = apply_migrations(&mut conn, MIGRATIONS).expect_err("version future");
        assert!(error.to_string().contains("plus récente"));
    }

    #[test]
    fn une_requete_parametree_n_execute_pas_le_texte_de_la_cle() {
        let mut conn = connection();
        apply_migrations(&mut conn, MIGRATIONS).expect("migration");

        let malicious_key = "'; DROP TABLE app_metadata;--";
        let value = crate::read_metadata(&conn, malicious_key).expect("lecture");
        assert!(value.is_none());

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_metadata", [], |row| row.get(0))
            .expect("table intacte");
        assert_eq!(count, 2);
    }
}
