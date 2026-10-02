//! Ordinary reusable Rust arithmetic; no Nagi or third-party types appear here.
#[derive(Debug, PartialEq, Eq)]
pub struct Totals {
    pub subtotal_cents: i64,
    pub discount_cents: i64,
    pub total_cents: i64,
}

pub fn calculate(
    unit_cents: i64,
    quantity: i64,
    discount_bps: i64,
) -> Result<Totals, &'static str> {
    if !(0..=100_000_000).contains(&unit_cents) {
        return Err("unit_cents must be between 0 and 100000000");
    }
    if !(1..=10_000).contains(&quantity) {
        return Err("quantity must be between 1 and 10000");
    }
    if !(0..=10_000).contains(&discount_bps) {
        return Err("discount_bps must be between 0 and 10000");
    }
    // Bounds keep both products within i64. Integer division rounds down.
    let subtotal_cents = unit_cents * quantity;
    let discount_cents = subtotal_cents * discount_bps / 10_000;
    Ok(Totals {
        subtotal_cents,
        discount_cents,
        total_cents: subtotal_cents - discount_cents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discount_rounds_down_in_minor_units() {
        assert_eq!(
            calculate(999, 3, 1250),
            Ok(Totals {
                subtotal_cents: 2997,
                discount_cents: 374,
                total_cents: 2623,
            })
        );
    }

    #[test]
    fn largest_accepted_inputs_and_full_discount_fit() {
        assert_eq!(
            calculate(100_000_000, 10_000, 10_000),
            Ok(Totals {
                subtotal_cents: 1_000_000_000_000,
                discount_cents: 1_000_000_000_000,
                total_cents: 0,
            })
        );
    }

    #[test]
    fn rejects_values_outside_the_contract() {
        for input in [
            (-1, 1, 0),
            (100_000_001, 1, 0),
            (1, 0, 0),
            (1, 10_001, 0),
            (1, 1, -1),
            (1, 1, 10_001),
        ] {
            assert!(calculate(input.0, input.1, input.2).is_err());
        }
    }
}
