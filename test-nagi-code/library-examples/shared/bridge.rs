//! Adapts the reusable Rust core to the class declared in foundation.nagi.
#[path = "pricing.rs"]
mod pricing;

pub fn foundation_rust_quote(
    label: &str,
    unit_cents: i64,
    quantity: i64,
    discount_bps: i64,
) -> Result<crate::FoundationQuote, nagi_runtime::Error> {
    if label.is_empty() || label.len() > 80 {
        return Err(nagi_runtime::Error::invalid(
            "label must contain 1 to 80 UTF-8 bytes",
        ));
    }
    let totals = pricing::calculate(unit_cents, quantity, discount_bps)
        .map_err(nagi_runtime::Error::invalid)?;
    Ok(crate::FoundationQuote {
        label: label.to_owned(),
        unit_cents,
        quantity,
        discount_bps,
        subtotal_cents: totals.subtotal_cents,
        discount_cents: totals.discount_cents,
        total_cents: totals.total_cents,
    })
}
