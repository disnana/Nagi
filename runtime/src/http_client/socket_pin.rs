//! Private numeric socket boundary, including checked Q1 TCP-only failover.
//! Only the address module or its branded plan supplies a checked selection.
//! Trusted client/auth/ledger production wiring remains a separate gate.

use super::{AddressError, CheckedSnapshot, DialSelection, ServiceIdentity};
use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::Instant,
};
use tokio::{net::TcpStream, sync::watch};

const MAX_NATIVE_SOCKETS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopState {
    Open,
    Closing,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::http_client) enum SocketError {
    Address(AddressError),
    InvalidLimit,
    Capacity,
    Closing,
    Faulted,
    Deadline,
    Connect(std::io::ErrorKind),
    PeerObservation,
    PeerMismatch,
}

pub(in crate::http_client) struct DialControl {
    state: Mutex<StopState>,
    changed: watch::Sender<StopState>,
    native_gate: Option<crate::http_client::ledger::NativeGate>,
}
struct DialGuard<'a> {
    _state: std::sync::MutexGuard<'a, StopState>,
    _native: Option<crate::http_client::ledger::NativeGuard<'a>>,
}

impl DialControl {
    pub(in crate::http_client) fn new() -> Self {
        let (changed, _) = watch::channel(StopState::Open);
        Self {
            state: Mutex::new(StopState::Open),
            changed,
            native_gate: None,
        }
    }
    pub(in crate::http_client) fn for_operation(
        gate: crate::http_client::ledger::NativeGate,
    ) -> Self {
        let mut control = Self::new();
        control.native_gate = Some(gate);
        control
    }

    fn transition(&self, next: StopState) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if *state == StopState::Open {
            *state = next;
            self.changed.send_replace(next);
        }
    }

    pub(in crate::http_client) fn close(&self) {
        self.transition(StopState::Closing);
    }

    pub(in crate::http_client) fn fault(&self) {
        self.transition(StopState::Faulted);
    }

    fn checked_state(&self) -> Result<DialGuard<'_>, SocketError> {
        let native = self
            .native_gate
            .as_ref()
            .map(|gate| gate.enter())
            .transpose()
            .map_err(|error| match error {
                crate::http_client::ledger::LedgerError::Expired => SocketError::Deadline,
                crate::http_client::ledger::LedgerError::Closing => SocketError::Closing,
                _ => SocketError::Faulted,
            })?;
        let state = self.state.lock().map_err(|_| SocketError::Faulted)?;
        match *state {
            StopState::Open => Ok(DialGuard {
                _state: state,
                _native: native,
            }),
            StopState::Closing => Err(SocketError::Closing),
            StopState::Faulted => Err(SocketError::Faulted),
        }
    }
}

pub(in crate::http_client) struct SocketQuota {
    inner: Arc<Mutex<Quota>>,
}

struct Quota {
    limit: usize,
    used: usize,
}

impl SocketQuota {
    pub(in crate::http_client) fn checked(limit: usize) -> Result<Self, SocketError> {
        if !(1..=MAX_NATIVE_SOCKETS).contains(&limit) {
            return Err(SocketError::InvalidLimit);
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(Quota { limit, used: 0 })),
        })
    }

    fn reserve(&self) -> Result<SocketTicket, SocketError> {
        let mut inner = self.inner.lock().map_err(|_| SocketError::Faulted)?;
        if inner.used >= inner.limit {
            return Err(SocketError::Capacity);
        }
        inner.used += 1;
        Ok(SocketTicket {
            quota: Arc::clone(&self.inner),
        })
    }

    pub(in crate::http_client) fn used(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .used
    }
}

struct SocketTicket {
    quota: Arc<Mutex<Quota>>,
}

impl Drop for SocketTicket {
    fn drop(&mut self) {
        let mut quota = self
            .quota
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        quota.used -= 1;
    }
}

// An interrupted connect drops the native future before returning its quota.
// Tokio owns any not-yet-returned raw socket inside that future.
struct ConnectAttempt<F> {
    dial: Option<Pin<Box<F>>>,
    ticket: Option<SocketTicket>,
}

impl<F> Drop for ConnectAttempt<F> {
    fn drop(&mut self) {
        drop(self.dial.take());
        drop(self.ticket.take());
    }
}

// A returned TCP stream is dropped before its quota, including mismatch and
// cancellation paths. TLS handoff must transfer this entire owner, not a bare
// TcpStream; no such handoff is implemented in this slice.
struct OwnedSocket {
    stream: Option<TcpStream>,
    ticket: Option<SocketTicket>,
}

impl Drop for OwnedSocket {
    fn drop(&mut self) {
        drop(self.stream.take());
        drop(self.ticket.take());
    }
}

pub(in crate::http_client) struct PinnedSocket {
    owned: OwnedSocket,
    identity: ServiceIdentity,
    selected: SocketAddr,
    operation_end: Instant,
}

impl PinnedSocket {
    // A later TLS adapter must recheck the same closing/whole-end gate before
    // using the retained stream. This does not establish TLS peer identity.
    fn ready_for_tls(&self, control: &DialControl) -> Result<(), SocketError> {
        let _state = control.checked_state()?;
        if Instant::now() >= self.operation_end {
            return Err(SocketError::Deadline);
        }
        Ok(())
    }
}

