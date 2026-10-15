use super::csv_io::read_csv;
use super::xlsx_io::read_xlsx;
use super::{
    ExportFormat, HistoryExportQuery, commit_article_import, export_catalogue, export_history,
    export_inventory, export_reorder, preview_article_import,
};
use crate::catalogue::{
    ActivityFilter, ProductDraft, ProductQuery, create_product, set_product_type_active,
    set_supplier_active,
};
use crate::connect;
use crate::stock::MovementQuery;
use crate::stock::{
    AlertQuery, CountInput, InventoryDraft, MovementDraft, add_inventory_products,
    create_inventory, get_product_stock, list_movements, record_entry, save_inventory_counts,
    set_reorder_threshold, validate_inventory,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("horloge")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gtm-exchange-{nanos}-{id}"));
    fs::create_dir_all(&path).expect("répertoire");
    path
}

fn open() -> (PathBuf, rusqlite::Connection) {
    let dir = temp_dir();
    let conn = connect(&dir).expect("connexion");
    (dir, conn)
}

fn ids(conn: &rusqlite::Connection) -> (i64, i64) {
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
    (type_id, supplier_id)
}

fn product(conn: &mut rusqlite::Connection, name: &str, reference: &str) -> i64 {
    let (type_id, supplier_id) = ids(conn);
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

fn catalogue_query(status: ActivityFilter) -> ProductQuery {
    ProductQuery {
        search: None,
        type_id: None,
        supplier_id: None,
        status,
    }
}

fn count(conn: &rusqlite::Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).expect("compte")
}

