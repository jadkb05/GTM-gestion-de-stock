use super::*;
use crate::catalogue::{ProductDraft, create_product};
use crate::connect;
use crate::migrate::{self, MIGRATIONS};
use crate::stock::{
    CountInput, InventoryDraft, MovementDraft, add_inventory_products, create_inventory,
    get_product_stock, record_entry, save_inventory_counts, validate_inventory,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("horloge")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gtm-backup-{nanos}-{id}"));
    fs::create_dir_all(&path).expect("répertoire");
    path
}

fn open() -> (PathBuf, Connection) {
    let dir = temp_dir();
    let conn = connect(&dir).expect("connexion");
    (dir, conn)
}

fn product(conn: &mut Connection, name: &str, reference: &str) -> i64 {
    let type_id: i64 = conn
        .query_row(
            "SELECT id FROM product_types WHERE name = 'Silicone'",
            [],
            |row| row.get(0),
        )
        .expect("type");
    let supplier_id: i64 = conn
        .query_row(
            "SELECT id FROM suppliers WHERE name = 'Selena'",
            [],
            |row| row.get(0),
        )
        .expect("fournisseur");
    create_product(
        conn,
        ProductDraft {
            name: name.into(),
            reference: Some(reference.into()),
            type_id,
            supplier_id: Some(supplier_id),
            description: None,
        },
    )
    .expect("produit")
    .id
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).expect("compte")
}

