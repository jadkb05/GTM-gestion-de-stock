use super::*;
use crate::catalogue::{ProductDraft, create_product, set_product_active};
use crate::connect;
use crate::stock::{
    MovementDraft, SheetQuery, get_product_stock, get_stock_sheet, list_sheet_movements,
    record_entry, record_exit,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("horloge")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gtm-inventory-{nanos}-{id}"));
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

fn supplier_id(conn: &rusqlite::Connection) -> i64 {
    conn.query_row(
        "SELECT id FROM suppliers WHERE name = 'Selena'",
        [],
        |row| row.get(0),
    )
    .expect("fournisseur")
}

fn product(conn: &mut rusqlite::Connection, name: &str) -> i64 {
    let type_id = type_id(conn);
    let supplier_id = supplier_id(conn);
    create_product(
        conn,
        ProductDraft {
            name: name.into(),
            reference: Some(format!("REF-{name}")),
            type_id,
            supplier_id: Some(supplier_id),
            description: None,
        },
    )
    .expect("produit")
    .id
}

fn enter(conn: &mut rusqlite::Connection, product_id: i64, quantity: &str) {
    record_entry(
        conn,
        MovementDraft {
            product_id,
            quantity: quantity.into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("entrée");
}

fn leave(conn: &mut rusqlite::Connection, product_id: i64, quantity: &str) {
    record_exit(
        conn,
        MovementDraft {
            product_id,
            quantity: quantity.into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("sortie");
}

fn started(conn: &mut rusqlite::Connection, product_id: i64) -> (i64, i64) {
    let inventory = create_inventory(
        conn,
        InventoryDraft {
            name: Some("Inventaire mars".into()),
            comment: Some("Zone A".into()),
        },
    )
    .expect("inventaire");
    let detail = add_inventory_products(conn, inventory.id, &[product_id]).expect("ligne");
    let line_id = detail.lines[0].id;
    (inventory.id, line_id)
}

fn corrections(conn: &rusqlite::Connection, product_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM stock_movements WHERE product_id = ?1 AND source = 'inventory'",
        [product_id],
        |row| row.get(0),
    )
    .expect("corrections")
}

#[test]
fn creation_d_un_inventaire() {
    let (dir, mut conn) = open();
    let created = create_inventory(
        &mut conn,
        InventoryDraft {
            name: Some("  Inventaire entrepôt  ".into()),
            comment: None,
        },
    )
    .expect("création");
    assert_eq!(created.name.as_deref(), Some("Inventaire entrepôt"));
    assert_eq!(created.status, "draft");
    assert!(created.validated_at.is_none());
    assert!(created.lines.is_empty());
    let listed = list_inventories(&conn).expect("liste");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
    assert_eq!(listed[0].product_count, 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn creation_de_lignes_et_instantane() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic");
    enter(&mut conn, id, "12.5");
    let (inventory_id, _) = started(&mut conn, id);
    let detail = get_inventory(&conn, inventory_id).expect("détail");
    assert_eq!(detail.lines.len(), 1);
    assert_eq!(detail.lines[0].theoretical_milli, 12_500);
    assert_eq!(detail.lines[0].current_milli, 12_500);
    assert!(!detail.lines[0].drifted);
    assert!(detail.lines[0].counted_milli.is_none());
    assert_eq!(
        detail.lines[0].product_reference.as_deref(),
        Some("REF-Mastic")
    );
    assert_eq!(detail.lines[0].type_name, "Autre");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn saisie_nulle_decimale_et_refus_de_precision() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Joint");
    let (inventory_id, line_id) = started(&mut conn, id);
    let zero = save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("0".into()),
        }],
    )
    .expect("zéro");
    assert_eq!(zero.lines[0].counted_milli, Some(0));
    assert_eq!(zero.lines[0].variance_milli, Some(0));

    let decimal = save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("1,25".into()),
        }],
    )
    .expect("décimal");
    assert_eq!(decimal.lines[0].counted_milli, Some(1_250));

    let error = save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("1.2345".into()),
        }],
    )
    .expect_err("trop de décimales");
    assert!(error.to_string().contains("3 décimales"));
    let kept = get_inventory(&conn, inventory_id).expect("inchangé");
    assert_eq!(kept.lines[0].counted_milli, Some(1_250));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn calcul_exact_d_un_surplus_d_un_manque_et_d_un_ecart_nul() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Silicone");
    enter(&mut conn, id, "12.5");
    let (inventory_id, line_id) = started(&mut conn, id);

    let shortage = save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("10".into()),
        }],
    )
    .expect("manque");
    assert_eq!(shortage.lines[0].variance_milli, Some(-2_500));
    assert_eq!(shortage.counts.shortage, 1);

    let surplus = save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("13".into()),
        }],
    )
    .expect("surplus");
    assert_eq!(surplus.lines[0].variance_milli, Some(500));
    assert_eq!(surplus.counts.surplus, 1);

    let even = save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("12.5".into()),
        }],
    )
    .expect("nul");
    assert_eq!(even.lines[0].variance_milli, Some(0));
    assert_eq!(even.counts.unchanged, 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_la_validation_d_un_inventaire_incomplet() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Bande");
    let (inventory_id, _) = started(&mut conn, id);
    let error = validate_inventory(&mut conn, inventory_id).expect_err("incomplet");
    assert!(error.to_string().contains("quantité comptée"));
    assert_eq!(
        get_inventory(&conn, inventory_id)
            .expect("brouillon")
            .status,
        "draft"
    );
    let empty = create_inventory(
        &mut conn,
        InventoryDraft {
            name: None,
            comment: None,
        },
    )
    .expect("vide");
    let error = validate_inventory(&mut conn, empty.id).expect_err("sans produit");
    assert!(error.to_string().contains("au moins un produit"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn validation_sans_ecart_ne_cree_pas_de_mouvement() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Primaire");
    enter(&mut conn, id, "4");
    let (inventory_id, line_id) = started(&mut conn, id);
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("4".into()),
        }],
    )
    .expect("comptage");
    let validated = validate_inventory(&mut conn, inventory_id).expect("validation");
    assert_eq!(validated.status, "validated");
    assert!(validated.validated_at.is_some());
    assert_eq!(corrections(&conn, id), 0);
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        4_000
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn validation_avec_surplus_cree_une_correction_et_met_a_jour_la_fiche() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Cartouche");
    enter(&mut conn, id, "10");
    let (inventory_id, line_id) = started(&mut conn, id);
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("12.5".into()),
        }],
    )
    .expect("comptage");
    validate_inventory(&mut conn, inventory_id).expect("validation");
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        12_500
    );
    let movement: (String, i64, i64, i64, String) = conn
        .query_row(
            "SELECT direction, quantity_milli, variance_milli, inventory_id, source
             FROM stock_movements WHERE product_id = ?1 AND source = 'inventory'",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("mouvement");
    assert_eq!(movement.0, "in");
    assert_eq!(movement.1, 2_500);
    assert_eq!(movement.2, 2_500);
    assert_eq!(movement.3, inventory_id);
    assert_eq!(movement.4, "inventory");
    let sheet = get_stock_sheet(&conn, id).expect("fiche");
    assert_eq!(sheet.quantity_milli, 12_500);
    assert_eq!(sheet.entries_milli, 12_500);
    let page = list_sheet_movements(
        &conn,
        &SheetQuery {
            product_id: id,
            direction: None,
            search: None,
            from_date: None,
            to_date: None,
            page: 1,
        },
    )
    .expect("historique");
    assert!(page.lines.iter().any(|line| line.source == "inventory"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn validation_avec_manque_diminue_le_stock() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Fond de joint");
    enter(&mut conn, id, "12.5");
    let (inventory_id, line_id) = started(&mut conn, id);
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("10".into()),
        }],
    )
    .expect("comptage");
    validate_inventory(&mut conn, inventory_id).expect("validation");
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        10_000
    );
    let movement: (String, i64, i64) = conn
        .query_row(
            "SELECT direction, quantity_milli, variance_milli
             FROM stock_movements WHERE source = 'inventory'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("mouvement");
    assert_eq!(movement, ("out".into(), 2_500, -2_500));
    let sheet = get_stock_sheet(&conn, id).expect("fiche");
    assert_eq!(sheet.exits_milli, 2_500);
    assert_eq!(sheet.quantity_milli, 10_000);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_une_seconde_validation_et_une_modification() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mousse");
    let (inventory_id, line_id) = started(&mut conn, id);
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("0".into()),
        }],
    )
    .expect("comptage");
    validate_inventory(&mut conn, inventory_id).expect("validation");
    let again = validate_inventory(&mut conn, inventory_id).expect_err("seconde");
    assert!(again.to_string().contains("déjà validé"));
    let edit = save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("1".into()),
        }],
    )
    .expect_err("modification");
    assert!(edit.to_string().contains("ne peut plus être modifié"));
    let add = add_inventory_products(&mut conn, inventory_id, &[id]).expect_err("ajout");
    assert!(add.to_string().contains("ne peut plus être modifié"));
    assert_eq!(
        get_inventory(&conn, inventory_id).expect("figé").lines[0].counted_milli,
        Some(0)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_correction_en_echec_annule_toute_la_validation() {
    let (dir, mut conn) = open();
    let first = product(&mut conn, "A");
    let second = product(&mut conn, "B");
    enter(&mut conn, first, "5");
    enter(&mut conn, second, "5");
    let inventory = create_inventory(
        &mut conn,
        InventoryDraft {
            name: None,
            comment: None,
        },
    )
    .expect("inventaire");
    let detail = add_inventory_products(&mut conn, inventory.id, &[first, second]).expect("lignes");
    let counts: Vec<CountInput> = detail
        .lines
        .iter()
        .map(|line| CountInput {
            line_id: line.id,
            quantity: Some("6".into()),
        })
        .collect();
    save_inventory_counts(&mut conn, inventory.id, &counts).expect("comptage");
    conn.execute_batch(
        "CREATE TRIGGER abort_balance_change
         BEFORE UPDATE ON stock_balances
         WHEN NEW.quantity_milli <> OLD.quantity_milli
         BEGIN
             SELECT RAISE(ABORT, 'forced');
         END;",
    )
    .expect("déclencheur");
    let error = validate_inventory(&mut conn, inventory.id).expect_err("échec");
    assert!(!error.to_string().to_lowercase().contains("sqlite"));
    assert_eq!(
        get_inventory(&conn, inventory.id)
            .expect("brouillon")
            .status,
        "draft"
    );
    assert!(
        get_inventory(&conn, inventory.id)
            .expect("date")
            .validated_at
            .is_none()
    );
    assert_eq!(corrections(&conn, first), 0);
    assert_eq!(corrections(&conn, second), 0);
    assert_eq!(
        get_product_stock(&conn, first).expect("a").quantity_milli,
        5_000
    );
    assert_eq!(
        get_product_stock(&conn, second).expect("b").quantity_milli,
        5_000
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn detecte_un_stock_modifie_et_refuse_une_reference_perimee() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Plot");
    enter(&mut conn, id, "10");
    let (inventory_id, line_id) = started(&mut conn, id);
    enter(&mut conn, id, "2");
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("10".into()),
        }],
    )
    .expect("comptage");
    let detail = get_inventory(&conn, inventory_id).expect("détail");
    assert_eq!(detail.lines[0].theoretical_milli, 10_000);
    assert_eq!(detail.lines[0].current_milli, 12_000);
    assert_eq!(detail.lines[0].counted_milli, Some(10_000));
    assert!(detail.lines[0].drifted);
    assert_eq!(detail.counts.unchanged, 0);
    assert_eq!(detail.counts.stale, 1);
    let drifts = check_inventory_drift(&conn, inventory_id).expect("écarts de référence");
    assert_eq!(drifts.len(), 1);
    assert_eq!(drifts[0].current_milli, 12_000);
    let error = validate_inventory(&mut conn, inventory_id).expect_err("périmé");
    assert!(error.to_string().contains("a changé"));
    assert_eq!(corrections(&conn, id), 0);
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        12_000
    );
    assert_eq!(
        get_inventory(&conn, inventory_id)
            .expect("brouillon")
            .status,
        "draft"
    );

    let refreshed = refresh_inventory_line(&mut conn, line_id, false).expect("référence");
    assert_eq!(refreshed.lines[0].theoretical_milli, 12_000);
    assert_eq!(refreshed.lines[0].counted_milli, Some(10_000));
    assert_eq!(refreshed.lines[0].variance_milli, Some(-2_000));
    assert!(!refreshed.lines[0].drifted);
    validate_inventory(&mut conn, inventory_id).expect("correction");
    assert_eq!(
        get_product_stock(&conn, id).expect("après").quantity_milli,
        10_000
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn empeche_les_lignes_en_double_et_persiste_apres_reouverture() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Vis");
    let (inventory_id, line_id) = started(&mut conn, id);
    let duplicate = add_inventory_products(&mut conn, inventory_id, &[id]).expect_err("doublon");
    assert!(duplicate.to_string().contains("déjà"));
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("3".into()),
        }],
    )
    .expect("comptage");
    drop(conn);
    let conn = connect(&dir).expect("réouverture");
    let detail = get_inventory(&conn, inventory_id).expect("relu");
    assert_eq!(detail.lines.len(), 1);
    assert_eq!(detail.lines[0].counted_milli, Some(3_000));
    assert_eq!(detail.status, "draft");
    drop(conn);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn seuil_cree_modifie_desactive_et_calcule_l_alerte() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Colle");
    assert!(
        list_reorder_alerts(
            &conn,
            &AlertQuery {
                search: None,
                type_id: None,
                supplier_id: None,
                sort: None
            }
        )
        .expect("vide")
        .is_empty()
    );
    set_reorder_threshold(&mut conn, id, Some("5")).expect("création");
    let below = list_reorder_alerts(&conn, &empty_query()).expect("sous le seuil");
    assert_eq!(below.len(), 1);
    assert_eq!(below[0].threshold_milli, 5_000);
    assert_eq!(below[0].quantity_milli, 0);

    set_reorder_threshold(&mut conn, id, Some("8")).expect("modification");
    assert_eq!(threshold_of(&conn, id), Some(8_000));

    enter(&mut conn, id, "8");
    let equal = list_reorder_alerts(&conn, &empty_query()).expect("égal");
    assert_eq!(equal.len(), 1);
    assert_eq!(equal[0].quantity_milli, 8_000);

    enter(&mut conn, id, "1");
    assert!(
        list_reorder_alerts(&conn, &empty_query())
            .expect("au-dessus")
            .is_empty()
    );

    set_reorder_threshold(&mut conn, id, None).expect("désactivation");
    assert_eq!(threshold_of(&conn, id), None);
    assert!(
        list_reorder_alerts(&conn, &empty_query())
            .expect("absent")
            .is_empty()
    );

    set_reorder_threshold(&mut conn, id, Some("0")).expect("seuil nul");
    assert!(
        list_reorder_alerts(&conn, &empty_query())
            .expect("stock positif")
            .is_empty()
    );
    let other = product(&mut conn, "Solvant");
    set_reorder_threshold(&mut conn, other, Some("0")).expect("zéro");
    let zero = list_reorder_alerts(&conn, &empty_query()).expect("stock nul");
    assert_eq!(zero.len(), 1);
    assert_eq!(zero[0].product_id, other);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn un_produit_desactive_sort_des_alertes_mais_garde_son_seuil() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Inactif");
    set_reorder_threshold(&mut conn, id, Some("2")).expect("seuil");
    set_product_active(&mut conn, id, false).expect("désactivation");
    assert!(
        list_reorder_alerts(&conn, &empty_query())
            .expect("liste")
            .is_empty()
    );
    assert_eq!(count_reorder_alerts(&conn).expect("compte"), 0);
    assert_eq!(threshold_of(&conn, id), Some(2_000));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn l_alerte_suit_une_entree_une_sortie_et_une_correction() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Ruban");
    set_reorder_threshold(&mut conn, id, Some("5")).expect("seuil");
    enter(&mut conn, id, "3");
    assert_eq!(count_reorder_alerts(&conn).expect("sous le seuil"), 1);
    enter(&mut conn, id, "3");
    assert_eq!(count_reorder_alerts(&conn).expect("après entrée"), 0);
    leave(&mut conn, id, "2");
    assert_eq!(count_reorder_alerts(&conn).expect("après sortie"), 1);

    enter(&mut conn, id, "4");
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        8_000
    );
    assert_eq!(count_reorder_alerts(&conn).expect("au-dessus"), 0);
    let (inventory_id, line_id) = started(&mut conn, id);
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("4".into()),
        }],
    )
    .expect("comptage");
    validate_inventory(&mut conn, inventory_id).expect("correction");
    assert_eq!(
        get_product_stock(&conn, id)
            .expect("après correction")
            .quantity_milli,
        4_000
    );
    assert_eq!(
        count_reorder_alerts(&conn).expect("alerte après correction"),
        1
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_une_correction_sur_un_produit_desactive_sans_changer_le_stock() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Gel");
    enter(&mut conn, id, "5");
    let (inventory_id, line_id) = started(&mut conn, id);
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("4".into()),
        }],
    )
    .expect("comptage");
    set_product_active(&mut conn, id, false).expect("désactivation");
    let error = validate_inventory(&mut conn, inventory_id).expect_err("refus");
    assert!(error.to_string().contains("désactivé"));
    assert_eq!(
        get_product_stock(&conn, id).expect("stock").quantity_milli,
        5_000
    );
    let status: String = conn
        .query_row(
            "SELECT status FROM inventories WHERE id = ?1",
            [inventory_id],
            |row| row.get(0),
        )
        .expect("statut");
    assert_eq!(status, "draft");
    let corrections: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM stock_movements WHERE source = 'inventory'",
            [],
            |row| row.get(0),
        )
        .expect("corrections");
    assert_eq!(corrections, 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn le_solde_reste_egal_a_l_historique_apres_une_sequence_complete() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic");
    enter(&mut conn, id, "10");
    leave(&mut conn, id, "3");
    let (inventory_id, line_id) = started(&mut conn, id);
    save_inventory_counts(
        &mut conn,
        inventory_id,
        &[CountInput {
            line_id,
            quantity: Some("6".into()),
        }],
    )
    .expect("comptage");
    validate_inventory(&mut conn, inventory_id).expect("validation");
    let balance = get_product_stock(&conn, id).expect("stock").quantity_milli;
    let ledger: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(CASE direction WHEN 'in' THEN quantity_milli ELSE -quantity_milli END), 0)
             FROM stock_movements WHERE product_id = ?1",
            [id],
            |row| row.get(0),
        )
        .expect("historique");
    assert_eq!(balance, 6_000);
    assert_eq!(ledger, balance);
    let reopened = connect(&dir).expect("réouverture");
    assert_eq!(
        get_product_stock(&reopened, id)
            .expect("stock rouvert")
            .quantity_milli,
        6_000
    );
    drop(reopened);
    let _ = fs::remove_dir_all(dir);
}

