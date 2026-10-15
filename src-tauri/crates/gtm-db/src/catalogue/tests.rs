use super::*;
use crate::connect;
use rusqlite::params;
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
    let path = std::env::temp_dir().join(format!("gtm-catalogue-{nanos}-{id}"));
    fs::create_dir_all(&path).expect("répertoire");
    path
}

fn open() -> (PathBuf, rusqlite::Connection) {
    let dir = temp_dir();
    let conn = connect(&dir).expect("connexion");
    (dir, conn)
}

fn type_id(conn: &rusqlite::Connection, name: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM product_types WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )
    .expect("type")
}

fn supplier_id(conn: &rusqlite::Connection, name: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM suppliers WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )
    .expect("fournisseur")
}

fn draft(
    name: &str,
    reference: Option<&str>,
    type_id: i64,
    supplier_id: Option<i64>,
) -> ProductDraft {
    ProductDraft {
        name: name.to_string(),
        reference: reference.map(str::to_string),
        type_id,
        supplier_id,
        description: None,
    }
}

#[test]
fn cree_un_produit_valide_et_le_retrouve_apres_reouverture() {
    let dir = temp_dir();
    let mut conn = connect(&dir).expect("connexion");
    let silicone = type_id(&conn, "Silicone");
    let selena = supplier_id(&conn, "Selena");
    let created = create_product(
        &mut conn,
        ProductDraft {
            name: "  Mastic façade  ".into(),
            reference: Some(" sil-01 ".into()),
            type_id: silicone,
            supplier_id: Some(selena),
            description: Some("  Joint de façade  ".into()),
        },
    )
    .expect("création");
    assert_eq!(created.name, "Mastic façade");
    assert_eq!(created.reference.as_deref(), Some("sil-01"));
    assert_eq!(created.type_name, "Silicone");
    assert_eq!(created.supplier_name.as_deref(), Some("Selena"));
    assert_eq!(created.description.as_deref(), Some("Joint de façade"));
    assert!(created.is_active);
    drop(conn);

    let conn = connect(&dir).expect("réouverture");
    let loaded = get_product(&conn, created.id).expect("lecture");
    assert_eq!(loaded, created);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejette_un_nom_vide() {
    let (dir, mut conn) = open();
    let autre = type_id(&conn, "Autre");
    let error = create_product(&mut conn, draft("   ", None, autre, None)).expect_err("nom vide");
    assert_eq!(
        error,
        CatalogueError::Invalid("Indiquez le nom du produit.".into())
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejette_un_type_inexistant_ou_desactive() {
    let (dir, mut conn) = open();
    let missing = create_product(&mut conn, draft("Produit", None, 999, None)).expect_err("type");
    assert_eq!(
        missing,
        CatalogueError::Invalid("Ce type de produit n'existe pas.".into())
    );
    assert!(!missing.to_string().to_lowercase().contains("sqlite"));

    let silicone = type_id(&conn, "Silicone");
    set_product_type_active(&mut conn, silicone, false).expect("désactivation");
    let inactive =
        create_product(&mut conn, draft("Produit", None, silicone, None)).expect_err("inactif");
    assert_eq!(
        inactive,
        CatalogueError::Invalid("Ce type de produit est désactivé.".into())
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejette_un_fournisseur_desactive() {
    let (dir, mut conn) = open();
    let tremco = supplier_id(&conn, "Tremco");
    set_supplier_active(&mut conn, tremco, false).expect("désactivation");
    let acrylique = type_id(&conn, "Acrylique");
    let error = create_product(&mut conn, draft("Produit", None, acrylique, Some(tremco)))
        .expect_err("fournisseur");
    assert_eq!(
        error,
        CatalogueError::Invalid("Ce fournisseur est désactivé.".into())
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn modifie_un_produit_et_detecte_une_reference_dupliquee() {
    let (dir, mut conn) = open();
    let acrylique = type_id(&conn, "Acrylique");
    let first = create_product(&mut conn, draft("Premier", Some("REF-A"), acrylique, None))
        .expect("premier");
    let second =
        create_product(&mut conn, draft("Second", Some("REF-B"), acrylique, None)).expect("second");

    let duplicate = update_product(
        &mut conn,
        second.id,
        draft("Second", Some("ref-a"), acrylique, None),
    )
    .expect_err("doublon");
    assert_eq!(
        duplicate,
        CatalogueError::Conflict("Cette référence est déjà utilisée.".into())
    );

    let legallais = supplier_id(&conn, "Legallais");
    let updated = update_product(
        &mut conn,
        first.id,
        ProductDraft {
            name: "Premier modifié".into(),
            reference: Some("REF-A".into()),
            type_id: acrylique,
            supplier_id: Some(legallais),
            description: Some("Nouvelle description".into()),
        },
    )
    .expect("modification");
    assert_eq!(updated.name, "Premier modifié");
    assert_eq!(updated.supplier_name.as_deref(), Some("Legallais"));
    assert_eq!(updated.description.as_deref(), Some("Nouvelle description"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn desactive_un_produit_sans_le_supprimer() {
    let (dir, mut conn) = open();
    let autre = type_id(&conn, "Autre");
    let created =
        create_product(&mut conn, draft("À ranger", None, autre, None)).expect("création");
    let disabled = set_product_active(&mut conn, created.id, false).expect("désactivation");
    assert!(!disabled.is_active);
    let loaded = get_product(&conn, created.id).expect("toujours présent");
    assert_eq!(loaded.name, "À ranger");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn conserve_les_produits_quand_un_type_ou_un_fournisseur_est_desactive() {
    let (dir, mut conn) = open();
    let silicone = type_id(&conn, "Silicone");
    let selena = supplier_id(&conn, "Selena");
    let product = create_product(
        &mut conn,
        draft("Cartouche", Some("CAR-1"), silicone, Some(selena)),
    )
    .expect("création");

    let disabled_type = set_product_type_active(&mut conn, silicone, false).expect("type");
    assert!(!disabled_type.is_active);
    let disabled_supplier = set_supplier_active(&mut conn, selena, false).expect("fournisseur");
    assert!(!disabled_supplier.is_active);

    let kept = get_product(&conn, product.id).expect("produit historique");
    assert_eq!(kept.type_id, silicone);
    assert_eq!(kept.supplier_id, Some(selena));
    assert!(kept.is_active);

    let kept_assignment = update_product(
        &mut conn,
        product.id,
        draft("Cartouche", Some("CAR-1"), silicone, Some(selena)),
    )
    .expect("conservation de l'affectation");
    assert_eq!(kept_assignment.type_id, silicone);

    let rejected = create_product(&mut conn, draft("Nouveau", None, silicone, None))
        .expect_err("nouveau type inactif");
    assert!(matches!(rejected, CatalogueError::Invalid(_)));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn ajoute_et_modifie_les_types_et_les_fournisseurs() {
    let (dir, mut conn) = open();
    let created_type = create_product_type(&mut conn, "  Mousse  ").expect("type");
    assert_eq!(created_type.name, "Mousse");
    let renamed = update_product_type(&mut conn, created_type.id, "Mousse PU").expect("renommage");
    assert_eq!(renamed.name, "Mousse PU");
    let duplicate = create_product_type(&mut conn, "silicone").expect_err("doublon type");
    assert_eq!(
        duplicate,
        CatalogueError::Conflict("Un type portant ce nom existe déjà.".into())
    );

    let created_supplier = create_supplier(&mut conn, "Sika").expect("fournisseur");
    let renamed_supplier =
        update_supplier(&mut conn, created_supplier.id, "Sika France").expect("renommage");
    assert_eq!(renamed_supplier.name, "Sika France");
    let duplicate_supplier = create_supplier(&mut conn, " TREMCO ").expect_err("doublon");
    assert_eq!(
        duplicate_supplier,
        CatalogueError::Conflict("Un fournisseur portant ce nom existe déjà.".into())
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn la_contrainte_sql_empeche_une_reference_en_double() {
    let (dir, conn) = open();
    let type_id = type_id(&conn, "Autre");
    conn.execute(
        "INSERT INTO products (name, reference, type_id, is_active, created_at, updated_at)
         VALUES ('Un', 'REF', ?1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        params![type_id],
    )
    .expect("première insertion");
    let error = conn
        .execute(
            "INSERT INTO products (name, reference, type_id, is_active, created_at, updated_at)
             VALUES ('Deux', 'ref', ?1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            params![type_id],
        )
        .expect_err("contrainte");
    assert!(crate::catalogue::error::is_constraint(&error));

    let missing_type = conn.execute(
        "INSERT INTO products (name, reference, type_id, is_active, created_at, updated_at)
         VALUES ('Trois', 'AUTRE', 999, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    );
    assert!(missing_type.is_err());
    assert!(crate::catalogue::error::is_constraint(
        &missing_type.expect_err("clé étrangère")
    ));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn filtre_les_produits_par_nom_reference_type_et_statut() {
    let (dir, mut conn) = open();
    let silicone = type_id(&conn, "Silicone");
    let autre = type_id(&conn, "Autre");
    create_product(&mut conn, draft("Mastic", Some("MZ-1"), silicone, None)).expect("un");
    let hidden =
        create_product(&mut conn, draft("Primaire", Some("PR-9"), autre, None)).expect("deux");
    set_product_active(&mut conn, hidden.id, false).expect("désactivation");

    let by_name = list_products(
        &conn,
        &ProductQuery {
            search: Some("mast".into()),
            type_id: None,
            supplier_id: None,
            status: ActivityFilter::All,
        },
    )
    .expect("recherche");
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].reference.as_deref(), Some("MZ-1"));

    let active = list_products(
        &conn,
        &ProductQuery {
            search: None,
            type_id: Some(silicone),
            supplier_id: None,
            status: ActivityFilter::Active,
        },
    )
    .expect("filtre");
    assert_eq!(active.len(), 1);

    let inactive = list_products(
        &conn,
        &ProductQuery {
            search: Some("pr-9".into()),
            type_id: None,
            supplier_id: None,
            status: ActivityFilter::Inactive,
        },
    )
    .expect("inactifs");
    assert_eq!(inactive.len(), 1);
    let _ = fs::remove_dir_all(dir);
}
