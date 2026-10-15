//! Closed observation of the existing real provider; never substitutes a clock.
use super::{ClockSample, Store};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub(crate) struct Sample {
    pub(crate) monotonic: Instant,
    pub(crate) floor_ms: i64,
    pub(crate) ceil_ms: i64,
}
enum State {
    Disabled,
    Armed {
        calls: usize,
        sample: Option<Sample>,
    },
    Taken,
}
pub(crate) struct Probe(Mutex<State>);
impl Probe {
    pub(crate) fn arm(&self) {
        let mut state = self.0.lock().unwrap();
        assert!(
            matches!(*state, State::Disabled),
            "projection_probe_arm_once"
        );
        *state = State::Armed {
            calls: 0,
            sample: None,
        };
    }
    pub(crate) fn take(&self) -> Sample {
        let state = std::mem::replace(&mut *self.0.lock().unwrap(), State::Taken);
        match state {
            State::Armed {
                calls: 1,
                sample: Some(sample),
            } => sample,
            _ => panic!("projection_probe_requires_unique_lookup_sample"),
        }
    }
    fn record(&self, c: &ClockSample) {
        if let State::Armed { calls, sample } = &mut *self.0.lock().unwrap() {
            *calls = calls.checked_add(1).expect("projection_probe_count");
            if sample.is_none() {
                *sample = Some(Sample {
                    monotonic: c.monotonic,
                    floor_ms: c.floor_ms,
                    ceil_ms: c.ceil_ms,
                });
            }
        }
    }
}
// Install before any Store clone. No callback/setter/sample constructor escapes.
pub(crate) fn install(store: &mut Store) -> Arc<Probe> {
    let foundation =
        Arc::get_mut(&mut store.foundation).expect("projection_probe_requires_unshared_store");
    let original = Arc::clone(&foundation.clock);
    let probe = Arc::new(Probe(Mutex::new(State::Disabled)));
    let observed = Arc::clone(&probe);
    foundation.clock = Arc::new(move || {
        let sample = original()?;
        observed.record(&sample);
        Ok(sample)
    });
    probe
}
