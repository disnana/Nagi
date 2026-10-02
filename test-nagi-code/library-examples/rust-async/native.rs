use std::time::Duration;

pub async fn delayed_double(value: i64, milliseconds: i64) -> Result<i64, nagi_runtime::Error> {
    if !(0..=1000).contains(&milliseconds) {
        return Err(nagi_runtime::Error::invalid(
            "milliseconds must be between 0 and 1000",
        ));
    }
    let doubled = value
        .checked_mul(2)
        .ok_or_else(|| nagi_runtime::Error::invalid("doubling exceeds the i64 range"))?;

    // Nagi already runs this future on its Tokio runtime. Do not create another.
    tokio::time::sleep(Duration::from_millis(milliseconds as u64)).await;
    Ok(doubled)
}
