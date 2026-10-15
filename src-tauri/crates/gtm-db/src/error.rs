use std::fmt;

/// Erreur explicite de la persistance locale.
#[derive(Debug)]
pub enum DbError {
    CreateDir(std::io::Error),
    Open(rusqlite::Error),
    Configure(rusqlite::Error),
    Query(rusqlite::Error),
    Migration {
        version: i64,
        name: &'static str,
        source: rusqlite::Error,
    },
    Inconsistent(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir(err) => {
                write!(f, "Impossible de créer le répertoire de données : {err}")
            }
            Self::Open(err) => write!(f, "Impossible d'ouvrir la base locale : {err}"),
            Self::Configure(err) => {
                write!(f, "Impossible de configurer la base locale : {err}")
            }
            Self::Query(err) => write!(f, "La lecture de la base locale a échoué : {err}"),
            Self::Migration {
                version,
                name,
                source,
            } => write!(f, "La migration {version} ({name}) a échoué : {source}"),
            Self::Inconsistent(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir(err) => Some(err),
            Self::Open(err) | Self::Configure(err) | Self::Query(err) => Some(err),
            Self::Migration { source, .. } => Some(source),
            Self::Inconsistent(_) => None,
        }
    }
}
