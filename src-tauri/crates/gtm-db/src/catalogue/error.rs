use std::fmt;

/// Erreur métier du catalogue, destinée à l'interface.
#[derive(Debug, PartialEq, Eq)]
pub enum CatalogueError {
    Invalid(String),
    NotFound(String),
    Conflict(String),
    Unavailable(String),
}

impl fmt::Display for CatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::Unavailable(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CatalogueError {}

pub(crate) fn unavailable(err: rusqlite::Error) -> CatalogueError {
    if is_constraint(&err) {
        CatalogueError::Conflict(
            "Cette valeur entre en conflit avec un enregistrement existant.".into(),
        )
    } else {
        CatalogueError::Unavailable("L'opération n'a pas pu être terminée.".into())
    }
}

pub(crate) fn is_constraint(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation
    )
}
