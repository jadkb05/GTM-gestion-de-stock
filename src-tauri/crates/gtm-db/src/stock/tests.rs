use super::*;
use crate::catalogue::{ProductDraft, create_product, set_product_active};
use crate::connect;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("horloge")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gtm-stock-{nanos}-{id}"));
    fs::create_dir_all(&path).expect("répertoire");
    path
}

fn open() -> (PathBuf, rusqlite::Connection) {
    let dir = temp_dir();
    let conn = connect(&dir).expect("connexion");
    (dir, conn)
}

fn type_id(conn: &rusqlite::Connection) -> i64 {
    conn.query_row(
        "SELECT id FROM product_types WHERE name = 'Autre'",
        [],
        |row| row.get(0),
    )
    .expect("type")
}

fn product(conn: &mut rusqlite::Connection, name: &str) -> i64 {
    let type_id = type_id(conn);
    create_product(
        conn,
        ProductDraft {
            name: name.into(),
            reference: None,
            type_id,
            supplier_id: None,
            description: None,
        },
    )
    .expect("produit")
    .id
}

fn draft(product_id: i64, quantity: &str) -> MovementDraft {
    MovementDraft {
        product_id,
        quantity: quantity.into(),
        reason: None,
        document_reference: None,
        comment: None,
    }
}

