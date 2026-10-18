//! Private native vertical attachment, deliberately not the public API cutover.
//! A real received request owns a finite Context; no dummy/Default producer.
//! Only framework production failures are handled here. App business errors,
//! mapper/rate policy and the public dispatch cutover remain separate gates.
use super::response_budget::{
    finalize::{CompletionOrigin, PreparedResponse, ResponseMeta},
    wire::{FallbackLedger, FallbackLimits, FallbackStatus},
    Diagnostic, Limits, Produced, Producer,
};
use super::response_send::{
    self, DriverConfig, NativeDriver, ReceivedRequest, SendFault, SendLedger,
};
use super::{BodyKind, PolicySource, Status};
use serde::Serialize;
use std::{future::Future, mem::size_of, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{OwnedSemaphorePermit, Semaphore},
};

pub(super) struct Options {
    producer: Arc<Producer>,
    fallback: Arc<FallbackLedger>,
    connections: Arc<Semaphore>,
    requests: Arc<Semaphore>,
    driver: DriverConfig,
}
fn add(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(Diagnostic::InvalidLimits)
}
impl Options {
    pub(super) fn checked(
        ordinary: Limits,
        fallback: FallbackLimits,
        connections: usize,
        requests: usize,
        external_catalog_bytes: usize,
        driver: DriverConfig,
    ) -> Result<Self, Diagnostic> {
        if connections == 0
            || connections > Semaphore::MAX_PERMITS
            || requests == 0
            || requests > Semaphore::MAX_PERMITS
            || external_catalog_bytes == 0
        {
            return Err(Diagnostic::InvalidLimits);
        }
        // Explicit request-owner count corresponds to public Options.requests,
        // independently of recycled connection permits. Retained wrappers keep
        // their own permit; startup metadata covers every permitted live owner.
        let request_metadata = requests
            .checked_mul(size_of::<Request>())
            .ok_or(Diagnostic::InvalidLimits)?;
        let metadata = add(
            external_catalog_bytes,
            add(
                request_metadata,
                add(
                    size_of::<Self>(),
                    add(2 * size_of::<Semaphore>(), 6 * size_of::<usize>())?,
                )?,
            )?,
        )?;
        let fallback = Arc::new(FallbackLedger::checked(fallback, connections)?);
        let producer = Arc::new(Producer::checked_attached(ordinary, connections, metadata)?);
        Ok(Self {
            producer,
            fallback,
            connections: Arc::new(Semaphore::new(connections)),
            requests: Arc::new(Semaphore::new(requests)),
            driver,
        })
    }
    pub(super) fn connection<T, H, F>(
        self: &Arc<Self>,
        io: T,
        handler: H,
    ) -> Result<
        (
            NativeDriver<impl Future<Output = Result<(), SendFault>> + Unpin>,
            Arc<SendLedger>,
        ),
        Diagnostic,
    >
    where
        T: AsyncRead + AsyncWrite + Unpin,
        H: Fn(Request) -> F,
        F: Future<Output = Result<Response, Diagnostic>>,
    {
        let permit = Arc::clone(&self.connections)
            .try_acquire_owned()
            .map_err(|_| Diagnostic::Capacity)?;
        let ordinary = Arc::clone(&self.producer);
        let requests = Arc::clone(&self.requests);
        Ok(response_send::configured_connection(
            io,
            Arc::clone(&ordinary),
            Arc::clone(&self.fallback),
            permit,
            self.driver,
            move |received| {
                let head = received.method == axum::http::Method::HEAD;
                // Admit before Context construction or invoking user code. A
                // retained first request may own no prior response claim at all.
                let prepared = Arc::clone(&requests).try_acquire_owned().map(|owner| {
                    let request = Request {
                        received,
                        context: ResponseContext {
                            producer: Arc::clone(&ordinary),
                        },
                        _owner: owner,
                    };
                    handler(request)
                });
                let ordinary = Arc::clone(&ordinary);
                async move {
                    let future = prepared.map_err(|_| FallbackStatus::Capacity)?;
                    let response = future.await.map_err(fallback_status)?;
                    let status = response.status.0;
                    let wire = PreparedResponse::new(
                        &ordinary,
                        response.produced,
                        ResponseMeta {
                            status,
                            kind: response.kind,
                            head,
                            source: PolicySource::Public,
                            origin: CompletionOrigin::HandlerOk,
                        },
                    )
                    .and_then(PreparedResponse::finish)
                    .map_err(fallback_status)?;
                    Ok((status, wire))
                }
            },
        ))
    }
}
fn fallback_status(error: Diagnostic) -> FallbackStatus {
    match error {
        Diagnostic::Capacity => FallbackStatus::Capacity,
        _ => FallbackStatus::Internal,
    }
}
// These are private attachment owners, not aliases for the public legacy API.
// The public Request/Options/Response cutover must replace that API atomically.
pub(super) struct Request {
    received: ReceivedRequest,
    context: ResponseContext,
    // Field order destroys received/context before releasing owner admission.
    // No private API extracts/clones received headers or this permit.
    _owner: OwnedSemaphorePermit,
}
impl Request {
    pub(super) fn path(&self) -> &str {
        self.received.uri.path()
    }
}
pub(super) struct ResponseContext {
    producer: Arc<Producer>,
}
pub(super) fn response_context(request: &Request) -> &ResponseContext {
    &request.context
}
pub(super) struct Response {
    produced: Produced,
    status: Status,
    kind: BodyKind,
}
fn representation(
    context: &ResponseContext,
    status: Status,
    kind: BodyKind,
    make: impl FnOnce(&Producer) -> Result<Produced, Diagnostic>,
) -> Result<Response, Diagnostic> {
    Ok(Response {
        produced: make(&context.producer)?,
        status,
        kind,
    })
}
pub(super) fn empty(context: &ResponseContext, status: Status) -> Result<Response, Diagnostic> {
    representation(context, status, BodyKind::Empty, Producer::empty)
}
pub(super) fn text(
    context: &ResponseContext,
    status: Status,
    body: &str,
) -> Result<Response, Diagnostic> {
    representation(context, status, BodyKind::Text, |p| p.text(body))
}
pub(super) fn bytes(
    context: &ResponseContext,
    status: Status,
    body: &[u8],
) -> Result<Response, Diagnostic> {
    representation(context, status, BodyKind::Binary, |p| p.bytes(body))
}
pub(super) fn json<T: Serialize + ?Sized>(
    context: &ResponseContext,
    status: Status,
    body: &T,
) -> Result<Response, Diagnostic> {
    representation(context, status, BodyKind::Json, |p| p.json(body))
}
pub(super) fn html_response(
    context: &ResponseContext,
    status: Status,
    document: crate::html::HtmlDocument,
) -> Result<Response, Diagnostic> {
    representation(context, status, BodyKind::Html, |p| p.html(&document))
}

#[path = "response_attachment_tests.rs"]
mod tests;
