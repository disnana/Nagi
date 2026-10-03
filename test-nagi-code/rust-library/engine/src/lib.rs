//! Reusable pricing calculations. Amounts are integer cents.
//! `volume-discount` takes 10% off orders of at least ten items, rounded down.
//! The default `service-fee` feature adds 100 cents to every valid order.

#[derive(Debug, PartialEq, Eq)]
pub struct Quote {
    pub product: String,
    pub subtotal_cents: i64,
    pub discount_cents: i64,
    pub fee_cents: i64,
    pub total_cents: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuoteError {
    EmptyProduct,
    InvalidAmount,
    Overflow,
}

impl std::fmt::Display for QuoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::EmptyProduct => "product name must not be blank",
            Self::InvalidAmount => "unit price must be nonnegative and quantity must be positive",
            Self::Overflow => "order total exceeds the supported range",
        })
    }
}

impl std::error::Error for QuoteError {}

/// Borrow the product name, validate the order, and return an owned quote.
/// This crate has no Nagi dependency and can be used by another Rust program.
pub fn quote(product: &str, unit_price_cents: i64, quantity: i64) -> Result<Quote, QuoteError> {
    if product.trim().is_empty() {
        return Err(QuoteError::EmptyProduct);
    }
    if unit_price_cents < 0 || quantity <= 0 {
        return Err(QuoteError::InvalidAmount);
    }
    let subtotal_cents = unit_price_cents
        .checked_mul(quantity)
        .ok_or(QuoteError::Overflow)?;
    let discount_cents = if cfg!(feature = "volume-discount") && quantity >= 10 {
        subtotal_cents / 10
    } else {
        0
    };
    let fee_cents = if cfg!(feature = "service-fee") {
        100
    } else {
        0
    };
    let total_cents = (subtotal_cents - discount_cents)
        .checked_add(fee_cents)
        .ok_or(QuoteError::Overflow)?;
    Ok(Quote {
        product: product.to_owned(),
        subtotal_cents,
        discount_cents,
        fee_cents,
        total_cents,
    })
}
