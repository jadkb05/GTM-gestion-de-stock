use std::fmt;

/// Erreur métier du stock, destinée à l'interface.
#[derive(Debug, PartialEq, Eq)]
pub enum StockError {
    Invalid(String),
    NotFound(String),
    Inactive(String),
    Insufficient(String),
    Unavailable(String),
}

impl fmt::Display for StockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message)
            | Self::NotFound(message)
            | Self::Inactive(message)
            | Self::Insufficient(message)
            | Self::Unavailable(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for StockError {}

pub(crate) fn unavailable() -> StockError {
    StockError::Unavailable("Le mouvement n'a pas pu être enregistré.".into())
}
