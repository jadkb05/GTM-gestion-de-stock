use super::db;
use gtm_db::{ActivityFilter, Product, ProductDraft, ProductQuery, ProductType, Supplier};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDto {
    id: i64,
    name: String,
    reference: Option<String>,
    type_id: i64,
    type_name: String,
    supplier_id: Option<i64>,
    supplier_name: Option<String>,
    description: Option<String>,
    is_active: bool,
    created_at: String,
    updated_at: String,
}

impl From<Product> for ProductDto {
    fn from(product: Product) -> Self {
        Self {
            id: product.id,
            name: product.name,
            reference: product.reference,
            type_id: product.type_id,
            type_name: product.type_name,
            supplier_id: product.supplier_id,
            supplier_name: product.supplier_name,
            description: product.description,
            is_active: product.is_active,
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedDto {
    id: i64,
    name: String,
    is_active: bool,
    created_at: String,
    updated_at: String,
}

impl From<ProductType> for NamedDto {
    fn from(value: ProductType) -> Self {
        Self {
            id: value.id,
            name: value.name,
            is_active: value.is_active,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Supplier> for NamedDto {
    fn from(value: Supplier) -> Self {
        Self {
            id: value.id,
            name: value.name,
            is_active: value.is_active,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductQueryDto {
    search: Option<String>,
    type_id: Option<i64>,
    supplier_id: Option<i64>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductInput {
    name: String,
    reference: Option<String>,
    type_id: i64,
    supplier_id: Option<i64>,
    description: Option<String>,
}

impl From<ProductInput> for ProductDraft {
    fn from(input: ProductInput) -> Self {
        Self {
            name: input.name,
            reference: input.reference,
            type_id: input.type_id,
            supplier_id: input.supplier_id,
            description: input.description,
        }
    }
}

fn status(value: Option<String>) -> Result<ActivityFilter, String> {
    match value.as_deref().map(str::trim) {
        None | Some("") | Some("active") => Ok(ActivityFilter::Active),
        Some("inactive") => Ok(ActivityFilter::Inactive),
        Some("all") => Ok(ActivityFilter::All),
        _ => Err("Le filtre de statut est invalide.".to_string()),
    }
}

pub(crate) fn to_query(input: ProductQueryDto) -> Result<ProductQuery, String> {
    Ok(ProductQuery {
        search: input.search,
        type_id: input.type_id,
        supplier_id: input.supplier_id,
        status: status(input.status)?,
    })
}

#[tauri::command]
pub fn list_products(app: AppHandle, query: ProductQueryDto) -> Result<Vec<ProductDto>, String> {
    let query = to_query(query)?;
    let conn = db::open(&app)?;
    gtm_db::list_products(&conn, &query)
        .map(|rows| rows.into_iter().map(ProductDto::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_product(app: AppHandle, id: i64) -> Result<ProductDto, String> {
    let conn = db::open(&app)?;
    gtm_db::get_product(&conn, id)
        .map(ProductDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_product(app: AppHandle, product: ProductInput) -> Result<ProductDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::create_product(&mut conn, product.into())
        .map(ProductDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_product(
    app: AppHandle,
    id: i64,
    product: ProductInput,
) -> Result<ProductDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::update_product(&mut conn, id, product.into())
        .map(ProductDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_product_active(app: AppHandle, id: i64, active: bool) -> Result<ProductDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::set_product_active(&mut conn, id, active)
        .map(ProductDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_product_types(app: AppHandle, include_inactive: bool) -> Result<Vec<NamedDto>, String> {
    let conn = db::open(&app)?;
    gtm_db::list_product_types(&conn, include_inactive)
        .map(|rows| rows.into_iter().map(NamedDto::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_product_type(app: AppHandle, name: String) -> Result<NamedDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::create_product_type(&mut conn, &name)
        .map(NamedDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_product_type(app: AppHandle, id: i64, name: String) -> Result<NamedDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::update_product_type(&mut conn, id, &name)
        .map(NamedDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_product_type_active(app: AppHandle, id: i64, active: bool) -> Result<NamedDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::set_product_type_active(&mut conn, id, active)
        .map(NamedDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_suppliers(app: AppHandle, include_inactive: bool) -> Result<Vec<NamedDto>, String> {
    let conn = db::open(&app)?;
    gtm_db::list_suppliers(&conn, include_inactive)
        .map(|rows| rows.into_iter().map(NamedDto::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_supplier(app: AppHandle, name: String) -> Result<NamedDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::create_supplier(&mut conn, &name)
        .map(NamedDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_supplier(app: AppHandle, id: i64, name: String) -> Result<NamedDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::update_supplier(&mut conn, id, &name)
        .map(NamedDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_supplier_active(app: AppHandle, id: i64, active: bool) -> Result<NamedDto, String> {
    let mut conn = db::open(&app)?;
    gtm_db::set_supplier_active(&mut conn, id, active)
        .map(NamedDto::from)
        .map_err(|error| error.to_string())
}
