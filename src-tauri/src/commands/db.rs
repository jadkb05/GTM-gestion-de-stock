use gtm_db::{Connection, DbError};
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};
use tauri::Manager;

static DB_GATE: Mutex<()> = Mutex::new(());

/// Connexion tenue pendant toute la commande.
///
/// Le verrou empêche une restauration de remplacer les fichiers pendant
/// qu'une autre commande a encore la base ouverte.
pub(crate) struct Session {
    _guard: MutexGuard<'static, ()>,
    connection: Connection,
}

impl Deref for Session {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

pub(crate) fn open(app: &tauri::AppHandle) -> Result<Session, String> {
    let guard = gate();
    let dir = app.path().app_data_dir().map_err(|_| {
        "Impossible de déterminer le répertoire de données de l'application.".to_string()
    })?;
    let connection = gtm_db::connect(&dir).map_err(message)?;
    Ok(Session {
        _guard: guard,
        connection,
    })
}

pub(crate) fn gate() -> MutexGuard<'static, ()> {
    DB_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn message(error: DbError) -> String {
    match error {
        DbError::Inconsistent(detail) => detail,
        DbError::CreateDir(_) => "Impossible de créer le répertoire de données.".to_string(),
        DbError::Open(_)
        | DbError::Configure(_)
        | DbError::Query(_)
        | DbError::Migration { .. } => "La base locale n'a pas pu être ouverte.".to_string(),
    }
}
