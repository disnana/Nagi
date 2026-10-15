//! One-checkpoint observation only; no clock, credential or authority injection.
use super::Inner;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    NotStarted,
    EntryLease,
    Begin,
    Check,
    InitialSample,
    Binding,
    Cleanup,
    ReadTokens,
    FreshSample,
    FreshBinding,
    FreshLease,
    Matched,
    Project,
    Maintain,
    Finish,
    PostSample,
    Restore,
    PostSampleResult,
    FinalSample,
    FinalBinding,
    FinalLease,
    FinalExpiry,
    Done,
}
#[derive(Clone, Copy)]
pub(super) struct Observation {
    pub(super) phase: Phase,
    pub(super) refusal: Option<Phase>,
    pub(super) matched: Option<bool>,
    pub(super) original_expiry: Option<bool>,
    pub(super) sample_expired: Option<bool>,
    pub(super) lease_valid: Option<bool>,
    pub(super) actual_expired: Option<bool>,
}
pub(super) struct Probe {
    original_expiry: i64,
    observation: Mutex<Observation>,
}
impl Probe {
    pub(super) fn new(original_expiry: i64) -> Self {
        Self {
            original_expiry,
            observation: Mutex::new(Observation {
                phase: Phase::NotStarted,
                refusal: None,
                matched: None,
                original_expiry: None,
                sample_expired: None,
                lease_valid: None,
                actual_expired: None,
            }),
        }
    }
    pub(super) fn failure(&self) -> Observation {
        let mut observed = *self.observation.lock().unwrap();
        if matches!(
            observed.phase,
            Phase::EntryLease | Phase::FreshLease | Phase::FinalLease
        ) || matches!(
            observed.refusal,
            Some(Phase::EntryLease | Phase::FreshLease | Phase::FinalLease)
        ) {
            observed.lease_valid = Some(false);
        }
        observed
    }
}
impl Inner {
    pub(super) fn checkpoint_refusal(&self, refused: bool) {
        if let Some(probe) = &self.checkpoint_probe {
            let mut observed = probe.observation.lock().unwrap();
            if refused {
                observed.refusal = Some(observed.phase);
            }
        }
    }
    pub(super) fn checkpoint_phase(&self, phase: Phase) {
        if let Some(probe) = &self.checkpoint_probe {
            let mut observed = probe.observation.lock().unwrap();
            if phase == Phase::EntryLease {
                assert!(
                    observed.phase == Phase::NotStarted,
                    "same_queue_probe_one_checkpoint"
                );
            }
            if matches!(phase, Phase::Begin | Phase::Matched | Phase::FinalExpiry) {
                observed.lease_valid = Some(true);
            }
            observed.phase = phase;
        }
    }
    pub(super) fn checkpoint_sample(&self, ceil_ms: i64) {
        if let Some(probe) = &self.checkpoint_probe {
            probe.observation.lock().unwrap().sample_expired =
                Some(ceil_ms >= probe.original_expiry);
        }
    }
    pub(super) fn checkpoint_sample_result(
        &self,
        sample: &Result<super::session::PreloginClock, super::Failure>,
    ) {
        if let Ok(sample) = sample {
            self.checkpoint_sample(sample.ceil_ms);
        }
    }
    pub(super) fn checkpoint_match(&self, matched: bool, expiry: i64) {
        if let Some(probe) = &self.checkpoint_probe {
            let mut observed = probe.observation.lock().unwrap();
            observed.matched = Some(matched);
            observed.original_expiry = matched.then_some(expiry == probe.original_expiry);
        }
    }
    pub(super) fn checkpoint_actual(&self, expired: Option<bool>) {
        if let Some(probe) = &self.checkpoint_probe {
            probe.observation.lock().unwrap().actual_expired = expired;
        }
    }
}