#[test]
fn sauvegarde_une_base_vide_puis_le_catalogue_et_les_mouvements() {
    let (dir, mut conn) = open();
    let empty = dir.join("vide.gtm-backup");
    let report = create_backup(&conn, &empty, "0.1.0").expect("vide");
    assert_eq!(report.product_count, 0);
    assert_eq!(report.type_count, 3);
    assert_eq!(report.supplier_count, 3);
    assert_eq!(report.schema_version, 4);
    assert_eq!(report.format_version.as_deref(), Some("1"));
    assert!(report.created_at.is_some());
    let status = last_backup_status(&conn).expect("statut");
    assert_eq!(status.path.as_deref(), Some(empty.to_str().unwrap()));

    let id = product(&mut conn, "Étanchéité", "ET-1");
    record_entry(
        &mut conn,
        MovementDraft {
            product_id: id,
            quantity: "12,25".into(),
            reason: Some("Réception".into()),
            document_reference: None,
            comment: None,
        },
    )
    .expect("entrée");
    let full = dir.join("complet.gtm-backup");
    let report = create_backup(&conn, &full, "0.1.0").expect("complet");
    assert_eq!(report.product_count, 1);
    assert_eq!(report.movement_count, 1);
    let inspected = inspect_backup(&full).expect("contrôle");
    assert_eq!(inspected.product_count, 1);
    assert_eq!(inspected.app_version.as_deref(), Some("0.1.0"));
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sauvegarde_un_inventaire_et_restaure_les_donnees() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Bande", "BAN");
    record_entry(
        &mut conn,
        MovementDraft {
            product_id: id,
            quantity: "10".into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("entrée");
    let draft = create_inventory(
        &mut conn,
        InventoryDraft {
            name: Some("Mars".into()),
            comment: None,
        },
    )
    .expect("inventaire");
    let draft = add_inventory_products(&mut conn, draft.id, &[id]).expect("ligne");
    save_inventory_counts(
        &mut conn,
        draft.id,
        &[CountInput {
            line_id: draft.lines[0].id,
            quantity: Some("8".into()),
        }],
    )
    .expect("comptage");
    validate_inventory(&mut conn, draft.id).expect("validation");
    let backup = dir.join("inventaire.gtm-backup");
    let report = create_backup(&conn, &backup, "0.1.0").expect("sauvegarde");
    assert_eq!(report.inventory_count, 1);
    assert_eq!(report.movement_count, 2);
    product(&mut conn, "Ajout après sauvegarde", "APRES");
    drop(conn);

    let restored = restore_backup(&dir, &backup).expect("restauration");
    assert_eq!(restored.product_count, 1);
    let conn = connect(&dir).expect("réouverture");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM products WHERE name = 'Bande'"),
        1
    );
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM products WHERE name = 'Ajout après sauvegarde'"
        ),
        0
    );
    let product_id: i64 = conn
        .query_row("SELECT id FROM products WHERE name = 'Bande'", [], |row| {
            row.get(0)
        })
        .expect("id");
    assert_eq!(
        get_product_stock(&conn, product_id)
            .expect("stock")
            .quantity_milli,
        8_000
    );
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM inventories WHERE status = 'validated'"
        ),
        1
    );
    assert!(dir.join(ROLLBACK_NAME).is_file());
    drop(conn);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn la_sauvegarde_ignore_une_transaction_non_validee() {
    let (dir, conn) = open();
    let id = product(
        &mut Connection::open(crate::database_path(&dir)).expect("écriture"),
        "Stable",
        "ST",
    );
    let reader = Connection::open(crate::database_path(&dir)).expect("lecteur");
    assert_eq!(count(&reader, "SELECT COUNT(*) FROM products"), 1);
    let in_use = dir.join("en-cours.gtm-backup");
    assert_eq!(
        create_backup(&conn, &in_use, "0.1.0")
            .expect("pendant lecture")
            .product_count,
        1
    );
    drop(reader);
    let writer = Connection::open(crate::database_path(&dir)).expect("écrivain");
    writer.execute_batch("BEGIN IMMEDIATE").expect("début");
    writer
        .execute(
            "INSERT INTO products (name, reference, type_id, supplier_id, description, is_active, created_at, updated_at)
             SELECT 'Invisible', 'INV', id, NULL, NULL, 1, ts, ts
             FROM product_types, (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts)
             WHERE name = 'Silicone'",
            [],
        )
        .expect("insertion");
    let mut target = Connection::open(dir.join("instantane.db")).expect("cible");
    copy_database(&conn, &mut target).expect("copie");
    let report = validate_connection(&target).expect("contrôle");
    assert_eq!(report.product_count, 1);
    writer.execute_batch("ROLLBACK").expect("annulation");
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM products WHERE name = 'Invisible'"
        ),
        0
    );
    let _ = id;
    drop(writer);
    drop(conn);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_un_fichier_invalide_corrompu_ou_trop_recent_sans_toucher_la_base() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Conservé", "CON");
    let garbage = dir.join("garbage.gtm-backup");
    fs::write(&garbage, b"ceci n'est pas sqlite").expect("fichier");
    assert!(inspect_backup(&garbage).is_err());
    assert!(restore_backup(&dir, &garbage).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);

    let broken = dir.join("broken.gtm-backup");
    create_backup(&conn, &broken, "0.1.0").expect("sauvegarde");
    let mut bytes = fs::read(&broken).expect("lu");
    bytes.truncate(bytes.len() / 3);
    fs::write(&broken, &bytes).expect("corruption");
    let error = inspect_backup(&broken).expect_err("corrompu");
    assert!(error.to_string().contains("intégrité") || error.to_string().contains("lisible"));
    assert!(restore_backup(&dir, &broken).is_err());

    let future = dir.join("future.db");
    let future_conn = Connection::open(&future).expect("future");
    future_conn
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);
             INSERT INTO schema_migrations (version, name, applied_at) VALUES
             (1, 'app_metadata', '2026-01-01'), (2, 'catalogue', '2026-01-01'),
             (3, 'stock', '2026-01-01'), (4, 'inventory', '2026-01-01'), (5, 'future', '2026-01-01');
             CREATE TABLE app_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_metadata (key, value) VALUES ('app_name', 'GTM Stock Manager');
             CREATE TABLE product_types (id INTEGER PRIMARY KEY, name TEXT, is_active INTEGER, created_at TEXT, updated_at TEXT);
             CREATE TABLE suppliers (id INTEGER PRIMARY KEY, name TEXT, is_active INTEGER, created_at TEXT, updated_at TEXT);
             CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, reference TEXT, type_id INTEGER, supplier_id INTEGER, description TEXT, is_active INTEGER, created_at TEXT, updated_at TEXT);
             CREATE TABLE stock_balances (product_id INTEGER PRIMARY KEY, quantity_milli INTEGER, updated_at TEXT);
             CREATE TABLE stock_movements (id INTEGER PRIMARY KEY, product_id INTEGER, direction TEXT, quantity_milli INTEGER, reason TEXT, document_reference TEXT, comment TEXT, created_at TEXT);
             CREATE TABLE inventories (id INTEGER PRIMARY KEY, name TEXT, comment TEXT, status TEXT, created_at TEXT, validated_at TEXT);
             CREATE TABLE inventory_lines (id INTEGER PRIMARY KEY, inventory_id INTEGER, product_id INTEGER, theoretical_milli INTEGER, counted_milli INTEGER, snapshot_at TEXT);",
        )
        .expect("schéma futur");
    drop(future_conn);
    let error = inspect_backup(&future).expect_err("future");
    assert!(error.to_string().contains("plus récente"));
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        0
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_des_cles_etrangeres_incoherentes_et_conserve_la_base() {
    let (dir, mut conn) = open();
    product(&mut conn, "Intact", "INT");
    let broken = dir.join("liens.gtm-backup");
    create_backup(&conn, &broken, "0.1.0").expect("base");
    let editable = Connection::open(&broken).expect("édition");
    editable
        .execute_batch("PRAGMA foreign_keys = OFF")
        .expect("pragma");
    editable
        .execute(
            "INSERT INTO products (name, reference, type_id, supplier_id, description, is_active, created_at, updated_at)
             VALUES ('Orphelin', 'ORP', 99999, NULL, NULL, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("orphelin");
    drop(editable);
    let error = inspect_backup(&broken).expect_err("liens");
    assert!(error.to_string().contains("liens"));
    assert!(restore_backup(&dir, &broken).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn l_inspection_ne_migre_pas_une_sauvegarde_ancienne() {
    let dir = temp_dir();
    let path = dir.join("ancienne.db");
    let mut conn = Connection::open(&path).expect("ouverture");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("pragma");
    migrate::apply_migrations(&mut conn, &MIGRATIONS[..3]).expect("migrations 1 à 3");
    let type_id: i64 = conn
        .query_row(
            "SELECT id FROM product_types WHERE name = 'Acrylique'",
            [],
            |row| row.get(0),
        )
        .expect("type");
    create_product(
        &mut conn,
        ProductDraft {
            name: "Ancien".into(),
            reference: Some("ANC".into()),
            type_id,
            supplier_id: None,
            description: None,
        },
    )
    .expect("produit");
    drop(conn);
    let report = inspect_backup(&path).expect("inspection");
    assert_eq!(report.schema_version, 3);
    assert_eq!(report.product_count, 1);
    assert_eq!(report.inventory_count, 0);
    let version: i64 = Connection::open(&path)
        .expect("relecture")
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("version");
    assert_eq!(version, 3);

    let (live_dir, live) = open();
    product(
        &mut Connection::open(crate::database_path(&live_dir)).expect("live"),
        "Récent",
        "REC",
    );
    drop(live);
    restore_backup(&live_dir, &path).expect("restauration");
    let reopened = connect(&live_dir).expect("réouverture");
    assert_eq!(
        count(&reopened, "SELECT MAX(version) FROM schema_migrations"),
        4
    );
    assert_eq!(
        count(
            &reopened,
            "SELECT COUNT(*) FROM products WHERE name = 'Ancien'"
        ),
        1
    );
    assert_eq!(
        count(
            &reopened,
            "SELECT COUNT(*) FROM products WHERE name = 'Récent'"
        ),
        0
    );
    drop(reopened);
    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(live_dir);
}

#[test]
fn un_echec_de_remplacement_remet_la_base_precedente() {
    let dir = temp_dir();
    let current = dir.join("gtm-stock.db");
    fs::write(&current, b"ANCIENNE").expect("base");
    let incoming = dir.join("incoming.db");
    fs::write(&incoming, b"NOUVELLE").expect("entrante");
    FAIL_AFTER_MOVING_CURRENT.store(true, AtomicOrdering::SeqCst);
    let error = swap_database(&current, &incoming).expect_err("échec");
    assert!(error.to_string().contains("remise en place"));
    assert_eq!(fs::read(&current).expect("relu"), b"ANCIENNE");
    assert!(incoming.is_file());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_interruption_apres_deplacement_retrouve_la_base() {
    let (dir, mut conn) = open();
    product(&mut conn, "Préservé", "PRE");
    drop(conn);
    let live = crate::database_path(&dir);
    let previous = dir.join("gtm-stock.previous.db");
    fs::rename(&live, &previous).expect("déplacement");
    fs::write(dir.join("gtm-stock.incoming.db"), b"incomplet").expect("fichier entrant");
    let reopened = connect(&dir).expect("reprise");
    assert_eq!(
        count(
            &reopened,
            "SELECT COUNT(*) FROM products WHERE name = 'Préservé'"
        ),
        1
    );
    assert!(live.is_file());
    assert!(!previous.exists());
    drop(reopened);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_base_presente_n_est_pas_ecrasee_par_le_fichier_precedent() {
    let (dir, conn) = open();
    drop(conn);
    let previous = dir.join("gtm-stock.previous.db");
    fs::write(&previous, b"NE PAS REPRENDRE").expect("marqueur");
    let reopened = connect(&dir).expect("ouverture");
    assert_eq!(count(&reopened, "SELECT COUNT(*) FROM products"), 0);
    assert_eq!(
        fs::read(&previous).expect("marqueur relu"),
        b"NE PAS REPRENDRE"
    );
    drop(reopened);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn restaure_une_sauvegarde_valide_meme_si_la_base_actuelle_est_abimee() {
    let (dir, mut conn) = open();
    product(&mut conn, "Sauvé", "SAU");
    let backup = dir.join("retour.gtm-backup");
    create_backup(&conn, &backup, "0.1.0").expect("sauvegarde");
    product(&mut conn, "Après", "APR");
    conn.pragma_update(None, "wal_checkpoint", "TRUNCATE")
        .expect("point de contrôle");
    drop(conn);
    let live = crate::database_path(&dir);
    let mut bytes = fs::read(&live).expect("lecture");
    let start = 4096.min(bytes.len().saturating_sub(64));
    if start + 32 <= bytes.len() {
        bytes[start..start + 32].fill(0xA5);
    }
    fs::write(&live, &bytes).expect("corruption");
    let _ = fs::remove_file(live.with_file_name("gtm-stock.db-wal"));
    let _ = fs::remove_file(live.with_file_name("gtm-stock.db-shm"));
    restore_backup(&dir, &backup).expect("restauration");
    let reopened = connect(&dir).expect("réouverture");
    assert_eq!(
        count(
            &reopened,
            "SELECT COUNT(*) FROM products WHERE name = 'Sauvé'"
        ),
        1
    );
    assert_eq!(
        count(
            &reopened,
            "SELECT COUNT(*) FROM products WHERE name = 'Après'"
        ),
        0
    );
    assert!(dir.join("gtm-stock.rollback.db").is_file());
    drop(reopened);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn signale_une_destination_impossible_et_n_ecrit_pas_la_date() {
    let (dir, conn) = open();
    let missing = Path::new("/tmp/gtm-sauvegarde-absente/sauvegarde.gtm-backup");
    let error = create_backup(&conn, missing, "0.1.0").expect_err("dossier");
    assert!(error.to_string().contains("introuvable"));
    let status = last_backup_status(&conn).expect("statut");
    assert!(status.created_at.is_none());
    let error = create_backup(&conn, &dir, "0.1.0").expect_err("dossier cible");
    assert!(error.to_string().contains("fichier") || error.to_string().contains("extension"));
    let _ = fs::remove_dir_all(dir);
}