async fn wait_stop(receiver: &mut watch::Receiver<StopState>) -> SocketError {
    loop {
        match *receiver.borrow_and_update() {
            StopState::Open => {}
            StopState::Closing => return SocketError::Closing,
            StopState::Faulted => return SocketError::Faulted,
        }
        if receiver.changed().await.is_err() {
            return SocketError::Faulted;
        }
    }
}

fn verify_stream(
    selected: DialSelection,
    owned: OwnedSocket,
    connect_end: Instant,
    control: &DialControl,
) -> Result<PinnedSocket, SocketError> {
    // Peer observation is native work too: order it with close/fault under the
    // same short gate, then recheck the absolute phase/whole end after it.
    let _state = control.checked_state()?;
    if Instant::now() >= selected.operation_end.min(connect_end) {
        return Err(SocketError::Deadline);
    }
    let observed = owned
        .stream
        .as_ref()
        .expect("owned native stream")
        .peer_addr()
        .map_err(|_| SocketError::PeerObservation)?;
    if observed != selected.selected {
        return Err(SocketError::PeerMismatch);
    }
    if Instant::now() >= selected.operation_end.min(connect_end) {
        return Err(SocketError::Deadline);
    }
    Ok(PinnedSocket {
        owned,
        identity: selected.identity,
        selected: selected.selected,
        operation_end: selected.operation_end,
    })
}

async fn connect(
    snapshot: CheckedSnapshot,
    connect_end: Instant,
    control: &DialControl,
    quota: &SocketQuota,
) -> Result<PinnedSocket, SocketError> {
    connect_inner(snapshot, connect_end, control, quota, TcpStream::connect).await
}

// The production caller above always passes Tokio's numeric SocketAddr path.
// This private seam permits a deterministic pending future in isolated tests;
// no arbitrary connector is exposed to the future client or a Nagi program.
async fn connect_inner<F, Fut>(
    snapshot: CheckedSnapshot,
    connect_end: Instant,
    control: &DialControl,
    quota: &SocketQuota,
    make: F,
) -> Result<PinnedSocket, SocketError>
where
    F: FnOnce(SocketAddr) -> Fut,
    Fut: Future<Output = std::io::Result<TcpStream>>,
{
    let selected = snapshot
        .begin_dial(Instant::now())
        .map_err(SocketError::Address)?;
    connect_selection_inner(selected, connect_end, control, quota, make).await
}

async fn connect_selection_inner<F, Fut>(
    selected: DialSelection,
    connect_end: Instant,
    control: &DialControl,
    quota: &SocketQuota,
    make: F,
) -> Result<PinnedSocket, SocketError>
where
    F: FnOnce(SocketAddr) -> Fut,
    Fut: Future<Output = std::io::Result<TcpStream>>,
{
    let end = selected.operation_end.min(connect_end);
    if Instant::now() >= end {
        return Err(SocketError::Deadline);
    }
    let ticket = quota.reserve()?;
    let mut stopped = control.changed.subscribe();
    // TcpStream::connect(SocketAddr) is lazy. Its first native poll is ordered
    // under the same short gate used by close/fault; no await is inside it.
    let mut attempt = ConnectAttempt {
        dial: Some(Box::pin(make(selected.selected))),
        ticket: Some(ticket),
    };
    let first = std::future::poll_fn(|cx| {
        let _state = match control.checked_state() {
            Ok(state) => state,
            Err(error) => return Poll::Ready(Err(error)),
        };
        if Instant::now() >= end {
            return Poll::Ready(Err(SocketError::Deadline));
        }
        // The snapshot was valid at begin_dial, but reservation or connector
        // preparation may have waited. Only this first native poll needs the
        // DNS validity gate; an already-started connection keeps its own end.
        if Instant::now() >= selected.valid_until {
            return Poll::Ready(Err(SocketError::Address(AddressError::Expired)));
        }
        Poll::Ready(Ok(attempt
            .dial
            .as_mut()
            .expect("pending dial")
            .as_mut()
            .poll(cx)))
    })
    .await?;
    let stream = match first {
        Poll::Ready(result) => result.map_err(|error| SocketError::Connect(error.kind()))?,
        Poll::Pending => {
            tokio::select! {
                biased;
                reason = wait_stop(&mut stopped) => return Err(reason),
                _ = tokio::time::sleep_until(end.into()) => return Err(SocketError::Deadline),
                result = attempt.dial.as_mut().expect("pending dial").as_mut() => {
                    result.map_err(|error| SocketError::Connect(error.kind()))?
                }
            }
        }
    };
    drop(attempt.dial.take());
    let ticket = attempt.ticket.take().expect("reserved socket slot");
    verify_stream(
        selected,
        OwnedSocket {
            stream: Some(stream),
            ticket: Some(ticket),
        },
        connect_end,
        control,
    )
}

#[cfg(test)]
#[path = "socket_pin_tests.rs"]
mod tests;

// The private transport consumes the entire pinned socket owner. The socket
// module remains unexported until trusted catalog/auth/ledger integration.
#[path = "tls_http1.rs"]
pub(in crate::http_client) mod tls_http1;

#[path = "q1_native.rs"]
pub(in crate::http_client) mod q1_native;
