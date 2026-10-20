//! Private post-spawn injection, not a stock Tokio H0 reproduction.
//! The only real handle lives in fixture escrow for actual abort/join teardown.
//! SUT never receives that handle, or uses its teardown join as acknowledgement.
use super::*;

#[derive(Debug, Default)]
pub(super) struct Inner {
    pub(super) handle: Option<JoinHandle<()>>,
    pub(super) pause: Option<Arc<Pause>>,
    pub(super) pre_bind: bool,
}
#[derive(Debug, Default)]
pub(super) struct Outer {
    pub(super) handle: Option<JoinHandle<Option<Fault>>>,
    pub(super) populate_inner: bool,
}

pub(super) fn after_inner_spawn(
    handle: JoinHandle<()>,
    probe: Option<Arc<Mutex<Inner>>>,
) -> JoinHandle<()> {
    if let Some(probe) = probe {
        let pause = probe.lock().unwrap().pause.clone();
        if let Some(pause) = pause {
            pause.arrive();
            return handle;
        }
        probe.lock().unwrap().handle = Some(handle);
        panic!("injected post-spawn inner launch unwind before registration");
    }
    handle
}

pub(super) fn after_outer_spawn(
    handle: JoinHandle<Option<Fault>>,
    probe: Option<Arc<Mutex<Outer>>>,
    owner: &Arc<Mutex<Owner>>,
) -> JoinHandle<Option<Fault>> {
    if let Some(probe) = probe {
        if probe.lock().unwrap().populate_inner {
            let mut owner = owner.lock().unwrap();
            let job = owner.fixture_admit().unwrap();
            assert!(owner
                .try_spawn(job, TaskKind::DnsChild, std::future::pending::<()>())
                .is_ok());
        }
        probe.lock().unwrap().handle = Some(handle);
        panic!("injected post-spawn outer launch unwind before registration");
    }
    handle
}

// A deterministic native launch barrier. Test control never supplies a handle
// to the SUT; release lets the original provider return its real handle.
#[derive(Debug, Default)]
pub(super) struct Pause {
    state: Mutex<(bool, bool)>,
    changed: std::sync::Condvar,
}
impl Pause {
    fn arrive(&self) {
        let mut state = self.state.lock().unwrap();
        state.0 = true;
        self.changed.notify_all();
        while !state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }
    pub(super) fn wait(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.0 {
            state = self.changed.wait(state).unwrap();
        }
    }
    pub(super) fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
}
