//! Opt-in CI measurement, compiled out of ordinary builds. No query payloads.
use std::cell::Cell;
use std::io::Write;
use std::time::Instant;

thread_local! {
    static TRACE: Cell<Option<(Instant, u8)>> = const { Cell::new(None) };
}

pub(crate) struct Guard(bool);

pub(crate) fn begin(args: &[String]) -> Guard {
    let enabled = args.first().is_some_and(|arg| arg == "symbols")
        && args.iter().any(|arg| arg == "--editor-input")
        && std::env::var_os("NAGI_EDITOR_STAGE_DIAG").is_some_and(|value| value == "1");
    let active = enabled
        && TRACE.with(|trace| {
            if trace.get().is_some() {
                return false;
            }
            trace.set(Some((Instant::now(), 0)));
            true
        });
    if active {
        mark(0);
    }
    Guard(active)
}

pub(crate) fn mark(stage: u8) {
    TRACE.with(|trace| {
        if let Some((start, next)) = trace.get() {
            if stage != next || stage > 10 {
                // Do not emit misleading suffixes after a non-prefix transition.
                trace.set(None);
                return;
            }
            let elapsed = start.elapsed().as_micros().min(60_000_000);
            let _ = writeln!(
                std::io::stderr().lock(),
                "NAGI_EDITOR_STAGE {stage} {elapsed}"
            );
            trace.set(Some((start, next + 1)));
        }
    });
}

impl Drop for Guard {
    fn drop(&mut self) {
        if self.0 {
            TRACE.with(|trace| trace.set(None));
        }
    }
}
