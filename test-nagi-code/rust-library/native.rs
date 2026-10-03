// The manifest dependency alias `pricing` selects the independent Rust crate.
// Only this adapter knows both the crate's types and the generated Nagi class.
pub fn quote(
    product: &str,
    unit_price_cents: i64,
    quantity: i64,
) -> Result<super::Quote, nagi_runtime::Error> {
    let result = pricing::quote(product, unit_price_cents, quantity)
        .map_err(|error| nagi_runtime::Error::invalid(error.to_string()))?;
    Ok(super::Quote {
        product: result.product,
        subtotal_cents: result.subtotal_cents,
        discount_cents: result.discount_cents,
        fee_cents: result.fee_cents,
        total_cents: result.total_cents,
    })
}