#[test]
fn un_produit_neuf_a_un_stock_nul() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Cartouche");
    let stock = get_product_stock(&conn, id).expect("stock");
    assert_eq!(stock.quantity_milli, 0);
    assert_eq!(format_quantity(stock.quantity_milli), "0");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_entree_augmente_le_stock_et_reste_dans_l_historique() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic");
    let movement = record_entry(
        &mut conn,
        MovementDraft {
            product_id: id,
            quantity: "1,5".into(),
            reason: Some("  Réception  ".into()),
            document_reference: Some(" BL-10 ".into()),
            comment: Some("Quai 2".into()),
        },
    )
    .expect("entrée");
    assert_eq!(movement.direction, "in");
    assert_eq!(movement.quantity_milli, 1500);
    assert_eq!(movement.reason.as_deref(), Some("Réception"));
    assert_eq!(movement.document_reference.as_deref(), Some("BL-10"));
    assert_eq!(movement.comment.as_deref(), Some("Quai 2"));
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        1500
    );
    let history = list_movements(
        &conn,
        &MovementQuery {
            search: Some("bl-10".into()),
            product_id: Some(id),
            direction: Some("in".into()),
        },
    )
    .expect("historique");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, movement.id);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_sortie_diminue_le_stock_et_reste_dans_l_historique() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic");
    record_entry(&mut conn, draft(id, "2.5")).expect("entrée");
    let movement = record_exit(&mut conn, draft(id, "0.5")).expect("sortie");
    assert_eq!(movement.direction, "out");
    assert_eq!(movement.quantity_milli, 500);
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        2000
    );
    let history = list_movements(
        &conn,
        &MovementQuery {
            search: None,
            product_id: Some(id),
            direction: Some("out".into()),
        },
    )
    .expect("historique");
    assert_eq!(history.len(), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_sortie_superieure_au_stock_est_refusee() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic");
    record_entry(&mut conn, draft(id, "1")).expect("entrée");
    let error = record_exit(&mut conn, draft(id, "1.001")).expect_err("sortie");
    assert_eq!(
        error,
        StockError::Insufficient("Le stock disponible est insuffisant.".into())
    );
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        1000
    );
    assert!(
        list_movements(
            &conn,
            &MovementQuery {
                search: None,
                product_id: Some(id),
                direction: Some("out".into()),
            },
        )
        .expect("historique")
        .is_empty()
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_les_quantites_nulles_negatives_et_invalides() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic");
    let zero = record_entry(&mut conn, draft(id, "0")).expect_err("zéro");
    assert_eq!(
        zero,
        StockError::Invalid("La quantité doit être strictement positive.".into())
    );
    let negative = record_entry(&mut conn, draft(id, "-2")).expect_err("négatif");
    assert_eq!(
        negative,
        StockError::Invalid("La quantité doit être strictement positive.".into())
    );
    for value in ["", "abc", "NaN", "inf", "1.2345", "1,2.3"] {
        let error = record_entry(&mut conn, draft(id, value)).expect_err(value);
        assert!(matches!(error, StockError::Invalid(_)));
        assert!(!error.to_string().to_lowercase().contains("sqlite"));
    }
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        0
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_un_produit_inexistant_ou_desactive_sans_perdre_l_historique() {
    let (dir, mut conn) = open();
    let missing = record_entry(&mut conn, draft(999, "1")).expect_err("absent");
    assert_eq!(
        missing,
        StockError::NotFound("Ce produit est introuvable.".into())
    );

    let id = product(&mut conn, "Mastic");
    record_entry(&mut conn, draft(id, "4")).expect("entrée");
    set_product_active(&mut conn, id, false).expect("désactivation");
    let inactive = record_exit(&mut conn, draft(id, "1")).expect_err("désactivé");
    assert_eq!(
        inactive,
        StockError::Inactive("Ce produit est désactivé.".into())
    );
    let stock = get_product_stock(&conn, id).expect("stock conservé");
    assert_eq!(stock.quantity_milli, 4000);
    assert!(!stock.is_active);
    let history = list_movements(
        &conn,
        &MovementQuery {
            search: None,
            product_id: Some(id),
            direction: None,
        },
    )
    .expect("historique");
    assert_eq!(history.len(), 1);
    let detail = get_movement(&conn, history[0].id).expect("détail");
    assert_eq!(detail.quantity_milli, 4000);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn le_stock_et_l_historique_survivent_a_la_reouverture() {
    let dir = temp_dir();
    let id = {
        let mut conn = connect(&dir).expect("connexion");
        let id = product(&mut conn, "Mastic");
        record_entry(&mut conn, draft(id, "3.25")).expect("entrée");
        record_exit(&mut conn, draft(id, "0.25")).expect("sortie");
        id
    };
    let conn = connect(&dir).expect("réouverture");
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        3000
    );
    assert_eq!(
        list_movements(
            &conn,
            &MovementQuery {
                search: None,
                product_id: None,
                direction: None,
            },
        )
        .expect("historique")
        .len(),
        2
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_erreur_pendant_l_enregistrement_n_ecrit_rien() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic");
    conn.execute_batch(
        "CREATE TRIGGER abort_balance
         BEFORE UPDATE ON stock_balances
         BEGIN
             SELECT RAISE(ABORT, 'forced');
         END;",
    )
    .expect("déclencheur");
    let error = record_entry(&mut conn, draft(id, "1")).expect_err("échec");
    assert_eq!(
        error,
        StockError::Unavailable("Le mouvement n'a pas pu être enregistré.".into())
    );
    assert!(!error.to_string().to_lowercase().contains("sqlite"));
    assert!(!error.to_string().contains("forced"));
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        0
    );
    let movements: i64 = conn
        .query_row("SELECT COUNT(*) FROM stock_movements", [], |row| row.get(0))
        .expect("compte");
    assert_eq!(movements, 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn deux_sorties_concurrentes_ne_rendent_pas_le_stock_negatif() {
    let dir = temp_dir();
    let id = {
        let mut conn = connect(&dir).expect("connexion");
        let id = product(&mut conn, "Mastic");
        record_entry(&mut conn, draft(id, "10")).expect("entrée");
        id
    };
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let dir = dir.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let mut conn = connect(&dir).expect("connexion");
            barrier.wait();
            record_exit(&mut conn, draft(id, "10"))
        }));
    }
    let mut results = handles
        .into_iter()
        .map(|handle| handle.join().expect("fil"))
        .collect::<Vec<_>>();
    results.sort_by_key(|result| result.is_err());
    assert!(results[0].is_ok());
    assert_eq!(
        results[1].as_ref().expect_err("seconde sortie"),
        &StockError::Insufficient("Le stock disponible est insuffisant.".into())
    );
    let conn = connect(&dir).expect("relecture");
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        0
    );
    let exits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM stock_movements WHERE direction = 'out'",
            [],
            |row| row.get(0),
        )
        .expect("sorties");
    assert_eq!(exits, 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn le_tableau_de_bord_separe_rupture_et_alerte() {
    let (dir, mut conn) = open();
    let rupture = product(&mut conn, "Vide");
    let alerte = product(&mut conn, "Bas");
    record_entry(&mut conn, draft(alerte, "3")).expect("entrée basse");
    set_reorder_threshold(&mut conn, alerte, Some("5")).expect("seuil bas");
    let confort = product(&mut conn, "Haut");
    record_entry(&mut conn, draft(confort, "8")).expect("entrée haute");
    set_reorder_threshold(&mut conn, confort, Some("5")).expect("seuil haut");
    let both = product(&mut conn, "Épuisé");
    set_reorder_threshold(&mut conn, both, Some("1")).expect("seuil nul");
    let archive = product(&mut conn, "Archive");
    set_reorder_threshold(&mut conn, archive, Some("0")).expect("seuil archive");
    set_product_active(&mut conn, archive, false).expect("désactivation");

    let overview = stock_overview(&conn).expect("indicateurs");
    assert_eq!(overview.active_products, 4);
    assert_eq!(overview.out_of_stock, 2);
    assert_eq!(overview.reorder_alerts, 2);
    assert_eq!(overview.movements_last_7_days, 2);
    assert_eq!(overview.recent.len(), 2);
    assert_eq!(overview.recent[0].product_id, confort);
    assert!(
        !overview
            .recent
            .iter()
            .any(|movement| movement.product_id == archive)
    );
    assert_eq!(
        get_product_stock(&conn, rupture)
            .expect("vide")
            .quantity_milli,
        0
    );
    let _ = fs::remove_dir_all(dir);
}
