//! Private actual-Hyper lifetime checkpoint, not a public runtime attachment.
//! Backend encoded buffers/RSS and inbound parser allocation are separate budgets.
//! A response claim follows Hyper's cached map into the next request owner.
pub(super) use super::request_headers::ReceivedHeaders;
use super::response_budget::{wire::*, Diagnostic, Producer};
use super::{ClosingIo, Sending, TimedIo};
use crate::security_sf07_private::{quota::Reservation, OwnedStateLock};
use axum::http::{Method, StatusCode, Uri};
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::OwnedSemaphorePermit,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SendFault {
    Foreign,
    Busy,
    Closed,
    Poisoned,
    Exhausted,
}
impl fmt::Display for SendFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("private response send refused")
    }
}
impl std::error::Error for SendFault {}

// One response head can be encoded/cached before the next service call. Hyper
// Server::poll_ready refuses the next message while in_flight is Some; receive
// transfers the previous cached claim before a later response can be published.
// No queue, replacement, or flush-based release exists.
pub(super) struct SendState {
    cached: Option<Arc<Reservation>>,
    closed: bool,
}
pub(super) struct SendLedger {
    ordinary: Arc<Producer>,
    fallback: Arc<FallbackLedger>,
    state: OwnedStateLock<SendState>,
}
impl SendLedger {
    fn new(ordinary: Arc<Producer>, fallback: Arc<FallbackLedger>) -> Arc<Self> {
        Arc::new(Self {
            ordinary,
            fallback,
            state: OwnedStateLock::new(SendState {
                cached: None,
                closed: false,
            }),
        })
    }
    pub(super) fn accepts(&self, wire: &ChargedWire) -> bool {
        wire.is_owned_by(&self.ordinary) || self.fallback.owns_wire(wire)
    }
    pub(super) fn publish(&self, charge: Arc<Reservation>) -> Result<(), SendFault> {
        let mut state = self.state.lock().map_err(|_| SendFault::Poisoned)?;
        if state.closed {
            return Err(SendFault::Closed);
        }
        if state.cached.is_some() {
            return Err(SendFault::Busy);
        }
        state.cached = Some(charge);
        Ok(())
    }
    fn receive(&self, request: hyper::Request<Incoming>) -> Result<ReceivedRequest, SendFault> {
        let prior = {
            let mut state = self.state.lock().map_err(|_| SendFault::Poisoned)?;
            if state.closed {
                return Err(SendFault::Closed);
            }
            state.cached.take()
        };
        let (parts, body) = request.into_parts();
        Ok(ReceivedRequest {
            method: parts.method,
            uri: parts.uri,
            headers: ReceivedHeaders::new(parts.headers, prior),
            _body: body,
        })
    }
}

pub(super) struct ReceivedRequest {
    pub(super) method: Method,
    pub(super) uri: Uri,
    headers: Arc<ReceivedHeaders>,
    _body: Incoming,
}
impl ReceivedRequest {
    // This is a shared owner of the SAME map, never HeaderMap::clone.
    fn snapshot(&self) -> Arc<ReceivedHeaders> {
        Arc::clone(&self.headers)
    }
}

struct AfterNative {
    ledger: Arc<SendLedger>,
}
impl Drop for AfterNative {
    fn drop(&mut self) {
        // Native future has ALREADY dropped. External ledger Arc clones cannot
        // keep the cached claim; request/frame owners keep their own claims.
        let mut state = match self.ledger.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.closed = true;
        let cached = state.cached.take();
        drop(state);
        drop(cached);
    }
}
pub(super) struct NativeDriver<F> {
    // Never extract native, complete into_parts, or detach. Ready explicitly
    // destroys native; the cache guard survives until this owner is dropped.
    // Rust drops native state/I/O, then ledger cache, THEN connection permit.
    native: Option<F>,
    _after_native: AfterNative,
    _permit: OwnedSemaphorePermit,
}
impl<F: Future + Unpin> Future for NativeDriver<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let output = match Pin::new(this.native.as_mut().expect("private native repoll")).poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(output) => output,
        };
        // Hyper service errors can return Ready without poll_shutdown. Actually
        // destroy its state/I/O NOW, before any ledger guard/permit can release.
        drop(this.native.take());
        Poll::Ready(output)
    }
}

// Uses the existing body/Sending/TimedIo/ClosingIo types. Production admission,
// dispatcher, absolute timeout/select loop and actual joins remain untouched.
// The connection permit bounds this checkpoint's fixed per-connection owner.
// Public inbound/request metadata admission is NOT established by this adapter.
/// Explicit private driver inputs. No Default and no implicit operational values.
#[derive(Clone, Copy)]
pub(super) struct DriverConfig {
    headers: usize,
    header_bytes: usize,
    header_deadline: Duration,
    send_deadline: Duration,
    close_deadline: Duration,
    keep_alive: bool,
}
impl DriverConfig {
    pub(super) fn checked(
        headers: usize,
        header_bytes: usize,
        header_deadline: Duration,
        send_deadline: Duration,
        close_deadline: Duration,
        keep_alive: bool,
    ) -> Result<Self, Diagnostic> {
        if header_bytes < 8192
            || header_bytes > isize::MAX as usize
            || headers == 0
            || headers > header_bytes / 4
            || [header_deadline, send_deadline, close_deadline]
                .into_iter()
                .any(|d| d.is_zero() || std::time::Instant::now().checked_add(d).is_none())
        {
            return Err(Diagnostic::InvalidLimits);
        }
        Ok(Self {
            headers,
            header_bytes,
            header_deadline,
            send_deadline,
            close_deadline,
            keep_alive,
        })
    }
}