#[test]
fn exporte_un_catalogue_vide_puis_des_accents_et_une_quantite_exacte() {
    let (dir, mut conn) = open();
    let empty = dir.join("vide.csv");
    let outcome = export_catalogue(
        &conn,
        &empty,
        ExportFormat::Csv,
        &catalogue_query(ActivityFilter::All),
    )
    .expect("export vide");
    assert_eq!(outcome.rows, 0);
    let rows = read_csv(&fs::read(&empty).expect("lu")).expect("csv");
    assert_eq!(rows[0][2], "Nom");
    assert_eq!(rows.len(), 1);

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
    set_reorder_threshold(&mut conn, id, Some("3")).expect("seuil");
    let path = dir.join("catalogue.csv");
    let before = count(&conn, "SELECT COUNT(*) FROM stock_movements");
    export_catalogue(
        &conn,
        &path,
        ExportFormat::Csv,
        &catalogue_query(ActivityFilter::All),
    )
    .expect("export");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM stock_movements"), before);
    let rows = read_csv(&fs::read(&path).expect("lu")).expect("csv");
    let line = &rows[1];
    assert_eq!(line[2], "Étanchéité");
    assert_eq!(line[5], "12,25");
    assert_eq!(line[6], "3");
    assert_eq!(line[7], "Actif");
    assert!(
        fs::read(&path)
            .expect("octets")
            .starts_with(&[0xEF, 0xBB, 0xBF])
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn neutralise_une_cellule_dangereuse_en_csv_et_en_excel() {
    let (dir, mut conn) = open();
    product(&mut conn, "=CMD()", "REF");
    let csv_path = dir.join("dangereux.csv");
    let xlsx_path = dir.join("dangereux.xlsx");
    export_catalogue(
        &conn,
        &csv_path,
        ExportFormat::Csv,
        &catalogue_query(ActivityFilter::All),
    )
    .expect("csv");
    export_catalogue(
        &conn,
        &xlsx_path,
        ExportFormat::Xlsx,
        &catalogue_query(ActivityFilter::All),
    )
    .expect("xlsx");
    let csv = String::from_utf8(fs::read(&csv_path).expect("csv")).expect("utf8");
    assert!(csv.contains("'=CMD()"));
    let rows = read_xlsx(&xlsx_path).expect("xlsx");
    assert_eq!(rows[1][2], "=CMD()");
    assert_eq!(rows[0][0], "Identifiant");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn exporte_l_historique_complet_filtre_et_borne_par_une_periode() {
    let (dir, mut conn) = open();
    let first = product(&mut conn, "Colle", "COL");
    let second = product(&mut conn, "Mastic", "MAS");
    for _ in 0..201 {
        record_entry(
            &mut conn,
            MovementDraft {
                product_id: first,
                quantity: "1".into(),
                reason: Some("Lot".into()),
                document_reference: None,
                comment: None,
            },
        )
        .expect("entrée");
    }
    record_entry(
        &mut conn,
        MovementDraft {
            product_id: second,
            quantity: "2,5".into(),
            reason: Some("Autre".into()),
            document_reference: Some("BL-9".into()),
            comment: Some("ligne\nsuivante".into()),
        },
    )
    .expect("autre");
    let listed = list_movements(
        &conn,
        &MovementQuery {
            search: None,
            product_id: None,
            direction: None,
        },
    )
    .expect("liste");
    assert_eq!(listed.len(), 200);
    let all = dir.join("historique.csv");
    let outcome = export_history(
        &conn,
        &all,
        ExportFormat::Csv,
        &HistoryExportQuery {
            search: None,
            product_id: None,
            direction: None,
            from_date: None,
            to_date: None,
        },
    )
    .expect("complet");
    assert_eq!(outcome.rows, 202);
    assert!(outcome.scope.contains("Historique complet"));
    let filtered = dir.join("filtre.csv");
    let outcome = export_history(
        &conn,
        &filtered,
        ExportFormat::Csv,
        &HistoryExportQuery {
            search: Some("Mastic".into()),
            product_id: None,
            direction: Some("in".into()),
            from_date: Some("2000-01-01".into()),
            to_date: Some("2099-12-31".into()),
        },
    )
    .expect("filtré");
    assert_eq!(outcome.rows, 1);
    assert!(outcome.scope.contains("filtres"));
    let rows = read_csv(&fs::read(&filtered).expect("lu")).expect("csv");
    assert_eq!(rows[1][4], "2,5");
    assert!(rows[1][7].contains('\n'));
    let outside = dir.join("hors-periode.csv");
    let outcome = export_history(
        &conn,
        &outside,
        ExportFormat::Csv,
        &HistoryExportQuery {
            search: None,
            product_id: None,
            direction: None,
            from_date: Some("1990-01-01".into()),
            to_date: Some("1990-01-02".into()),
        },
    )
    .expect("période vide");
    assert_eq!(outcome.rows, 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn exporte_un_inventaire_incomplet_puis_un_inventaire_valide() {
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
    .expect("brouillon");
    let draft = add_inventory_products(&mut conn, draft.id, &[id]).expect("ligne");
    let incomplete = dir.join("incomplet.csv");
    export_inventory(&conn, &incomplete, ExportFormat::Csv, draft.id).expect("export brouillon");
    let rows = read_csv(&fs::read(&incomplete).expect("lu")).expect("csv");
    assert_eq!(rows[1][2], "Brouillon");
    assert_eq!(rows[1][6], "10");
    assert_eq!(rows[1][7], "");
    assert_eq!(rows[1][8], "");
    assert!(!rows[1][9].is_empty());

    let line_id = draft.lines[0].id;
    save_inventory_counts(
        &mut conn,
        draft.id,
        &[CountInput {
            line_id,
            quantity: Some("12,5".into()),
        }],
    )
    .expect("comptage");
    validate_inventory(&mut conn, draft.id).expect("validation");
    let validated = dir.join("valide.csv");
    export_inventory(&conn, &validated, ExportFormat::Csv, draft.id).expect("export validé");
    let rows = read_csv(&fs::read(&validated).expect("lu")).expect("csv");
    assert_eq!(rows[1][2], "Validé");
    assert_eq!(rows[1][6], "10");
    assert_eq!(rows[1][7], "12,5");
    assert_eq!(rows[1][8], "2,5");
    assert!(!rows[1][3].is_empty());

    let history = dir.join("corrections.csv");
    export_history(
        &conn,
        &history,
        ExportFormat::Csv,
        &HistoryExportQuery {
            search: None,
            product_id: None,
            direction: None,
            from_date: None,
            to_date: None,
        },
    )
    .expect("historique");
    let rows = read_csv(&fs::read(&history).expect("lu")).expect("csv");
    let correction = rows
        .iter()
        .find(|row| row[8] == "Inventaire")
        .expect("correction");
    assert_eq!(correction[3], "Entrée");
    assert_eq!(correction[4], "2,5");
    assert_eq!(correction[9], draft.id.to_string());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn exporte_les_alertes_courantes_sans_figer_un_etat() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Primaire", "PRI");
    record_entry(
        &mut conn,
        MovementDraft {
            product_id: id,
            quantity: "1".into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("entrée");
    set_reorder_threshold(&mut conn, id, Some("5")).expect("seuil");
    let path = dir.join("alertes.csv");
    let outcome = export_reorder(
        &conn,
        &path,
        ExportFormat::Csv,
        &AlertQuery {
            search: None,
            type_id: None,
            supplier_id: None,
            sort: None,
        },
    )
    .expect("alertes");
    assert_eq!(outcome.rows, 1);
    let rows = read_csv(&fs::read(&path).expect("lu")).expect("csv");
    assert_eq!(rows[1][0], "Primaire");
    assert_eq!(rows[1][4], "1");
    assert_eq!(rows[1][5], "5");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn le_pdf_porte_le_titre_les_accents_et_la_pagination() {
    let (dir, mut conn) = open();
    product(&mut conn, "Étanchéité", "ET");
    let path = dir.join("catalogue.pdf");
    export_catalogue(
        &conn,
        &path,
        ExportFormat::Pdf,
        &catalogue_query(ActivityFilter::All),
    )
    .expect("pdf");
    let bytes = fs::read(&path).expect("lu");
    assert!(bytes.starts_with(b"%PDF"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Catalogue et stocks"));
    assert!(text.contains("Page 1"));
    assert!(text.contains("\\311"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn importe_un_csv_apres_apercu_sans_creer_de_stock() {
    let (dir, mut conn) = open();
    let path = dir.join("articles.csv");
    fs::write(
        &path,
        "\u{feff}Nom;Référence;Type;Fournisseur;Description\nÉtanchéité;ET-9;Silicone;Selena;Joint façade\n",
    )
    .expect("fichier");
    let preview = preview_article_import(&conn, &path).expect("aperçu");
    assert!(preview.issues.is_empty(), "{:?}", preview.issues);
    assert_eq!(preview.ready[0].name, "Étanchéité");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 0);
    let created = commit_article_import(&mut conn, &path).expect("import");
    assert_eq!(created, 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM stock_movements"), 0);
    assert_eq!(
        count(
            &conn,
            "SELECT COALESCE(SUM(quantity_milli), 0) FROM stock_balances"
        ),
        0
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn annule_l_import_quand_une_ligne_est_invalide_et_preserve_le_stock() {
    let (dir, mut conn) = open();
    let existing = product(&mut conn, "Déjà là", "DEJA");
    record_entry(
        &mut conn,
        MovementDraft {
            product_id: existing,
            quantity: "5".into(),
            reason: None,
            document_reference: None,
            comment: None,
        },
    )
    .expect("entrée");
    let path = dir.join("mix.csv");
    fs::write(
        &path,
        "Nom;Référence;Type;Stock\nNouvelle;NEU-1;Silicone;\nMauvaise;NEU-2;Inconnu;4\n",
    )
    .expect("fichier");
    let preview = preview_article_import(&conn, &path).expect("aperçu");
    assert!(
        preview
            .issues
            .iter()
            .any(|issue| issue.message.contains("n'existe pas"))
    );
    assert!(commit_article_import(&mut conn, &path).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);
    assert_eq!(
        get_product_stock(&conn, existing)
            .expect("stock")
            .quantity_milli,
        5_000
    );
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM stock_movements"), 1);

    let stock_file = dir.join("stock.csv");
    fs::write(
        &stock_file,
        "Nom;Type;Stock actuel\nAutre article;Silicone;99\n",
    )
    .expect("fichier");
    let preview = preview_article_import(&conn, &stock_file).expect("aperçu stock");
    assert!(
        preview
            .issues
            .iter()
            .any(|issue| issue.message.contains("stock"))
    );
    assert!(commit_article_import(&mut conn, &stock_file).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM stock_movements"), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuse_les_references_en_double_les_fournisseurs_inconnus_et_un_fichier_mal_forme() {
    let (dir, mut conn) = open();
    product(&mut conn, "Existant", "DEJA");
    let duplicates = dir.join("doublons.csv");
    fs::write(
        &duplicates,
        "Nom;Référence;Type\nUn;AAA;Silicone\nDeux;aaa;Silicone\nTrois;DEJA;Silicone\n",
    )
    .expect("fichier");
    let preview = preview_article_import(&conn, &duplicates).expect("aperçu");
    assert!(
        preview
            .issues
            .iter()
            .any(|issue| issue.message.contains("en double"))
    );
    assert!(
        preview
            .issues
            .iter()
            .any(|issue| issue.message.contains("déjà utilisée"))
    );
    assert!(commit_article_import(&mut conn, &duplicates).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);

    let unknown = dir.join("fournisseur.csv");
    fs::write(&unknown, "Nom;Type;Fournisseur\nJoint;Silicone;Inconnu\n").expect("fichier");
    let preview = preview_article_import(&conn, &unknown).expect("aperçu");
    assert!(
        preview
            .issues
            .iter()
            .any(|issue| issue.message.contains("fournisseur"))
    );
    let (type_id, _) = ids(&conn);
    set_product_type_active(&mut conn, type_id, false).expect("désactivation");
    let inactive = dir.join("type.csv");
    fs::write(&inactive, "Nom;Type\nJoint;Silicone\n").expect("fichier");
    let preview = preview_article_import(&conn, &inactive).expect("aperçu");
    assert!(
        preview
            .issues
            .iter()
            .any(|issue| issue.message.contains("désactivé"))
    );
    let malformed = dir.join("casse.csv");
    fs::write(&malformed, "Titre;Valeur\nBonjour;monde\n").expect("fichier");
    let error = preview_article_import(&conn, &malformed).expect_err("colonne");
    assert!(error.to_string().contains("Nom"));
    let garbage = dir.join("casse.xlsx");
    fs::write(&garbage, b"pas un classeur").expect("fichier");
    assert!(preview_article_import(&conn, &garbage).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn n_execute_pas_le_contenu_importe_et_signale_une_ecriture_impossible() {
    let (dir, mut conn) = open();
    let path = dir.join("injection.csv");
    fs::write(
        &path,
        "Nom;Type\n\"Robert'); DROP TABLE products;--\";Silicone\n",
    )
    .expect("fichier");
    commit_article_import(&mut conn, &path).expect("import texte");
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM products WHERE name LIKE '%DROP TABLE%'"
        ),
        1
    );
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM product_types"), 3);
    let missing = Path::new("/tmp/gtm-dossier-absent-export/catalogue.csv");
    let error = export_catalogue(
        &conn,
        missing,
        ExportFormat::Csv,
        &catalogue_query(ActivityFilter::All),
    )
    .expect_err("dossier");
    assert!(error.to_string().contains("introuvable") || error.to_string().contains("écrit"));
    let error = export_catalogue(
        &conn,
        &dir,
        ExportFormat::Csv,
        &catalogue_query(ActivityFilter::All),
    )
    .expect_err("dossier cible");
    assert!(error.to_string().contains("fichier") || error.to_string().contains("extension"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn l_apercu_seul_n_ecrit_rien() {
    let (dir, mut conn) = open();
    let path = dir.join("attente.csv");
    fs::write(&path, "Nom;Type\nEn attente;Silicone\n").expect("fichier");
    preview_article_import(&conn, &path).expect("aperçu");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 0);
    let (_, supplier_id) = ids(&conn);
    set_supplier_active(&mut conn, supplier_id, false).expect("fournisseur");
    let blocked = dir.join("fournisseur-inactif.csv");
    fs::write(&blocked, "Nom;Type;Fournisseur\nJoint;Acrylique;Selena\n").expect("fichier");
    let preview = preview_article_import(&conn, &blocked).expect("aperçu");
    assert!(
        preview
            .issues
            .iter()
            .any(|issue| issue.message.contains("désactivé"))
    );
    assert!(commit_article_import(&mut conn, &blocked).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn l_export_de_l_historique_affiche_et_filtre_en_heure_locale() {
    let (dir, mut conn) = open();
    let id = product(&mut conn, "Mastic", "MAS");
    let stamp = "2026-06-30T22:30:00.000Z";
    conn.execute(
        "INSERT INTO stock_movements (product_id, direction, quantity_milli, created_at)
         VALUES (?1, 'in', 1000, ?2)",
        rusqlite::params![id, stamp],
    )
    .expect("mouvement");
    let (local_day, local_display): (String, String) = conn
        .query_row(
            "SELECT date(?1, 'localtime'), strftime('%d/%m/%Y %H:%M:%S', ?1, 'localtime')",
            rusqlite::params![stamp],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("heure locale");
    let path = dir.join("jour.csv");
    let outcome = export_history(
        &conn,
        &path,
        ExportFormat::Csv,
        &HistoryExportQuery {
            search: None,
            product_id: Some(id),
            direction: None,
            from_date: Some(local_day.clone()),
            to_date: Some(local_day),
        },
    )
    .expect("export");
    assert_eq!(outcome.rows, 1);
    let rows = read_csv(&fs::read(&path).expect("lu")).expect("csv");
    assert_eq!(rows[1][0], local_display);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn le_pdf_rend_la_typographie_courante_sans_point_d_interrogation() {
    let (dir, mut conn) = open();
    // Nom court : le PDF tronque les cellules d'une colonne étroite.
    let id = product(&mut conn, "l’joint – “A”", "MAS-1");
    let inventory = create_inventory(
        &mut conn,
        InventoryDraft {
            name: Some("Zone A".into()),
            comment: None,
        },
    )
    .expect("inventaire");
    add_inventory_products(&mut conn, inventory.id, &[id]).expect("ligne");
    let path = dir.join("inventaire.pdf");
    export_inventory(&conn, &path, ExportFormat::Pdf, inventory.id).expect("pdf");
    let text = String::from_utf8_lossy(&fs::read(&path).expect("lu")).into_owned();
    let drawn = text
        .lines()
        .filter(|line| line.contains(") Tj"))
        .collect::<Vec<_>>();
    assert!(
        drawn.iter().all(|line| !line.contains('?')),
        "glyphe manquant : {drawn:#?}"
    );
    // Tiret cadratin du titre, apostrophe et tiret demi-cadratin du nom (WinAnsi).
    assert!(text.contains("(Inventaire \\227 Zone A)"));
    assert!(text.contains("\\222") && text.contains("\\226"));
    // Le signe moins du périmètre devient un trait d'union.
    assert!(text.contains("compt\\351e - stock"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn une_colonne_excel_ignoree_ne_bloque_pas_l_import() {
    use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
    let (dir, mut conn) = open();
    let path = dir.join("articles.xlsx");
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    for (column, header) in ["Nom", "Type", "Référence", "Prix HT", "Ajouté le"]
        .iter()
        .enumerate()
    {
        sheet
            .write_string(0, column as u16, *header)
            .expect("en-tête");
    }
    let date = ExcelDateTime::from_ymd(2026, 1, 2).expect("date");
    let day = Format::new().set_num_format("dd/mm/yyyy");
    sheet.write_string(1, 0, "Mastic").expect("cellule");
    sheet.write_string(1, 1, "Silicone").expect("cellule");
    sheet.write_number(1, 2, 300).expect("cellule");
    sheet.write_number(1, 3, 12.5).expect("cellule");
    sheet
        .write_datetime_with_format(1, 4, &date, &day)
        .expect("cellule");
    // Une valeur décimale dans une colonne utilisée reste signalée, avec sa ligne.
    sheet.write_string(2, 0, "Joint").expect("cellule");
    sheet.write_string(2, 1, "Silicone").expect("cellule");
    sheet.write_number(2, 2, 12.5).expect("cellule");
    // Une rangée vide hormis une date dans une colonne ignorée est sautée.
    sheet
        .write_datetime_with_format(3, 4, &date, &day)
        .expect("cellule");
    workbook.save(&path).expect("xlsx");

    let preview = preview_article_import(&conn, &path).expect("aperçu");
    assert_eq!(preview.ready.len(), 1);
    assert_eq!(preview.ready[0].reference.as_deref(), Some("300"));
    assert_eq!(preview.issues.len(), 1);
    assert_eq!(preview.issues[0].line, 3);
    assert!(preview.issues[0].message.starts_with("Colonne Référence"));
    assert_eq!(preview.ignored_columns, vec!["Prix HT", "Ajouté le"]);
    assert!(commit_article_import(&mut conn, &path).is_err());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM products"), 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn les_numeros_de_ligne_suivent_le_fichier_malgre_les_lignes_vides() {
    use rust_xlsxwriter::Workbook;
    let (dir, conn) = open();
    let csv = dir.join("articles.csv");
    fs::write(
        &csv,
        "Nom;Type;Description\r\n\r\nMastic;Silicone;\"Sur\ndeux lignes\"\r\n\r\nJoint;;\r\n",
    )
    .expect("csv");
    let preview = preview_article_import(&conn, &csv).expect("aperçu csv");
    assert_eq!(preview.ready[0].line, 3);
    assert_eq!(preview.issues[0].line, 6);

    let xlsx = dir.join("articles.xlsx");
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    // Le tableau commence à la ligne 2 de la feuille, avec une ligne vide.
    sheet.write_string(1, 0, "Nom").expect("cellule");
    sheet.write_string(1, 1, "Type").expect("cellule");
    sheet.write_string(3, 0, "Mastic").expect("cellule");
    sheet.write_string(3, 1, "Silicone").expect("cellule");
    sheet.write_string(4, 0, "Joint").expect("cellule");
    workbook.save(&xlsx).expect("xlsx");
    let preview = preview_article_import(&conn, &xlsx).expect("aperçu xlsx");
    assert_eq!(preview.ready[0].line, 4);
    assert_eq!(preview.issues[0].line, 5);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn l_apercu_compare_references_et_noms_comme_l_import() {
    let (dir, mut conn) = open();
    product(&mut conn, "Existant", "ÉTANCHE-1");
    let (type_id, _) = ids(&conn);
    conn.execute(
        "UPDATE product_types SET name = 'Étanchéité' WHERE id = ?1",
        [type_id],
    )
    .expect("type accentué");
    let path = dir.join("articles.csv");
    fs::write(
        &path,
        "Nom;Type;Référence\nDoublon;étanchéité;étanche-1\nNouveau;ÉTANCHÉITÉ;é-2\nCopie;Étanchéité;É-2\n",
    )
    .expect("csv");
    let preview = preview_article_import(&conn, &path).expect("aperçu");
    let lines = preview
        .issues
        .iter()
        .map(|issue| issue.line)
        .collect::<Vec<_>>();
    assert_eq!(lines, vec![2, 4], "{:?}", preview.issues);
    assert_eq!(preview.ready.len(), 1);
    assert_eq!(preview.ready[0].line, 3);
    assert!(commit_article_import(&mut conn, &path).is_err());

    fs::write(&path, "Nom;Type;Référence\nNouveau;ÉTANCHÉITÉ;é-2\n").expect("csv");
    assert!(
        preview_article_import(&conn, &path)
            .expect("aperçu")
            .issues
            .is_empty()
    );
    assert_eq!(commit_article_import(&mut conn, &path).expect("import"), 1);
    let _ = fs::remove_dir_all(dir);
}
