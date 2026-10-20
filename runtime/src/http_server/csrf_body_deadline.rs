//! Private absolute deadline selection, independent of preparation scheduling.
use std::time::{Duration, Instant};

pub(super) struct BodyDeadline {
    pub(super) until: Instant,
    pub(super) body_first: bool,
}

pub(super) fn select_body_deadline(
    common: Instant,
    body_started: Instant,
    body_budget: Duration,
) -> Option<BodyDeadline> {
    let body_until = body_started.checked_add(body_budget)?;
    Some(BodyDeadline {
        until: body_until.min(common),
        // The original common deadline wins an exact tie.
        body_first: body_until < common,
    })
}
