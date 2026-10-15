use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub struct ExchangeError {
    message: String,
}

impl ExchangeError {
    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ExchangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ExchangeError {}

impl From<std::io::Error> for ExchangeError {
    fn from(error: std::io::Error) -> Self {
        let message = match error.kind() {
            std::io::ErrorKind::PermissionDenied => {
                "Le fichier n'a pas pu être écrit. Vérifiez les droits du dossier."
            }
            std::io::ErrorKind::NotFound => "Le dossier de destination est introuvable.",
            _ => "Le fichier n'a pas pu être écrit.",
        };
        Self::message(message)
    }
}

impl From<csv::Error> for ExchangeError {
    fn from(_: csv::Error) -> Self {
        Self::message("Le fichier CSV n'a pas pu être lu.")
    }
}

impl From<calamine::Error> for ExchangeError {
    fn from(_: calamine::Error) -> Self {
        Self::message("Le fichier Excel n'a pas pu être lu.")
    }
}

impl From<rust_xlsxwriter::XlsxError> for ExchangeError {
    fn from(_: rust_xlsxwriter::XlsxError) -> Self {
        Self::message("Le fichier Excel n'a pas pu être écrit.")
    }
}