// Existing unit profile only; not used by the attachment entry/configuration.
fn connection<T, H, F>(
    io: T,
    ordinary: Arc<Producer>,
    fallback: Arc<FallbackLedger>,
    permit: OwnedSemaphorePermit,
    handler: H,
) -> (
    NativeDriver<impl Future<Output = Result<(), SendFault>> + Unpin>,
    Arc<SendLedger>,
)
where
    T: AsyncRead + AsyncWrite + Unpin,
    H: Fn(ReceivedRequest) -> F,
    F: Future<Output = Result<(StatusCode, ChargedWire), FallbackStatus>>,
{
    configured_connection(
        io,
        ordinary,
        fallback,
        permit,
        DriverConfig::checked(
            16,
            8192,
            Duration::from_secs(2),
            Duration::from_secs(2),
            Duration::from_secs(2),
            true,
        )
        .unwrap(),
        handler,
    )
}

pub(super) fn configured_connection<T, H, F>(
    io: T,
    ordinary: Arc<Producer>,
    fallback: Arc<FallbackLedger>,
    permit: OwnedSemaphorePermit,
    config: DriverConfig,
    handler: H,
) -> (
    NativeDriver<impl Future<Output = Result<(), SendFault>> + Unpin>,
    Arc<SendLedger>,
)
where
    T: AsyncRead + AsyncWrite + Unpin,
    H: Fn(ReceivedRequest) -> F,
    F: Future<Output = Result<(StatusCode, ChargedWire), FallbackStatus>>,
{
    let ledger = SendLedger::new(ordinary, fallback);
    let service_ledger = Arc::clone(&ledger);
    let (sending, sent) = tokio::sync::watch::channel(Sending::default());
    let service_sending = sending.clone();
    let service = service_fn(move |request| {
        let received = service_ledger.receive(request);
        let ledger = Arc::clone(&service_ledger);
        let sending = service_sending.clone();
        let prepared = received.map(|request| {
            let head = request.method == Method::HEAD;
            (head, handler(request))
        });
        async move {
            let (head, handled) = prepared?;
            let mut response = match handled.await {
                Ok((status, mut wire)) => {
                    if head {
                        wire.suppress_body_for_head();
                    }
                    wire.transfer_native(&ledger, status)?
                }
                Err(status) => {
                    let Some((status, mut wire)) = ledger.fallback.produce(status).into_response()
                    else {
                        // A service error makes Hyper terminate, then driver
                        // destruction closes actual transport before the permit.
                        return Err(SendFault::Exhausted);
                    };
                    if head {
                        wire.suppress_body_for_head();
                    }
                    wire.transfer_native(
                        &ledger,
                        StatusCode::from_u16(status).map_err(|_| SendFault::Closed)?,
                    )?
                }
            };
            let body_transferred = response.body().bytes.is_empty();
            if !body_transferred {
                response.body_mut().sending = Some(sending.clone());
            }
            sending.send_replace(Sending {
                deadline: Some(
                    tokio::time::Instant::now()
                        .checked_add(config.send_deadline)
                        .ok_or(SendFault::Closed)?,
                ),
                discard_remaining: false,
                body_transferred,
            });
            Ok::<_, SendFault>(response)
        }
    });
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .keep_alive(config.keep_alive)
        .header_read_timeout(config.header_deadline)
        .max_headers(config.headers)
        .max_buf_size(config.header_bytes);
    let native = builder.serve_connection(
        TimedIo {
            inner: ClosingIo::new(TokioIo::new(io), config.close_deadline, sent.clone()),
            sending,
            probe: None,
            #[cfg(windows)]
            windows_stalled_fixture: false,
        },
        service,
    );
    // Keep the native future inside the same destruction guard while applying
    // the existing absolute send-watch semantics; flush only disarms the timer.
    let native = Box::pin(async move {
        tokio::pin!(native);
        let mut sent = sent;
        loop {
            let deadline = sent.borrow_and_update().deadline;
            let expires = async {
                match deadline {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                biased;
                result = &mut native => return result.map_err(|_| SendFault::Closed),
                changed = sent.changed() => {
                    if changed.is_err() { return Err(SendFault::Closed); }
                }
                _ = expires => return Err(SendFault::Closed),
            }
        }
    });
    (
        NativeDriver {
            native: Some(native),
            _after_native: AfterNative {
                ledger: Arc::clone(&ledger),
            },
            _permit: permit,
        },
        ledger,
    )
}

#[path = "response_send_tests.rs"]
mod tests;

#[path = "response_send_owned_state_tests.rs"]
mod owned_state_lock_tests;