fn empty_query() -> AlertQuery {
    AlertQuery {
        search: None,
        type_id: None,
        supplier_id: None,
        sort: None,
    }
}

fn threshold_of(conn: &rusqlite::Connection, product_id: i64) -> Option<i64> {
    conn.query_row(
        "SELECT reorder_threshold_milli FROM products WHERE id = ?1",
        [product_id],
        |row| row.get(0),
    )
    .expect("seuil")
}

#[test]
fn une_sortie_egale_au_stock_vide_le_produit_et_le_seuil_zero_alerte() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Joint");
    enter(&mut conn, id, "2.5");
    set_reorder_threshold(&mut conn, id, Some("0")).expect("seuil zéro");
    assert!(
        list_reorder_alerts(&conn, &empty_query())
            .expect("stock positif")
            .is_empty()
    );

    record_exit(
        &mut conn,
        MovementDraft {
            product_id: id,
            quantity: "2,5".into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("sortie de tout le stock");
    assert_eq!(get_product_stock(&conn, id).expect("stock").quantity_milli, 0);

    let alerts = list_reorder_alerts(&conn, &empty_query()).expect("rupture");
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].quantity_milli, 0);
    let overview = crate::stock::stock_overview(&conn).expect("tableau de bord");
    assert_eq!(overview.out_of_stock, 1);
    assert_eq!(overview.reorder_alerts, 1);
    fs::remove_dir_all(dir).ok();
}
