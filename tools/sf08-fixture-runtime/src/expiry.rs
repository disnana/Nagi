//! Conservative private observation, never a clock or token authority.
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct ExpiryWitness {
    greatest_wall: Option<i64>,
    original: Option<(i64, Instant)>,
    invalid: bool,
}

impl ExpiryWitness {
    pub(crate) fn observe(
        &mut self,
        raw_wall: i64,
        persisted: i64,
        first_ready_expiry: Option<i64>,
        sampled_at: Instant,
        observed_at: Instant,
    ) -> bool {
        // Match PreloginContext::track_sample: retain the greatest bound even
        // when this raw sample is rejected. Never replace a raw sample by max().
        let bound = self.greatest_wall.unwrap_or(0).max(persisted);
        self.greatest_wall = Some(bound.max(raw_wall));
        if raw_wall < bound || persisted < 0 || observed_at < sampled_at {
            self.invalid = true;
        }
        if self.invalid {
            return false;
        }
        if self.original.is_none() {
            let Some(expiry) = first_ready_expiry else {
                return false;
            };
            let Some(remaining) = expiry.checked_sub(raw_wall).filter(|value| *value > 0) else {
                self.invalid = true;
                return false;
            };
            let Some(deadline) = sampled_at.checked_add(Duration::from_millis(remaining as u64))
            else {
                self.invalid = true;
                return false;
            };
            self.original = Some((expiry, deadline));
        }
        let (expiry, deadline) = self.original.unwrap();
        let unexpired = raw_wall < expiry && observed_at < deadline;
        // No later clock recovery, row, or bootstrap can resurrect this witness.
        self.invalid |= !unexpired;
        unexpired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    #[test]
    fn empty_initial_snapshot_and_valid_first_expiry() {
        let start = Instant::now();
        let mut witness = ExpiryWitness::default();
        assert!(!witness.observe(100, 100, None, start, start));
        assert!(witness.observe(101, 100, Some(200), at(start, 1), at(start, 1)));
        assert_eq!(witness.original, Some((200, at(start, 100))));
    }

    #[test]
    fn wall_and_monotonic_expiry_boundaries_are_exclusive() {
        let start = Instant::now();
        let mut wall = ExpiryWitness::default();
        assert!(wall.observe(100, 100, Some(200), start, start));
        assert!(wall.observe(199, 100, None, at(start, 99), at(start, 99)));
        assert!(!wall.observe(200, 100, None, at(start, 99), at(start, 99)));
        let mut mono = ExpiryWitness::default();
        assert!(mono.observe(100, 100, Some(200), start, start));
        assert!(!mono.observe(199, 100, None, at(start, 100), at(start, 100)));
    }

    #[test]
    fn rollback_below_metadata_before_first_ready_row_latches_false() {
        let start = Instant::now();
        let mut witness = ExpiryWitness::default();
        assert!(!witness.observe(99, 100, Some(200), start, start));
        assert!(witness.original.is_none());
        assert!(!witness.observe(101, 100, Some(200), at(start, 1), at(start, 1)));
    }

    #[test]
    fn rollback_in_empty_snapshot_cannot_be_hidden_by_later_first_row() {
        let start = Instant::now();
        let mut witness = ExpiryWitness::default();
        assert!(!witness.observe(100, 100, None, start, start));
        assert!(!witness.observe(99, 0, None, start, start));
        assert!(!witness.observe(101, 0, Some(200), at(start, 1), at(start, 1)));
    }

    #[test]
    fn past_maximum_survives_lower_metadata_and_recovery() {
        let start = Instant::now();
        let mut witness = ExpiryWitness::default();
        assert!(witness.observe(120, 120, Some(300), start, start));
        assert!(!witness.observe(119, 0, None, at(start, 1), at(start, 1)));
        assert!(!witness.observe(121, 0, Some(900), at(start, 2), at(start, 2)));
        assert_eq!(witness.greatest_wall, Some(121));
    }

    #[test]
    fn rejected_persisted_floor_is_retained_and_never_becomes_a_new_bound() {
        let start = Instant::now();
        let mut witness = ExpiryWitness::default();
        assert!(!witness.observe(150, 200, None, start, start));
        assert_eq!(witness.greatest_wall, Some(200));
        assert!(!witness.observe(190, 0, Some(300), start, start));
        assert!(!witness.observe(201, 0, Some(300), start, start));
    }

    #[test]
    fn original_expiry_and_first_monotonic_bound_cannot_extend() {
        let start = Instant::now();
        let mut witness = ExpiryWitness::default();
        assert!(witness.observe(100, 100, Some(200), start, at(start, 80)));
        let original = witness.original;
        assert!(witness.observe(150, 100, Some(1_000), at(start, 90), at(start, 90)));
        assert_eq!(witness.original, original);
        assert_eq!(original, Some((200, at(start, 100))));
        assert!(!witness.observe(150, 100, Some(1_000), at(start, 100), at(start, 100)));
    }

    #[test]
    fn initially_expired_row_and_invalid_samples_stay_false() {
        let start = Instant::now();
        let mut expired = ExpiryWitness::default();
        assert!(!expired.observe(200, 200, Some(200), start, start));
        assert!(!expired.observe(201, 200, Some(900), start, start));
        let mut invalid_mono = ExpiryWitness::default();
        assert!(!invalid_mono.observe(100, 100, Some(200), at(start, 1), start));
        assert!(!invalid_mono.observe(101, 100, Some(200), at(start, 1), at(start, 1)));
        for (raw, persisted) in [(-1, 0), (100, -1)] {
            let mut witness = ExpiryWitness::default();
            assert!(!witness.observe(raw, persisted, Some(200), start, start));
            assert!(!witness.observe(101, 0, Some(200), start, start));
        }
    }
}
