use super::error::StockError;

/// Nombre de millièmes pour une unité. Trois décimales, sans binaire flottant.
pub const SCALE: i64 = 1000;

/// Quantité maximale, en millièmes : 1 000 000 000,000.
pub const MAX_MILLI: i64 = 1_000_000_000_000;

pub(crate) fn parse_quantity(raw: &str) -> Result<i64, StockError> {
    parse_scaled(raw, false)
}

/// Quantité positive ou nulle, pour un comptage physique ou un seuil.
pub(crate) fn parse_non_negative(raw: &str) -> Result<i64, StockError> {
    parse_scaled(raw, true)
}

fn parse_scaled(raw: &str, allow_zero: bool) -> Result<i64, StockError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(invalid("Indiquez une quantité."));
    }
    if trimmed.starts_with('-') {
        return Err(invalid(if allow_zero {
            "La quantité doit être positive ou nulle."
        } else {
            "La quantité doit être strictement positive."
        }));
    }
    if trimmed.starts_with('+') || trimmed.chars().any(|c| c.is_whitespace()) {
        return Err(invalid("La quantité n'est pas un nombre valide."));
    }

    let comma = trimmed.contains(',');
    let dot = trimmed.contains('.');
    if comma && dot {
        return Err(invalid("La quantité n'est pas un nombre valide."));
    }
    let normalized = if comma {
        trimmed.replace(',', ".")
    } else {
        trimmed.to_string()
    };
    if normalized.chars().filter(|c| *c == '.').count() > 1 {
        return Err(invalid("La quantité n'est pas un nombre valide."));
    }

    let (whole, frac) = match normalized.split_once('.') {
        Some((whole, frac)) => (whole, frac),
        None => (normalized.as_str(), ""),
    };
    if whole.is_empty() && frac.is_empty() {
        return Err(invalid("La quantité n'est pas un nombre valide."));
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalid("La quantité n'est pas un nombre valide."));
    }
    if frac.len() > 3 {
        return Err(invalid("La quantité dépasse 3 décimales."));
    }

    let whole_value: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| limit())?
    };
    let frac_value: i64 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<3}").parse().map_err(|_| limit())?
    };
    let milli = whole_value
        .checked_mul(SCALE)
        .and_then(|value| value.checked_add(frac_value))
        .ok_or_else(limit)?;
    if milli > MAX_MILLI {
        return Err(limit());
    }
    if milli == 0 && !allow_zero {
        return Err(invalid("La quantité doit être strictement positive."));
    }
    Ok(milli)
}

pub fn format_quantity(milli: i64) -> String {
    let negative = milli < 0;
    let milli = milli.unsigned_abs();
    let whole = milli / SCALE as u64;
    let frac = milli % SCALE as u64;
    let body = if frac == 0 {
        whole.to_string()
    } else {
        let mut digits = format!("{frac:03}");
        while digits.ends_with('0') {
            digits.pop();
        }
        format!("{whole}.{digits}")
    };
    if negative { format!("-{body}") } else { body }
}

fn invalid(message: &str) -> StockError {
    StockError::Invalid(message.to_string())
}

fn limit() -> StockError {
    StockError::Invalid("La quantité dépasse la limite autorisée.".into())
}
