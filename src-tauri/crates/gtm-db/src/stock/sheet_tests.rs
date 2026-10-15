use super::*;
use crate::catalogue::{ProductDraft, create_product, set_product_active};
use crate::connect;
use crate::stock::{MovementDraft as Draft, record_entry, record_exit};
use rusqlite::params;
use std::fs;

fn temp() -> (std::path::PathBuf, rusqlite::Connection) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("horloge")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("gtm-sheet-{nanos}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("répertoire");
    let conn = connect(&dir).expect("connexion");
    (dir, conn)
}

fn product(conn: &mut rusqlite::Connection, name: &str) -> i64 {
    let type_id = conn
        .query_row(
            "SELECT id FROM product_types WHERE name = 'Silicone'",
            [],
            |row| row.get(0),
        )
        .expect("type");
    let supplier_id = conn
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
            reference: Some("SIL-01".into()),
            type_id,
            supplier_id: Some(supplier_id),
            description: None,
        },
    )
    .expect("produit")
    .id
}

fn query(product_id: i64) -> SheetQuery {
    SheetQuery {
        product_id,
        direction: None,
        search: None,
        from_date: None,
        to_date: None,
        page: 1,
    }
}

#[test]
fn une_fiche_sans_mouvement_a_un_stock_nul() {
    let (dir, mut conn) = temp();
    let id = product(&mut conn, "Mastic");
    let sheet = get_stock_sheet(&conn, id).expect("fiche");
    assert_eq!(sheet.name, "Mastic");
    assert_eq!(sheet.reference.as_deref(), Some("SIL-01"));
    assert_eq!(sheet.type_name, "Silicone");
    assert_eq!(sheet.supplier_name.as_deref(), Some("Selena"));
    assert!(sheet.is_active);
    assert_eq!(sheet.quantity_milli, 0);
    assert_eq!(sheet.movement_count, 0);
    assert_eq!(sheet.entries_milli, 0);
    assert_eq!(sheet.exits_milli, 0);
    let page = list_sheet_movements(&conn, &query(id)).expect("page");
    assert!(page.lines.is_empty());
    assert_eq!(page.total, 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn les_soldes_les_totaux_et_le_detail_sont_exacts() {
    let (dir, mut conn) = temp();
    let id = product(&mut conn, "Mastic");
    record_entry(
        &mut conn,
        Draft {
            product_id: id,
            quantity: "1.5".into(),
            reason: Some("Réception".into()),
            document_reference: Some("BL-10".into()),
            comment: Some("Quai 2".into()),
        },
    )
    .expect("entrée");
    let exit = record_exit(
        &mut conn,
        Draft {
            product_id: id,
            quantity: "0.25".into(),
            reason: Some("Chantier".into()),
            document_reference: Some("BS-2".into()),
            comment: Some("Équipe A".into()),
        },
    )
    .expect("sortie");
    record_entry(
        &mut conn,
        Draft {
            product_id: id,
            quantity: "0.001".into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("millième");

    let sheet = get_stock_sheet(&conn, id).expect("fiche");
    assert_eq!(sheet.quantity_milli, 1251);
    assert_eq!(sheet.entries_milli, 1501);
    assert_eq!(sheet.exits_milli, 250);
    assert_eq!(sheet.movement_count, 3);

    let page = list_sheet_movements(&conn, &query(id)).expect("page");
    assert_eq!(page.total, 3);
    assert_eq!(
        page.lines
            .iter()
            .map(|line| line.balance_after_milli)
            .collect::<Vec<_>>(),
        vec![1251, 1250, 1500]
    );
    let oldest = page.lines.last().expect("ligne");
    assert_eq!(oldest.quantity_milli, 1500);
    assert_eq!(oldest.direction, "in");

    let detail = get_movement_detail(&conn, exit.id).expect("détail");
    assert_eq!(detail.balance_after_milli, 1250);
    assert_eq!(detail.movement.quantity_milli, 250);
    assert_eq!(detail.movement.comment.as_deref(), Some("Équipe A"));
    assert_eq!(detail.movement.document_reference.as_deref(), Some("BS-2"));
    assert_eq!(detail.movement.reason.as_deref(), Some("Chantier"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn un_meme_horodatage_garde_un_ordre_stable() {
    let (dir, mut conn) = temp();
    let id = product(&mut conn, "Mastic");
    for quantity in [1000, 500] {
        conn.execute(
            "INSERT INTO stock_movements (
                product_id, direction, quantity_milli, created_at
             ) VALUES (?1, 'in', ?2, '2026-03-01T08:00:00.000Z')",
            params![id, quantity],
        )
        .expect("mouvement");
    }
    conn.execute(
        "UPDATE stock_balances SET quantity_milli = 1500 WHERE product_id = ?1",
        params![id],
    )
    .expect("solde");
    let page = list_sheet_movements(&conn, &query(id)).expect("page");
    assert_eq!(page.lines.len(), 2);
    assert!(page.lines[0].id > page.lines[1].id);
    assert_eq!(page.lines[0].balance_after_milli, 1500);
    assert_eq!(page.lines[1].balance_after_milli, 1000);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn les_filtres_et_la_pagination_conservent_les_soldes() {
    let (dir, mut conn) = temp();
    let id = product(&mut conn, "Mastic");
    conn.execute(
        "INSERT INTO stock_movements (
            product_id, direction, quantity_milli, reason, document_reference, created_at
         ) VALUES
         (?1, 'in', 1000, 'Réception', 'BL-1', '2026-01-10T10:00:00.000Z'),
         (?1, 'out', 200, 'Pose', 'BS-9', '2026-02-10T10:00:00.000Z'),
         (?1, 'in', 50, 'Retour', 'AV-3', '2026-03-10T10:00:00.000Z')",
        params![id],
    )
    .expect("historique");
    conn.execute(
        "UPDATE stock_balances SET quantity_milli = 850 WHERE product_id = ?1",
        params![id],
    )
    .expect("solde");

    let outs = list_sheet_movements(
        &conn,
        &SheetQuery {
            direction: Some("out".into()),
            ..query(id)
        },
    )
    .expect("sorties");
    assert_eq!(outs.total, 1);
    assert_eq!(outs.lines[0].balance_after_milli, 800);
    assert_eq!(outs.lines[0].quantity_milli, 200);

    let period = list_sheet_movements(
        &conn,
        &SheetQuery {
            from_date: Some("2026-02-01".into()),
            to_date: Some("2026-02-28".into()),
            ..query(id)
        },
    )
    .expect("période");
    assert_eq!(period.total, 1);
    assert_eq!(period.lines[0].document_reference.as_deref(), Some("BS-9"));

    let text = list_sheet_movements(
        &conn,
        &SheetQuery {
            search: Some("av-3".into()),
            ..query(id)
        },
    )
    .expect("recherche");
    assert_eq!(text.total, 1);
    assert_eq!(text.lines[0].reason.as_deref(), Some("Retour"));
    assert_eq!(text.lines[0].balance_after_milli, 850);

    let reversed = list_sheet_movements(
        &conn,
        &SheetQuery {
            from_date: Some("2026-04-01".into()),
            to_date: Some("2026-01-01".into()),
            ..query(id)
        },
    )
    .expect_err("période");
    assert_eq!(
        reversed,
        StockError::Invalid("La date de fin précède la date de début.".into())
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plus_de_200_mouvements_ne_limitent_pas_la_fiche() {
    let (dir, mut conn) = temp();
    let id = product(&mut conn, "Mastic");
    let tx = conn.transaction().expect("transaction");
    for index in 1..=205 {
        let stamp = format!("2026-01-01T{:02}:{:02}:00.000Z", index / 60, index % 60);
        tx.execute(
            "INSERT INTO stock_movements (product_id, direction, quantity_milli, created_at)
             VALUES (?1, 'in', 1, ?2)",
            params![id, stamp],
        )
        .expect("ligne");
    }
    tx.execute(
        "UPDATE stock_balances SET quantity_milli = 205 WHERE product_id = ?1",
        params![id],
    )
    .expect("solde");
    tx.commit().expect("validation");

    let sheet = get_stock_sheet(&conn, id).expect("fiche");
    assert_eq!(sheet.movement_count, 205);
    assert_eq!(sheet.entries_milli, 205);
    assert_eq!(sheet.exits_milli, 0);
    assert_eq!(sheet.quantity_milli, 205);

    let first = list_sheet_movements(&conn, &query(id)).expect("page 1");
    assert_eq!(first.total, 205);
    assert_eq!(first.lines.len(), 50);
    assert_eq!(first.lines[0].balance_after_milli, 205);

    let second = list_sheet_movements(
        &conn,
        &SheetQuery {
            page: 2,
            ..query(id)
        },
    )
    .expect("page 2");
    assert_eq!(second.lines[0].balance_after_milli, 155);
    assert_eq!(second.page_size, 50);

    let general = crate::list_movements(
        &conn,
        &crate::MovementQuery {
            search: None,
            product_id: Some(id),
            direction: None,
        },
    )
    .expect("historique général");
    assert_eq!(general.len(), 200);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn un_produit_desactive_conserve_sa_fiche_et_un_absent_est_refuse() {
    let (dir, mut conn) = temp();
    let id = product(&mut conn, "Mastic");
    record_entry(
        &mut conn,
        Draft {
            product_id: id,
            quantity: "2".into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("entrée");
    set_product_active(&mut conn, id, false).expect("désactivation");
    let sheet = get_stock_sheet(&conn, id).expect("fiche");
    assert!(!sheet.is_active);
    assert_eq!(sheet.quantity_milli, 2000);
    assert_eq!(sheet.movement_count, 1);
    let missing = get_stock_sheet(&conn, 999).expect_err("absent");
    assert_eq!(
        missing,
        StockError::NotFound("Ce produit est introuvable.".into())
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn le_filtre_de_periode_utilise_la_date_locale_du_poste() {
    let (dir, mut conn) = temp();
    let id = product(&mut conn, "Mastic");
    // Mouvement enregistré à 22:30 UTC : en France, il appartient au lendemain.
    let stamp = "2026-06-30T22:30:00.000Z";
    conn.execute(
        "INSERT INTO stock_movements (product_id, direction, quantity_milli, created_at)
         VALUES (?1, 'in', 1000, ?2)",
        params![id, stamp],
    )
    .expect("mouvement");
    let local_day: String = conn
        .query_row("SELECT date(?1, 'localtime')", params![stamp], |row| row.get(0))
        .expect("jour local");

    let same_day = list_sheet_movements(
        &conn,
        &SheetQuery {
            from_date: Some(local_day.clone()),
            to_date: Some(local_day.clone()),
            ..query(id)
        },
    )
    .expect("jour local");
    assert_eq!(same_day.total, 1);

    if local_day != "2026-06-30" {
        let utc_day = list_sheet_movements(
            &conn,
            &SheetQuery {
                from_date: Some("2026-06-30".into()),
                to_date: Some("2026-06-30".into()),
                ..query(id)
            },
        )
        .expect("jour UTC");
        assert_eq!(utc_day.total, 0);
    }
    fs::remove_dir_all(dir).ok();
}
