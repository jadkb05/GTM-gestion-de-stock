use super::error::CatalogueError;

pub(crate) const NAME_MAX: usize = 120;
pub(crate) const REFERENCE_MAX: usize = 64;
pub(crate) const DESCRIPTION_MAX: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedProduct {
    pub name: String,
    pub reference: Option<String>,
    pub type_id: i64,
    pub supplier_id: Option<i64>,
    pub description: Option<String>,
}

pub(crate) fn normalize_label(value: &str, empty_message: &str) -> Result<String, CatalogueError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CatalogueError::Invalid(empty_message.to_string()));
    }
    if trimmed.chars().count() > NAME_MAX {
        return Err(CatalogueError::Invalid(format!(
            "Le nom dépasse {NAME_MAX} caractères."
        )));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_product(
    draft: &super::ProductDraft,
) -> Result<NormalizedProduct, CatalogueError> {
    let name = normalize_label(&draft.name, "Indiquez le nom du produit.")?;
    let reference = match draft.reference.as_deref() {
        None => None,
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.chars().count() > REFERENCE_MAX {
                return Err(CatalogueError::Invalid(format!(
                    "La référence dépasse {REFERENCE_MAX} caractères."
                )));
            } else {
                Some(trimmed.to_string())
            }
        }
    };
    let description = match draft.description.as_deref() {
        None => None,
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.chars().count() > DESCRIPTION_MAX {
                return Err(CatalogueError::Invalid(
                    "La description dépasse 2 000 caractères.".into(),
                ));
            } else {
                Some(trimmed.to_string())
            }
        }
    };
    if draft.type_id < 1 {
        return Err(CatalogueError::Invalid(
            "Choisissez un type de produit.".into(),
        ));
    }
    if matches!(draft.supplier_id, Some(id) if id < 1) {
        return Err(CatalogueError::Invalid(
            "Ce fournisseur n'existe pas.".into(),
        ));
    }

    Ok(NormalizedProduct {
        name,
        reference,
        type_id: draft.type_id,
        supplier_id: draft.supplier_id,
        description,
    })
}

pub(crate) fn same_name(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}
