//! Policy-required HTTP resources and one request lifecycle boundary.
use crate::auth::{AuthScope, Failure, FailureKind, Grant, Lease, LeaseOwner, VerifiedIdentity};
use crate::{Error, ErrorKind};
use axum::{
    body::Bytes,
    http::{header as names, HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
};
use futures_util::FutureExt;
use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use serde::Serialize;
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    panic::AssertUnwindSafe,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::{net::TcpListener, sync::Semaphore, task::JoinSet};

mod closing_io;
#[cfg(test)]
mod lifecycle_probe;
#[cfg(all(test, windows))]
mod windows_native_fixture;
use closing_io::ClosingIo;
mod authority;
mod cors;
mod origin;
mod vary;
pub use cors::{cors_policy, cors_route, CorsCredentials, CorsPolicy};
mod browser_login;
mod csrf_http;
mod prelogin_http;
pub(crate) use authority::checked_prelogin_origin;
use authority::require_authority;
pub(crate) use authority::ExternalOrigin as BoundOrigin;
pub use authority::{authority, trusted_proxy};
#[cfg(test)]
use prelogin_http::{finalize_prelogin, register_prelogin_bootstrap, PreloginCompletion};

pub use axum::http::Method;

/// A validated final response status. Informational responses use a different
/// protocol operation and cannot be constructed through this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Status(StatusCode);

macro_rules! statuses {
    ($($name:ident),* $(,)?) => {
        impl Status { $(pub const $name: Self = Self(StatusCode::$name);)* }
    };
}
statuses!(
    OK,
    CREATED,
    ACCEPTED,
    NON_AUTHORITATIVE_INFORMATION,
    NO_CONTENT,
    RESET_CONTENT,
    PARTIAL_CONTENT,
    MULTI_STATUS,
    ALREADY_REPORTED,
    IM_USED,
    MULTIPLE_CHOICES,
    MOVED_PERMANENTLY,
    FOUND,
    SEE_OTHER,
    NOT_MODIFIED,
    USE_PROXY,
    TEMPORARY_REDIRECT,
    PERMANENT_REDIRECT,
    BAD_REQUEST,
    UNAUTHORIZED,
    PAYMENT_REQUIRED,
    FORBIDDEN,
    NOT_FOUND,
    METHOD_NOT_ALLOWED,
    NOT_ACCEPTABLE,
    PROXY_AUTHENTICATION_REQUIRED,
    REQUEST_TIMEOUT,
    CONFLICT,
    GONE,
    LENGTH_REQUIRED,
    PRECONDITION_FAILED,
    PAYLOAD_TOO_LARGE,
    URI_TOO_LONG,
    UNSUPPORTED_MEDIA_TYPE,
    RANGE_NOT_SATISFIABLE,
    EXPECTATION_FAILED,
    IM_A_TEAPOT,
    MISDIRECTED_REQUEST,
    UNPROCESSABLE_ENTITY,
    LOCKED,
    FAILED_DEPENDENCY,
    TOO_EARLY,
    UPGRADE_REQUIRED,
    PRECONDITION_REQUIRED,
    TOO_MANY_REQUESTS,
    REQUEST_HEADER_FIELDS_TOO_LARGE,
    UNAVAILABLE_FOR_LEGAL_REASONS,
    INTERNAL_SERVER_ERROR,
    NOT_IMPLEMENTED,
    BAD_GATEWAY,
    SERVICE_UNAVAILABLE,
    GATEWAY_TIMEOUT,
    HTTP_VERSION_NOT_SUPPORTED,
    VARIANT_ALSO_NEGOTIATES,
    INSUFFICIENT_STORAGE,
    LOOP_DETECTED,
    NOT_EXTENDED,
    NETWORK_AUTHENTICATION_REQUIRED,
);
impl Status {
    pub const CONTENT_TOO_LARGE: Self = Self::PAYLOAD_TOO_LARGE;
    pub const REQUEST_ENTITY_TOO_LARGE: Self = Self::PAYLOAD_TOO_LARGE;
    pub const REQUEST_URI_TOO_LONG: Self = Self::URI_TOO_LONG;
    pub const REQUESTED_RANGE_NOT_SATISFIABLE: Self = Self::RANGE_NOT_SATISFIABLE;
    pub const UNPROCESSABLE_CONTENT: Self = Self::UNPROCESSABLE_ENTITY;

    pub fn value(self) -> i64 {
        i64::from(self.0.as_u16())
    }
    pub fn phrase(self) -> &'static str {
        self.0.canonical_reason().unwrap_or("")
    }
    pub fn is_success(self) -> bool {
        self.0.is_success()
    }
    pub fn is_redirection(self) -> bool {
        self.0.is_redirection()
    }
    pub fn is_client_error(self) -> bool {
        self.0.is_client_error()
    }
    pub fn is_server_error(self) -> bool {
        self.0.is_server_error()
    }
}
pub fn status(code: i64) -> Result<Status, Error> {
    if !(200..=599).contains(&code) {
        return Err(Error::invalid(
            "final HTTP status must be between 200 and 599",
        ));
    }
    StatusCode::from_u16(code as u16)
        .map(Status)
        .map_err(|_| Error::invalid("invalid HTTP status"))
}
pub fn method(name: &str) -> Result<Method, Error> {
    Method::from_bytes(name.as_bytes()).map_err(|_| Error::invalid("invalid HTTP method token"))
}
pub fn method_name(value: &Method) -> &str {
    value.as_str()
}

/// Native request components are moved into the handler, without cloning the
/// method, URI, headers, or a single received body chunk.
pub struct Request {
    pub method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
}
impl Request {
    pub fn path(&self) -> &str {
        self.uri.path()
    }
    pub fn query(&self) -> Option<&str> {
        self.uri.query()
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn is_get(&self) -> bool {
        self.method == Method::GET
    }
    pub fn is_post(&self) -> bool {
        self.method == Method::POST
    }
    pub fn is_put(&self) -> bool {
        self.method == Method::PUT
    }
    pub fn is_delete(&self) -> bool {
        self.method == Method::DELETE
    }
    pub fn is_patch(&self) -> bool {
        self.method == Method::PATCH
    }
    pub fn is_head(&self) -> bool {
        self.method == Method::HEAD
    }
    pub fn is_options(&self) -> bool {
        self.method == Method::OPTIONS
    }
    pub fn is_connect(&self) -> bool {
        self.method == Method::CONNECT
    }
    pub fn is_trace(&self) -> bool {
        self.method == Method::TRACE
    }
}
impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path())
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}
fn header_name(name: &str) -> Result<HeaderName, Error> {
    HeaderName::from_bytes(name.as_bytes()).map_err(|_| Error::invalid("invalid HTTP header name"))
}
fn validate_lookup_name(name: &str) -> Result<(), Error> {
    // Match native HeaderName's length and RFC token grammar without creating
    // an owned custom name for each borrowed lookup. HeaderMap then normalizes
    // borrowed names on the stack and handles case-insensitive comparison.
    if name.is_empty()
        || name.len() > 65535
        || !name.bytes().all(|byte| {
            matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' |
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' |
            b'.' | b'^' | b'_' | b'`' | b'|' | b'~')
        })
    {
        return Err(Error::invalid("invalid HTTP header name"));
    }
    Ok(())
}
pub fn headers<'a>(request: &'a Request, name: &str) -> Result<Vec<&'a [u8]>, Error> {
    validate_lookup_name(name)?;
    Ok(request
        .headers
        .get_all(name)
        .iter()
        .map(HeaderValue::as_bytes)
        .collect())
}
pub fn header<'a>(request: &'a Request, name: &str) -> Result<Option<&'a [u8]>, Error> {
    validate_lookup_name(name)?;
    let mut values = request.headers.get_all(name).iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(Error::invalid("HTTP header occurs more than once"));
    }
    Ok(first.map(HeaderValue::as_bytes))
}
pub fn header_text<'a>(request: &'a Request, name: &str) -> Result<Option<&'a str>, Error> {
    header(request, name)?
        .map(|value| {
            std::str::from_utf8(value).map_err(|_| Error::invalid("HTTP header is not UTF-8"))
        })
        .transpose()
}

/// Check one Content-Type value without allocating or copying the request body.
/// Parameters are validated but do not select a JSON encoding: JSON bodies are
/// decoded as UTF-8. Structured suffix types such as application/problem+json
/// are distinct media types and do not match application/json here.
pub fn is_json_content_type(request: &Request) -> Result<bool, Error> {
    let Some(value) = header(request, "content-type")? else {
        return Ok(false);
    };
    let (kind, subtype) =
        media_type(value).ok_or_else(|| Error::invalid("malformed HTTP Content-Type"))?;
    Ok(kind.eq_ignore_ascii_case(b"application") && subtype.eq_ignore_ascii_case(b"json"))
}

fn media_type(value: &[u8]) -> Option<(&[u8], &[u8])> {
    fn ows(value: &[u8], position: &mut usize) {
        while value
            .get(*position)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
        {
            *position += 1;
        }
    }
    fn token<'a>(value: &'a [u8], position: &mut usize) -> Option<&'a [u8]> {
        let start = *position;
        while value.get(*position).is_some_and(|byte| {
            matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' |
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' |
            b'.' | b'^' | b'_' | b'`' | b'|' | b'~')
        }) {
            *position += 1;
        }
        (*position != start).then(|| &value[start..*position])
    }
    let mut position = 0;
    ows(value, &mut position);
    let kind = token(value, &mut position)?;
    if value.get(position) != Some(&b'/') {
        return None;
    }
    position += 1;
    let subtype = token(value, &mut position)?;
    loop {
        ows(value, &mut position);
        if position == value.len() {
            return Some((kind, subtype));
        }
        if value.get(position) != Some(&b';') {
            return None;
        }
        position += 1;
        ows(value, &mut position);
        // RFC 9110 parameters permit empty semicolon-separated entries.
        if position == value.len() || value.get(position) == Some(&b';') {
            continue;
        }
        token(value, &mut position)?;
        if value.get(position) != Some(&b'=') {
            return None;
        }
        position += 1;
        if value.get(position) != Some(&b'"') {
            token(value, &mut position)?;
            continue;
        }
        position += 1;
        loop {
            match *value.get(position)? {
                b'"' => {
                    position += 1;
                    break;
                }
                b'\\' => {
                    position += 1;
                    if !matches!(*value.get(position)?, b'\t' | b' '..=b'~' | 128..=255) {
                        return None;
                    }
                }
                b'\t' | b' ' | b'!' | b'#'..=b'[' | b']'..=b'~' | 128..=255 => {}
                _ => return None,
            }
            position += 1;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyKind {
    Empty,
    Text,
    Binary,
    Json,
    Html,
}
impl BodyKind {
    fn content_type(self) -> Option<&'static str> {
        match self {
            Self::Empty => None,
            Self::Text => Some("text/plain; charset=utf-8"),
            Self::Binary => Some("application/octet-stream"),
            Self::Json => Some("application/json"),
            Self::Html => Some("text/html; charset=utf-8"),
        }
    }
}
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'none'; style-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

pub struct Response {
    session_seal: Option<crate::auth::session::ResponseSeal>,
    status: Status,
    headers: HeaderMap,
    body: Bytes,
    body_kind: BodyKind,
}
impl Response {
    pub(crate) fn has_session_seal(&self) -> bool {
        self.session_seal.is_some()
    }
    pub(crate) fn with_session_seal(mut self, seal: crate::auth::session::ResponseSeal) -> Self {
        self.session_seal = Some(seal);
        self
    }
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    fn into_http(mut self, head: bool) -> axum::http::Response<BufferedBody> {
        // Public appending already rejects this for HTML. Recheck the private
        // invariant before finalizing; incompatible internal state is a fixed
        // safe error, never a differently interpreted HTML representation.
        if self.body_kind == BodyKind::Html && self.headers.contains_key(names::CONTENT_ENCODING) {
            self = text(Status::INTERNAL_SERVER_ERROR, "internal error");
        }
        match self.body_kind.content_type() {
            Some(value) => {
                self.headers
                    .insert(names::CONTENT_TYPE, HeaderValue::from_static(value));
            }
            None => {
                self.headers.remove(names::CONTENT_TYPE);
            }
        }
        self.headers.insert(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        );
        self.headers.insert(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        );
        // User framing headers are forbidden. Set framing from the exact
        // representation we own, preserving duplicate application headers.
        let code = self.status.0;
        let no_representation = code == StatusCode::NO_CONTENT || code == StatusCode::RESET_CONTENT;
        if no_representation || code == StatusCode::NOT_MODIFIED {
            self.body = Bytes::new();
            self.headers.remove(names::CONTENT_TYPE);
            self.headers.remove(names::CONTENT_ENCODING);
            self.headers.remove(names::CONTENT_LENGTH);
            self.headers.remove(names::TRANSFER_ENCODING);
            if code == StatusCode::RESET_CONTENT {
                self.headers
                    .insert(names::CONTENT_LENGTH, HeaderValue::from_static("0"));
            }
        } else if head {
            // An integer rendered into HeaderValue cannot contain invalid
            // octets; this is not an externally supplied header value.
            // HEAD carries the corresponding representation length while its
            // body is empty. For other responses Hyper formats its exact Body
            // size directly into the wire buffer, avoiding a HeaderValue
            // allocation solely to repeat a length it already knows.
            let length = HeaderValue::from(self.body.len());
            self.headers.insert(names::CONTENT_LENGTH, length);
        }
        let body = if head { Bytes::new() } else { self.body };
        let mut response = axum::http::Response::new(BufferedBody {
            bytes: body,
            sending: None,
        });
        *response.status_mut() = code;
        *response.headers_mut() = self.headers;
        response
    }
}

/// This API returns one owned buffered frame. Keeping its concrete native
/// representation avoids a dynamic body box on every response.
struct BufferedBody {
    bytes: Bytes,
    sending: Option<tokio::sync::watch::Sender<Sending>>,
}
impl hyper::body::Body for BufferedBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Bytes>, Infallible>>> {
        if self.bytes.is_empty() {
            Poll::Ready(None)
        } else {
            let bytes = std::mem::take(&mut self.bytes);
            if let Some(sending) = self.sending.take() {
                sending.send_modify(|state| state.body_transferred = true);
            }
            Poll::Ready(Some(Ok(hyper::body::Frame::data(bytes))))
        }
    }
    fn is_end_stream(&self) -> bool {
        self.bytes.is_empty()
    }
    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::with_exact(self.bytes.len() as u64)
    }
}
impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}
pub fn empty(status: Status) -> Response {
    Response {
        session_seal: None,
        status,
        headers: HeaderMap::new(),
        body: Bytes::new(),
        body_kind: BodyKind::Empty,
    }
}
fn representation(status: Status, body_kind: BodyKind, body: Bytes) -> Response {
    let mut response = empty(status);
    if let Some(value) = body_kind.content_type() {
        response
            .headers
            .insert(names::CONTENT_TYPE, HeaderValue::from_static(value));
    }
    response.body = body;
    response.body_kind = body_kind;
    response
}
pub fn text(status: Status, body: &str) -> Response {
    representation(
        status,
        BodyKind::Text,
        Bytes::copy_from_slice(body.as_bytes()),
    )
}
pub fn bytes(status: Status, body: &[u8]) -> Response {
    representation(status, BodyKind::Binary, Bytes::copy_from_slice(body))
}
pub fn json<T: Serialize + ?Sized>(status: Status, body: &T) -> Result<Response, Error> {
    let body = serde_json::to_vec(body).map_err(|error| Error::internal(error.to_string()))?;
    Ok(representation(status, BodyKind::Json, Bytes::from(body)))
}
pub fn html_response(status: Status, document: crate::html::HtmlDocument) -> Response {
    representation(status, BodyKind::Html, Bytes::from(document.into_encoded()))
}
pub fn append_header(mut response: Response, name: &str, value: &[u8]) -> Result<Response, Error> {
    let name = header_name(name)?;
    if response.body_kind == BodyKind::Html && name == names::CONTENT_ENCODING {
        return Err(Error::invalid(
            "HTML representation encoding is managed by the server",
        ));
    }
    if matches!(
        name,
        names::CONTENT_LENGTH
            | names::TRANSFER_ENCODING
            | names::CONTENT_TYPE
            | names::SET_COOKIE
            | names::CACHE_CONTROL
            | names::VARY
            | names::WWW_AUTHENTICATE
    ) || name.as_str().starts_with("access-control-")
        || matches!(
            name.as_str(),
            "content-security-policy"
                | "content-security-policy-report-only"
                | "x-content-type-options"
                | "x-frame-options"
                | "strict-transport-security"
                | "clear-site-data"
        )
    {
        return Err(Error::invalid(
            "response framing and security headers are managed by the server",
        ));
    }
    let value =
        HeaderValue::from_bytes(value).map_err(|_| Error::invalid("invalid HTTP header value"))?;
    response.headers.append(name, value);
    Ok(response)
}
pub fn append_header_text(response: Response, name: &str, value: &str) -> Result<Response, Error> {
    append_header(response, name, value.as_bytes())
}

#[derive(Debug)]
pub struct Options {
    authority: Option<authority::Config>,
    body_bytes: usize,
    body_deadline: Duration,
    handler_deadline: Duration,
    security_deadline: Duration,
    header_deadline: Duration,
    header_bytes: usize,
    header_count: usize,
    send_deadline: Duration,
    shutdown_deadline: Duration,
    connections: usize,
    requests: usize,
    #[cfg(test)]
    lifecycle_probe: Option<Arc<lifecycle_probe::Probe>>,
    // Explicit protocol schedule for the two Windows stalled-send fixtures only.
    #[cfg(all(test, windows))]
    windows_stalled_fixture: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            authority: None,
            body_bytes: 1024 * 1024,
            body_deadline: Duration::from_secs(10),
            handler_deadline: Duration::from_secs(2),
            security_deadline: Duration::from_secs(2),
            header_deadline: Duration::from_secs(10),
            header_bytes: 32 * 1024,
            header_count: 100,
            send_deadline: Duration::from_secs(10),
            shutdown_deadline: Duration::from_secs(10),
            connections: 1024,
            requests: 256,
            #[cfg(test)]
            lifecycle_probe: None,
            #[cfg(all(test, windows))]
            windows_stalled_fixture: false,
        }
    }
}
pub fn default_options() -> Options {
    Options::default()
}
fn positive_size(value: i64, limit: usize, label: &str) -> Result<usize, Error> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0 && *value <= limit)
        .ok_or_else(|| Error::invalid(format!("{label} must be a positive supported integer")))
}
fn milliseconds(value: i64) -> Result<Duration, Error> {
    let value = u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::invalid("HTTP deadline must be positive milliseconds"))?;
    let duration = Duration::from_millis(value);
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| Error::invalid("HTTP deadline is too large"))?;
    Ok(duration)
}
pub fn options(
    body_bytes: i64,
    body_ms: i64,
    handler_ms: i64,
    shutdown_ms: i64,
) -> Result<Options, Error> {
    Ok(Options {
        body_bytes: positive_size(body_bytes, isize::MAX as usize, "body_bytes")?,
        body_deadline: milliseconds(body_ms)?,
        handler_deadline: milliseconds(handler_ms)?,
        shutdown_deadline: milliseconds(shutdown_ms)?,
        ..Options::default()
    })
}
pub fn capacity(mut options: Options, connections: i64, requests: i64) -> Result<Options, Error> {
    options.connections = positive_size(connections, Semaphore::MAX_PERMITS, "connections")?;
    options.requests = positive_size(requests, Semaphore::MAX_PERMITS, "requests")?;
    Ok(options)
}
pub fn header_timeout(mut options: Options, milliseconds_value: i64) -> Result<Options, Error> {
    options.header_deadline = milliseconds(milliseconds_value)?;
    Ok(options)
}
pub fn header_limits(mut options: Options, bytes: i64, count: i64) -> Result<Options, Error> {
    let bytes = positive_size(bytes, 1024 * 1024, "header bytes")?;
    if bytes < 8192 {
        return Err(Error::invalid(
            "HTTP header buffer must be at least 8192 bytes",
        ));
    }
    let count = positive_size(count, (bytes / 4).min(8192), "header count")?;
    options.header_bytes = bytes;
    options.header_count = count;
    Ok(options)
}
pub fn send_timeout(mut options: Options, milliseconds_value: i64) -> Result<Options, Error> {
    options.send_deadline = milliseconds(milliseconds_value)?;
    Ok(options)
}

// Completion origin is private dispatcher metadata, never a public Response
// property. A mapper cannot promote its output to handler success.
enum PreparedCompletion {
    HandlerOk(Response),
    Replacement(Response),
}
impl PreparedCompletion {
    fn is_success(&self) -> bool {
        match self {
            Self::HandlerOk(response) | Self::Replacement(response) => response.status.is_success(),
        }
    }
}
type ResponseFuture = Pin<Box<dyn Future<Output = PreparedCompletion> + Send>>;
type PolicyFuture<A> = Pin<Box<dyn Future<Output = Result<A, Failure>> + Send>>;
type PolicyFactory<S, A> = dyn Fn(Request, Arc<S>, Arc<Lease>) -> PolicyFuture<A> + Send + Sync;
/// A sealed route policy. A is its callback output, not a stored proof.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PolicySource {
    Public,
    Bearer,
    Session,
    Prelogin,
    BrowserLogin,
}
type BrowserPolicyFuture<A> =
    Pin<Box<dyn Future<Output = Result<A, crate::auth::prelogin::Failure>> + Send>>;
type BrowserPolicyFactory<S, A> = dyn Fn(Request, Arc<S>, crate::auth::prelogin::ClaimContext) -> BrowserPolicyFuture<A>
    + Send
    + Sync;
enum PolicyKind<S, A> {
    Ordinary(Arc<PolicyFactory<S, A>>),
    Session {
        store: Arc<crate::auth::session::Store>,
        factory: Arc<SessionPolicyFactory<S, A>>,
        csrf: Option<Arc<crate::auth::csrf::Inner>>,
    },
    Browser {
        pool: Arc<crate::auth::prelogin::Pool>,
        factory: Arc<BrowserPolicyFactory<S, A>>,
    },
}
enum PreparationProfile {
    Plain,
    GuardedSession,
}
type SessionPolicyFuture<A> =
    Pin<Box<dyn Future<Output = Result<(A, crate::auth::session::CsrfBinding), Failure>> + Send>>;
type SessionPolicyFactory<S, A> =
    dyn Fn(Request, Arc<S>, Arc<Lease>, PreparationProfile) -> SessionPolicyFuture<A> + Send + Sync;
fn require_session_csrf(source: PolicySource, method: &Method) -> Result<(), Error> {
    if source == PolicySource::Session
        && method != Method::GET
        && method != Method::HEAD
        && method != Method::OPTIONS
    {
        return Err(Error::invalid(
            "unsafe Session routes require a CSRF-protected policy",
        ));
    }
    Ok(())
}
pub struct Policy<S, A> {
    source: PolicySource,
    kind: PolicyKind<S, A>,
}
impl<S, A> fmt::Debug for Policy<S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Policy { .. }")
    }
}
pub fn public_policy<S: Send + Sync + 'static>() -> Policy<S, ()> {
    Policy {
        source: PolicySource::Public,
        kind: PolicyKind::Ordinary(Arc::new(|_, _, _| Box::pin(async { Ok(()) }))),
    }
}
fn security_response(failure: Failure, source: PolicySource) -> Response {
    let status = match failure.kind() {
        FailureKind::InvalidCredential | FailureKind::Expired => Status::UNAUTHORIZED,
        FailureKind::Denied => Status::FORBIDDEN,
        FailureKind::InvalidRequest => Status::BAD_REQUEST,
        FailureKind::Unavailable => Status::SERVICE_UNAVAILABLE,
        FailureKind::Internal => Status::INTERNAL_SERVER_ERROR,
    };
    let mut response = text(status, failure.message());
    if status == Status::UNAUTHORIZED {
        response.headers.insert(
            names::WWW_AUTHENTICATE,
            HeaderValue::from_static(match source {
                PolicySource::Session => "Session",
                _ => "Bearer",
            }),
        );
    }
    response
}
fn bearer_headers(request: &Request) -> Result<(), Failure> {
    let mut credentials = request.headers.get_all(names::AUTHORIZATION).iter();
    let value = credentials.next().ok_or_else(Failure::invalid_credential)?;
    if credentials.next().is_some() || request.headers.contains_key(names::COOKIE) {
        return Err(Failure::invalid_request());
    }
    let value = value.to_str().map_err(|_| Failure::invalid_request())?;
    let (scheme, token) = value
        .split_once(' ')
        .ok_or_else(Failure::invalid_credential)?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.bytes().any(|b| b <= b' ' || b == 127)
    {
        return Err(Failure::invalid_credential());
    }
    Ok(())
}
pub fn authenticated_policy<S, H, Fut>(verifier: H) -> Policy<S, AuthScope>
where
    S: Send + Sync + 'static,
    H: Fn(Request, Arc<S>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<VerifiedIdentity, Failure>> + Send + 'static,
{
    let verifier = Arc::new(verifier);
    Policy {
        source: PolicySource::Bearer,
        kind: PolicyKind::Ordinary(Arc::new(move |head, state, lease| {
            let verifier = Arc::clone(&verifier);
            Box::pin(async move {
                bearer_headers(&head)?;
                let identity = verifier(head, state).await?;
                let scope = AuthScope::bind(identity, Arc::clone(&lease))?;
                if !scope.belongs_to(&lease) {
                    return Err(Failure::invalid_request());
                }
                scope.validate()?;
                Ok(scope)
            })
        })),
    }
}
pub fn authorized_policy<S, P, H, V, A, Fut>(verifier: H, authorizer: A) -> Policy<S, Grant<P>>
where
    S: Send + Sync + 'static,
    P: 'static,
    H: Fn(Request, Arc<S>) -> V + Send + Sync + 'static,
    V: Future<Output = Result<VerifiedIdentity, Failure>> + Send + 'static,
    A: Fn(AuthScope, Request, Arc<S>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Grant<P>, Failure>> + Send + 'static,
{
    let verifier = Arc::new(verifier);
    let authorizer = Arc::new(authorizer);
    Policy {
        source: PolicySource::Bearer,
        kind: PolicyKind::Ordinary(Arc::new(move |head, state, lease| {
            let verifier = Arc::clone(&verifier);
            let authorizer = Arc::clone(&authorizer);
            Box::pin(async move {
                bearer_headers(&head)?;
                let authorization_head = head.security_snapshot();
                let identity = verifier(head, Arc::clone(&state)).await?;
                let scope = AuthScope::bind(identity, Arc::clone(&lease))?;
                let grant = authorizer(scope, authorization_head, state).await?;
                if !grant.belongs_to(&lease) {
                    return Err(Failure::invalid_request());
                }
                grant.validate()?;
                Ok(grant)
            })
        })),
    }
}
/// Fixed Session source over a checked persistent Store; no verifier fallback.
pub fn session_authenticated_policy<S: Send + Sync + 'static>(
    store: crate::auth::session::Store,
) -> Policy<S, AuthScope> {
    let store = Arc::new(store);
    Policy {
        source: PolicySource::Session,
        kind: PolicyKind::Session {
            store: Arc::clone(&store),
            csrf: None,
            factory: Arc::new(move |head, _, lease, _| {
                let store = Arc::clone(&store);
                Box::pin(async move {
                    let identity = store.lookup_http_identity(&head.headers).await?;
                    let binding =
                        crate::auth::session::CsrfBinding::from_identity(&identity, &lease)?;
                    let scope = AuthScope::bind(identity, Arc::clone(&lease))?;
                    if !scope.belongs_to(&lease) {
                        return Err(Failure::invalid_request());
                    }
                    scope.validate()?;
                    Ok((scope, binding))
                })
            }),
        },
    }
}
pub fn session_authorized_policy<S, P, A, Fut>(
    store: crate::auth::session::Store,
    authorizer: A,
) -> Policy<S, Grant<P>>
where
    S: Send + Sync + 'static,
    P: 'static,
    A: Fn(AuthScope, Request, Arc<S>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Grant<P>, Failure>> + Send + 'static,
{
    let store = Arc::new(store);
    let authorizer = Arc::new(authorizer);
    Policy {
        source: PolicySource::Session,
        kind: PolicyKind::Session {
            store: Arc::clone(&store),
            csrf: None,
            factory: Arc::new(move |mut head, state, lease, profile| {
                let store = Arc::clone(&store);
                let authorizer = Arc::clone(&authorizer);
                Box::pin(async move {
                    let identity = store.lookup_http_identity(&head.headers).await?;
                    let binding =
                        crate::auth::session::CsrfBinding::from_identity(&identity, &lease)?;
                    let scope = AuthScope::bind(identity, Arc::clone(&lease))?;
                    if matches!(profile, PreparationProfile::GuardedSession) {
                        csrf_http::strip_credentials(&mut head.headers);
                    }
                    let grant = authorizer(scope, head, state).await?;
                    if !grant.belongs_to(&lease) {
                        return Err(Failure::invalid_request());
                    }
                    grant.validate()?;
                    Ok((grant, binding))
                })
            }),
        },
    }
}
/// Attach the sealed authenticated B1 checkpoint to an existing Session policy.
pub fn session_csrf_policy<S, A>(
    mut policy: Policy<S, A>,
    store: crate::auth::csrf::CsrfStore,
) -> Result<Policy<S, A>, Error> {
    match &mut policy.kind {
        PolicyKind::Session {
            store: session,
            csrf,
            ..
        } if policy.source == PolicySource::Session
            && csrf.is_none()
            && store.matches_store(session) =>
        {
            *csrf = Some(store.into_inner());
            Ok(policy)
        }
        _ => Err(Error::invalid(
            "CSRF requires an unguarded policy over the same checked Session store",
        )),
    }
}
/// Register closed authenticated GET and explicit HEAD token delivery.
pub fn session_csrf_tokens<S, E>(
    app: App<S, E>,
    path: &str,
    store: crate::auth::csrf::CsrfStore,
) -> Result<App<S, E>, Error> {
    csrf_http::register(app, path, store.into_inner())
}
/// Dedicated POST BrowserLogin source. The named Rust adapter remains trusted
/// to verify Bearer credentials; cookie/CSRF material never reaches callbacks.
pub fn browser_login_policy<S, H, Fut>(
    pool: crate::auth::prelogin::PreLoginPool,
    verifier: H,
) -> Policy<S, AuthScope>
where
    S: Send + Sync + 'static,
    H: Fn(Request, Arc<S>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<VerifiedIdentity, Failure>> + Send + 'static,
{
    let verifier = Arc::new(verifier);
    Policy {
        source: PolicySource::BrowserLogin,
        kind: PolicyKind::Browser {
            pool: crate::auth::prelogin::into_http_pool(pool),
            factory: Arc::new(move |head, state, context| {
                let verifier = Arc::clone(&verifier);
                Box::pin(async move {
                    let lease = context.lease();
                    let identity = context.verify(|| verifier(head, state)).await?;
                    let scope = AuthScope::bind(identity, Arc::clone(&lease))
                        .map_err(crate::auth::prelogin::Failure::from_authority)?;
                    if !scope.belongs_to(&lease) {
                        return Err(crate::auth::prelogin::Failure::from_authority(
                            Failure::invalid_request(),
                        ));
                    }
                    scope
                        .validate()
                        .map_err(crate::auth::prelogin::Failure::from_authority)?;
                    Ok(scope)
                })
            }),
        },
    }
}
pub fn security_timeout(mut options: Options, ms: i64) -> Result<Options, Error> {
    options.security_deadline = milliseconds(ms)?;
    Ok(options)
}
impl Request {
    fn security_snapshot(&self) -> Self {
        Self {
            method: self.method.clone(),
            uri: self.uri.clone(),
            headers: self.headers.clone(),
            body: Bytes::new(),
        }
    }
}
type PreparedHandler<S> = Box<dyn FnOnce(Request, Arc<S>) -> ResponseFuture + Send>;
type PrepareFuture<S> = Pin<Box<dyn Future<Output = Result<PreparedHandler<S>, Failure>> + Send>>;
type RoutePrepare<S> = Box<dyn Fn(Request, Arc<S>, Arc<Lease>) -> PrepareFuture<S> + Send + Sync>;
type GuardedPrepare<S> = Box<
    dyn Fn(
            Request,
            Arc<S>,
            Arc<Lease>,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            (PreparedHandler<S>, crate::auth::session::CsrfBinding),
                            Failure,
                        >,
                    > + Send,
            >,
        > + Send
        + Sync,
>;
type BrowserPrepare<S> = Box<
    dyn Fn(
            Request,
            Arc<S>,
            crate::auth::prelogin::ClaimContext,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<PreparedHandler<S>, crate::auth::prelogin::Failure>>
                    + Send,
            >,
        > + Send
        + Sync,
>;
enum RouteKind<S> {
    Ordinary(RoutePrepare<S>),
    CsrfTokens(Arc<crate::auth::csrf::Inner>),
    Guarded {
        store: Arc<crate::auth::csrf::Inner>,
        prepare: GuardedPrepare<S>,
    },
    Bootstrap(Arc<crate::auth::prelogin::Pool>),
    Browser {
        pool: Arc<crate::auth::prelogin::Pool>,
        prepare: BrowserPrepare<S>,
    },
}
struct Route<S> {
    source: PolicySource,
    method: Method,
    kind: RouteKind<S>,
    cors: Option<CorsPolicy>,
}
pub struct App<S, E> {
    state: Arc<S>,
    mapper: fn(E) -> Response,
    routes: HashMap<String, Vec<Route<S>>>,
    paths: matchit::Router<String>,
    error: PhantomData<fn(E)>,
}
impl<S, E> fmt::Debug for App<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("App")
            .field("route_paths", &self.routes.len())
            .finish_non_exhaustive()
    }
}
pub fn app<S, E>(state: S, mapper: fn(E) -> Response) -> App<S, E> {
    App {
        state: Arc::new(state),
        mapper,
        routes: HashMap::new(),
        paths: matchit::Router::new(),
        error: PhantomData,
    }
}
fn default_error(error: Error) -> Response {
    let status = match error.kind {
        ErrorKind::Invalid => Status::BAD_REQUEST,
        ErrorKind::NotFound => Status::NOT_FOUND,
        ErrorKind::Busy => Status::SERVICE_UNAVAILABLE,
        ErrorKind::Database | ErrorKind::Internal => Status::INTERNAL_SERVER_ERROR,
    };
    let message = if matches!(error.kind, ErrorKind::Database | ErrorKind::Internal) {
        "internal error"
    } else {
        &error.message
    };
    text(status, message)
}
pub fn app_default<S>(state: S) -> App<S, Error> {
    app(state, default_error)
}
fn require_route_policies<S, E>(app: &App<S, E>) -> Result<(), Error> {
    for route in app.routes.values().flatten() {
        if matches!(&route.kind, RouteKind::Guarded { .. }) {
            if route.source != PolicySource::Session {
                return Err(Error::invalid(
                    "CSRF route has an incompatible policy source",
                ));
            }
        } else {
            require_session_csrf(route.source, &route.method)?;
        }
    }
    Ok(())
}
pub fn route<S, E, A, H, Fut>(
    app: App<S, E>,
    method: Method,
    path: &str,
    policy: Policy<S, A>,
    handler: H,
) -> Result<App<S, E>, Error>
where
    S: Send + Sync + 'static,
    E: 'static,
    A: Send + 'static,
    H: Fn(Request, Arc<S>, A) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Response, E>> + Send + 'static,
{
    let mapper = app.mapper;
    route_mapped(app, method, path, policy, handler, mapper)
}
pub fn route_mapped<S, E, F, A, H, Fut>(
    mut app: App<S, E>,
    method: Method,
    path: &str,
    policy: Policy<S, A>,
    handler: H,
    mapper: fn(F) -> Response,
) -> Result<App<S, E>, Error>
where
    S: Send + Sync + 'static,
    F: 'static,
    A: Send + 'static,
    H: Fn(Request, Arc<S>, A) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Response, F>> + Send + 'static,
{
    if !matches!(&policy.kind, PolicyKind::Session { csrf: Some(_), .. }) {
        require_session_csrf(policy.source, &method)?;
    }
    if policy.source == PolicySource::BrowserLogin && method != Method::POST {
        return Err(Error::invalid("BrowserLogin is a dedicated POST profile"));
    }
    if !path.starts_with('/')
        || path
            .bytes()
            .any(|byte| byte <= b' ' || byte == 127 || matches!(byte, b'?' | b'#'))
    {
        return Err(Error::invalid(
            "route must be an absolute path without query or fragment",
        ));
    }
    if !app.routes.contains_key(path) {
        app.paths
            .insert(path, path.to_owned())
            .map_err(|error| Error::invalid(format!("invalid HTTP route: {error}")))?;
    }
    let routes = app.routes.entry(path.to_owned()).or_default();
    if routes.iter().any(|route| route.method == method) {
        return Err(Error::invalid(
            "HTTP method and route path are already registered",
        ));
    }
    let handler = Arc::new(handler);
    let source = policy.source;
    let kind = match policy.kind {
        PolicyKind::Session { factory, csrf, .. } => {
            if let Some(store) = csrf {
                RouteKind::Guarded {
                    store,
                    prepare: Box::new(move |head, state, lease| {
                        let handler = Arc::clone(&handler);
                        let factory = Arc::clone(&factory);
                        Box::pin(async move {
                            let (authority, binding) = factory(
                                head,
                                state,
                                Arc::clone(&lease),
                                PreparationProfile::GuardedSession,
                            )
                            .await?;
                            Ok((
                                prepared_handler(handler, mapper, authority, lease, source),
                                binding,
                            ))
                        })
                    }),
                }
            } else {
                RouteKind::Ordinary(Box::new(move |head, state, lease| {
                    let handler = Arc::clone(&handler);
                    let factory = Arc::clone(&factory);
                    Box::pin(async move {
                        let (authority, _) =
                            factory(head, state, Arc::clone(&lease), PreparationProfile::Plain)
                                .await?;
                        Ok(prepared_handler(handler, mapper, authority, lease, source))
                    })
                }))
            }
        }
        PolicyKind::Ordinary(factory) => {
            RouteKind::Ordinary(Box::new(move |head, state, lease| {
                let handler = Arc::clone(&handler);
                let factory = Arc::clone(&factory);
                Box::pin(async move {
                    let authority = factory(head, state, Arc::clone(&lease)).await?;
                    Ok(prepared_handler(handler, mapper, authority, lease, source))
                })
            }))
        }
        PolicyKind::Browser { pool, factory } => RouteKind::Browser {
            pool,
            prepare: Box::new(move |head, state, context| {
                let handler = Arc::clone(&handler);
                let factory = Arc::clone(&factory);
                Box::pin(async move {
                    let lease = context.lease();
                    let authority = factory(head, state, context).await?;
                    Ok(prepared_handler(handler, mapper, authority, lease, source))
                })
            }),
        },
    };
    routes.push(Route {
        cors: None,
        source,
        method,
        kind,
    });
    Ok(app)
}
fn prepared_handler<S, A, H, Fut, F>(
    handler: Arc<H>,
    mapper: fn(F) -> Response,
    authority: A,
    lease: Arc<Lease>,
    source: PolicySource,
) -> PreparedHandler<S>
where
    S: Send + Sync + 'static,
    A: Send + 'static,
    F: 'static,
    H: Fn(Request, Arc<S>, A) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Response, F>> + Send + 'static,
{
    Box::new(move |request, state| {
        Box::pin(async move {
            if let Err(failure) = lease.validate() {
                return PreparedCompletion::Replacement(security_response(failure, source));
            }
            match handler(request, state, authority).await {
                Ok(response) => PreparedCompletion::HandlerOk(response),
                Err(error) => PreparedCompletion::Replacement(mapper(error)),
            }
        })
    })
}

enum BodyFailure {
    TooLarge,
    Invalid,
}
async fn receive_body<B>(mut body: B, maximum: usize) -> Result<Bytes, BodyFailure>
where
    B: hyper::body::Body<Data = Bytes> + Unpin,
{
    let mut first: Option<Bytes> = None;
    let mut combined: Option<Vec<u8>> = None;
    let mut total = 0usize;
    while let Some(frame) =
        futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await
    {
        let frame = frame.map_err(|_| BodyFailure::Invalid)?;
        // Trailers are protocol metadata and are not part of the exposed body
        // bytes. The current Request API exposes ordinary headers separately.
        let Ok(chunk) = frame.into_data() else {
            continue;
        };
        total = total
            .checked_add(chunk.len())
            .filter(|total| *total <= maximum)
            .ok_or(BodyFailure::TooLarge)?;
        if chunk.is_empty() {
            continue;
        }
        if let Some(combined) = combined.as_mut() {
            combined.extend_from_slice(&chunk);
        } else if let Some(first) = first.take() {
            let mut bytes = Vec::with_capacity(total);
            bytes.extend_from_slice(&first);
            bytes.extend_from_slice(&chunk);
            combined = Some(bytes);
        } else {
            first = Some(chunk);
        }
    }
    Ok(match combined {
        Some(bytes) => Bytes::from(bytes),
        None => first.unwrap_or_default(),
    })
}
fn transport(status: Status, head: bool, close: bool) -> axum::http::Response<BufferedBody> {
    let mut response = text(status, status.phrase()).into_http(head);
    response
        .headers_mut()
        .insert(names::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if close {
        response
            .headers_mut()
            .insert(names::CONNECTION, HeaderValue::from_static("close"));
    }
    response
}
fn authority_rejection(status: Status, head: bool) -> axum::http::Response<BufferedBody> {
    let mut response = transport(status, head, true);
    response
        .headers_mut()
        .insert(names::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
// Managed authenticated headers are shared by successful delivery and all
// terminal replacements; secret take is confined to the success boundary below.
fn finalize_authenticated(
    mut response: axum::http::Response<BufferedBody>,
    source: PolicySource,
    head: bool,
) -> axum::http::Response<BufferedBody> {
    if source == PolicySource::Public {
        return response;
    }
    if response.status() == StatusCode::NOT_MODIFIED
        || response
            .headers()
            .get_all(names::CACHE_CONTROL)
            .iter()
            .any(|value| value.as_bytes() != b"no-store")
    {
        response = transport(Status::INTERNAL_SERVER_ERROR, head, true);
    }
    response
        .headers_mut()
        .insert(names::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    vary::add(&mut response, vary::Dependencies::source(source));
    response
}
// A response can retain only a nonsecret seal; the unique request owner supplies
// the material through the checked, destructive take. No field move from a Drop
// owner, secret clone, callback or await occurs in this terminal boundary.
fn finalize_session_response(
    mut response: Response,
    owner: &LeaseOwner,
    source: PolicySource,
    head: bool,
) -> axum::http::Response<BufferedBody> {
    let has_seal = response.session_seal.is_some();
    let effective_source = if has_seal && source == PolicySource::Public {
        PolicySource::Session
    } else {
        source
    };
    if has_seal
        && (response.status.0 == StatusCode::NOT_MODIFIED
            || response
                .headers
                .get_all(names::CACHE_CONTROL)
                .iter()
                .any(|value| value.as_bytes() != b"no-store"))
    {
        crate::auth::session::retire_http_delivery(owner);
        return finalize_authenticated(
            transport(Status::INTERNAL_SERVER_ERROR, head, true),
            effective_source,
            head,
        );
    }
    let cookie = match response.session_seal.take() {
        Some(seal) => match crate::auth::session::take_http_cookie(owner, seal) {
            Ok(cookie) => Some(cookie),
            Err(()) => {
                crate::auth::session::retire_http_delivery(owner);
                return finalize_authenticated(
                    transport(Status::INTERNAL_SERVER_ERROR, head, true),
                    effective_source,
                    head,
                );
            }
        },
        None => {
            crate::auth::session::retire_http_delivery(owner);
            None
        }
    };
    let mut wire = finalize_authenticated(response.into_http(head), effective_source, head);
    if let Some(cookie) = cookie {
        wire.headers_mut().append(names::SET_COOKIE, cookie);
        vary::add(&mut wire, vary::Dependencies::COOKIE);
    }
    wire
}
async fn dispatch<S, E>(
    app: Arc<App<S, E>>,
    request: axum::http::Request<hyper::body::Incoming>,
    options: Arc<Options>,
    requests: Arc<Semaphore>,
    peer: std::net::IpAddr,
) -> Result<axum::http::Response<BufferedBody>, Infallible>
where
    S: Send + Sync + 'static,
    E: 'static,
{
    let head = request.method() == Method::HEAD;
    let Some(authority) = options.authority.as_ref() else {
        return Ok(authority_rejection(Status::INTERNAL_SERVER_ERROR, head));
    };
    if let Err(status) = authority.check(request.version(), request.uri(), request.headers(), peer)
    {
        let preflight = cors::candidate(request.method(), request.headers());
        let response = authority_rejection(status, head);
        // Metadata-only failures carry no credential-source dependencies, even
        // when an ordinary authenticated OPTIONS record exists at this path.
        let response = if preflight {
            response
        } else {
            csrf_http::finalize_path(response, &app, request.method(), request.uri().path(), head)
        };
        return Ok(cors::route_less(response, preflight));
    }
    let preflight = cors::candidate(request.method(), request.headers());
    let requested_method = if preflight {
        match cors::requested_method(request.headers()) {
            Ok(method) => method,
            Err(status) => return Ok(cors::route_less(transport(status, false, true), true)),
        }
    } else {
        request.method().clone()
    };
    let select_head = requested_method == Method::HEAD;
    let connect = request.method() == Method::CONNECT;
    let Ok(matched) = app.paths.at(request.uri().path()) else {
        return Ok(cors::route_less(
            transport(Status::NOT_FOUND, head, true),
            preflight,
        ));
    };
    let Some(routes) = app.routes.get(matched.value) else {
        return Ok(cors::route_less(
            transport(Status::INTERNAL_SERVER_ERROR, head, true),
            preflight,
        ));
    };
    let selected = routes
        .iter()
        .find(|route| route.method == requested_method)
        .or_else(|| {
            if select_head {
                routes.iter().find(|route| route.method == Method::GET)
            } else {
                None
            }
        });
    let Some(route) = selected else {
        let mut allowed = routes
            .iter()
            .map(|route| route.method.as_str())
            .collect::<Vec<_>>();
        if allowed.contains(&"GET") && !allowed.contains(&"HEAD") {
            allowed.push("HEAD");
        }
        allowed.sort_unstable();
        let mut response = transport(Status::METHOD_NOT_ALLOWED, head, true);
        if let Ok(value) = HeaderValue::from_str(&allowed.join(", ")) {
            response.headers_mut().insert(names::ALLOW, value);
        }
        return Ok(cors::route_less(response, preflight));
    };
    // Capture immutable CORS metadata before any credential stripping or await.
    let cors_context = cors::Context::new(route, request.headers(), preflight);
    let response = async {
        let finish = |response| {
            if preflight {
                response
            } else {
                csrf_http::finalize_route(response, route, head)
            }
        };
        let Ok(_permit) = requests.try_acquire_owned() else {
            return Ok(finish(transport(Status::SERVICE_UNAVAILABLE, head, true)));
        };
        if let Some(length) = request.headers().get(names::CONTENT_LENGTH) {
            if length
                .to_str()
                .ok()
                .and_then(|length| length.parse::<u64>().ok())
                .is_some_and(|length| length > options.body_bytes as u64)
            {
                return Ok(finish(transport(Status::CONTENT_TOO_LARGE, head, true)));
            }
        }
        if preflight {
            return Ok(cors::preflight(
                route,
                &requested_method,
                request.headers(),
                request.body(),
                &options,
            ));
        }
        let (parts, body) = request.into_parts();
        let total = options
            .security_deadline
            .checked_add(options.body_deadline)
            .and_then(|t| t.checked_add(options.handler_deadline));
        let admitted = Instant::now();
        let Some(deadline) = total.and_then(|t| admitted.checked_add(t)) else {
            return Ok(finish(transport(Status::SERVICE_UNAVAILABLE, head, true)));
        };
        let owner = match LeaseOwner::new(deadline) {
            Ok(owner) => owner,
            Err(_) => return Ok(finish(transport(Status::SERVICE_UNAVAILABLE, head, true))),
        };
        let prepare_route = match &route.kind {
            RouteKind::CsrfTokens(store) => {
                return Ok(csrf_http::dispatch_tokens(
                    store, parts, body, &options, owner, head, admitted,
                )
                .await)
            }
            RouteKind::Guarded { store, prepare } => {
                return Ok(csrf_http::dispatch_guarded(
                    store,
                    prepare,
                    Arc::clone(&app.state),
                    parts,
                    body,
                    &options,
                    owner,
                    head,
                    admitted,
                )
                .await)
            }
            RouteKind::Browser { pool, prepare } => {
                return Ok(browser_login::dispatch_browser(
                    pool,
                    prepare,
                    Arc::clone(&app.state),
                    axum::http::Request::from_parts(parts, body),
                    &options,
                    owner,
                    admitted,
                )
                .await);
            }
            RouteKind::Ordinary(prepare) => prepare,
            RouteKind::Bootstrap(pool) => {
                return Ok(prelogin_http::dispatch_bootstrap(
                    pool, parts, body, &options, owner, head, admitted,
                )
                .await);
            }
        };
        // All terminal replacements discard the slot, including pending/ready/
        // applied material; accepted SQL effects are not rolled back by this Drop.
        let finish = |response| {
            crate::auth::session::retire_http_delivery(&owner);
            finalize_authenticated(response, route.source, head)
        };
        let snapshot = Request {
            method: parts.method.clone(),
            uri: parts.uri.clone(),
            headers: parts.headers.clone(),
            body: Bytes::new(),
        };
        let security_started = Instant::now();
        let prepare = AssertUnwindSafe(async {
            (prepare_route)(snapshot, Arc::clone(&app.state), Arc::clone(&owner.lease)).await
        })
        .catch_unwind();
        let prepared = match tokio::time::timeout(options.security_deadline, prepare).await {
            Ok(_) if security_started.elapsed() >= options.security_deadline => {
                return Ok(finish(transport(Status::GATEWAY_TIMEOUT, head, true)))
            }
            Ok(Ok(Ok(prepared))) => prepared,
            Ok(Ok(Err(error))) => {
                let mut response = security_response(error, route.source).into_http(head);
                response
                    .headers_mut()
                    .insert(names::CONNECTION, HeaderValue::from_static("close"));
                return Ok(finish(response));
            }
            Ok(Err(_)) => return Ok(finish(transport(Status::INTERNAL_SERVER_ERROR, head, true))),
            Err(_) => return Ok(finish(transport(Status::GATEWAY_TIMEOUT, head, true))),
        };
        let body = match tokio::time::timeout(
            options.body_deadline,
            receive_body(body, options.body_bytes),
        )
        .await
        {
            Ok(Ok(body)) => body,
            Ok(Err(BodyFailure::TooLarge)) => {
                return Ok(finish(transport(Status::CONTENT_TOO_LARGE, head, true)))
            }
            Ok(Err(BodyFailure::Invalid)) => {
                return Ok(finish(transport(Status::BAD_REQUEST, head, true)))
            }
            Err(_) => return Ok(finish(transport(Status::REQUEST_TIMEOUT, head, true))),
        };
        let request = Request {
            method: parts.method,
            uri: parts.uri,
            headers: parts.headers,
            body,
        };
        // The constructor is synchronous for Rust hosts; both it and subsequent
        // handler/error-mapper polling form one request failure boundary. Catching
        // an unwind does not roll back application state or repair poisoned locks.
        let future = AssertUnwindSafe(async { prepared(request, Arc::clone(&app.state)).await })
            .catch_unwind();
        let handler_started = tokio::time::Instant::now();
        let response = match tokio::time::timeout(options.handler_deadline, future).await {
            // Tokio polls the operation before its timer. A non-yielding poll can
            // return Ready after the deadline; do not accept that late response.
            Ok(Ok(_)) if handler_started.elapsed() >= options.handler_deadline => {
                transport(Status::GATEWAY_TIMEOUT, head, false)
            }
            // A successful CONNECT changes the connection into a byte tunnel.
            // This resource API has no tunnel operation or upgrade owner.
            Ok(Ok(completion)) if connect && completion.is_success() => {
                transport(Status::NOT_IMPLEMENTED, head, true)
            }
            Ok(Ok(PreparedCompletion::HandlerOk(response))) => {
                finalize_session_response(response, &owner, route.source, head)
            }
            Ok(Ok(PreparedCompletion::Replacement(response))) => {
                // Retire before conversion/mapping output reaches any wire path.
                // The mapped status/body remain ordinary response data, but a
                // same-request valid seal cannot make handler Err deliver a cookie.
                crate::auth::session::retire_http_delivery(&owner);
                let source = if response.has_session_seal() && route.source == PolicySource::Public
                {
                    PolicySource::Session
                } else {
                    route.source
                };
                finalize_authenticated(response.into_http(head), source, head)
            }
            Ok(Err(_)) => transport(Status::INTERNAL_SERVER_ERROR, head, true),
            Err(_) => transport(Status::GATEWAY_TIMEOUT, head, false),
        };
        Ok::<_, Infallible>(finish(response))
    }
    .await?;
    Ok(cors_context.finish(response))
}

// Hyper can flush headers before pulling the body's one buffered frame.
// Clearing the original deadline requires both frame handoff and a successful
// native flush. Idle keep-alive timing then uses its independent header timer.
// Sending bytes and intermediate flushes do not reset the absolute limit.
#[derive(Clone, Copy, Default)]
struct Sending {
    deadline: Option<tokio::time::Instant>,
    discard_remaining: bool,
    body_transferred: bool,
}
struct TimedIo<T> {
    inner: T,
    sending: tokio::sync::watch::Sender<Sending>,
    #[cfg(test)]
    probe: Option<Arc<lifecycle_probe::Probe>>,
    #[cfg(all(test, windows))]
    windows_stalled_fixture: bool,
}
impl<T: hyper::rt::Read + Unpin> hyper::rt::Read for TimedIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buffer)
    }
}
impl<T: hyper::rt::Write + Unpin> hyper::rt::Write for TimedIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        #[cfg(all(test, windows))]
        let result = if self.windows_stalled_fixture {
            let transferred = self.sending.borrow().body_transferred;
            windows_native_fixture::poll_windows_scalar_once(&mut self.inner, cx, buffer, transferred)
        } else {
            Pin::new(&mut self.inner).poll_write(cx, buffer)
        };
        #[cfg(not(all(test, windows)))]
        let result = Pin::new(&mut self.inner).poll_write(cx, buffer);
        #[cfg(test)]
        if result.is_pending() {
            if let Some(probe) = &self.probe {
                probe.pending(self.sending.borrow().deadline);
            }
        }
        result
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffers: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        #[cfg(all(test, windows))]
        let result = if self.windows_stalled_fixture {
            let transferred = self.sending.borrow().body_transferred;
            windows_native_fixture::poll_windows_vectored_once(&mut self.inner, cx, buffers, transferred)
        } else {
            Pin::new(&mut self.inner).poll_write_vectored(cx, buffers)
        };
        #[cfg(not(all(test, windows)))]
        let result = Pin::new(&mut self.inner).poll_write_vectored(cx, buffers);
        #[cfg(test)]
        if result.is_pending() {
            if let Some(probe) = &self.probe {
                probe.pending(self.sending.borrow().deadline);
            }
        }
        result
    }
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        let state = *self.sending.borrow();
        if matches!(result, Poll::Ready(Ok(())))
            && state.deadline.is_some()
            && state.body_transferred
        {
            self.sending.send_modify(|state| state.deadline = None);
        }
        result
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Register fixed bootstrap GET and explicit HEAD. No user callback or response
/// factory can inspect the secret material; startup still checks pool origin.
pub fn prelogin_bootstrap<S, E>(
    app: App<S, E>,
    path: &str,
    pool: crate::auth::prelogin::PreLoginPool,
) -> Result<App<S, E>, Error> {
    prelogin_http::register_prelogin_bootstrap(
        app,
        path,
        crate::auth::prelogin::into_http_pool(pool),
    )
}

pub async fn serve<S, E>(app: App<S, E>, port: i64, options: Options) -> Result<(), Error>
where
    S: Send + Sync + 'static,
    E: 'static,
{
    require_authority(&options)?;
    require_route_policies(&app)?;
    prelogin_http::check_startup(&app, &options)?;
    csrf_http::check_startup(&app, &options)?;
    let port = u16::try_from(port)
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| Error::invalid("HTTP port must be between 1 and 65535"))?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|error| Error::internal(error.to_string()))?;
    serve_listener(listener, app, options, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
}
/// Listener/shutdown injection keeps lifecycle tests deterministic and also
/// lets a Rust host integrate Nagi with its existing termination signal.
pub async fn serve_listener<S, E>(
    listener: TcpListener,
    app: App<S, E>,
    options: Options,
    shutdown_signal: impl Future<Output = ()>,
) -> Result<(), Error>
where
    S: Send + Sync + 'static,
    E: 'static,
{
    require_authority(&options)?;
    require_route_policies(&app)?;
    prelogin_http::check_startup(&app, &options)?;
    csrf_http::check_startup(&app, &options)?;
    let app = Arc::new(app);
    let options = Arc::new(options);
    let connections = Arc::new(Semaphore::new(options.connections));
    let requests = Arc::new(Semaphore::new(options.requests));
    let (shutdown, _) = tokio::sync::watch::channel(false);
    let mut tasks = JoinSet::new();
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(options.header_deadline)
        .max_buf_size(options.header_bytes);
    // The native default is the same 100-header limit and uses its optimized
    // fixed array. Reserve configured parser storage only for other limits.
    if options.header_count != 100 {
        builder.max_headers(options.header_count);
    }
    tokio::pin!(shutdown_signal);
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_signal => break,
            result = tasks.join_next(), if !tasks.is_empty() => {
                #[cfg(test)]
                if result.is_some() {
                    if let Some(probe) = &options.lifecycle_probe { probe.joined(); }
                }
                let _ = result;
            }
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(_) => {
                        tokio::select! { _ = &mut shutdown_signal => break, _ = tokio::time::sleep(Duration::from_millis(100)) => {} }
                        continue;
                    }
                };
                let Ok(permit) = Arc::clone(&connections).try_acquire_owned() else { drop(stream); continue; };
                #[cfg(all(test, windows))]
                let windows_stalled_fixture = options.windows_stalled_fixture;
                #[cfg(all(test, windows))]
                if windows_stalled_fixture {
                    let (_, after) = windows_native_fixture::accepted_send_buffer(&stream)
                        .expect("owned accepted-socket buffer configuration");
                    assert_eq!(after, 4096, "owned accepted-socket buffer readback");
                }
                let app = Arc::clone(&app);
                let options = Arc::clone(&options);
                let requests = Arc::clone(&requests);
                let (sending, mut sent) = tokio::sync::watch::channel(Sending::default());
                let send_deadline = options.send_deadline;
                let close_deadline = options.body_deadline;
                let service_sending = sending.clone();
                #[cfg(test)]
                let probe = options.lifecycle_probe.clone();
                #[cfg(test)]
                let service_probe = probe.clone();
                let service = service_fn(move |request| {
                    let may_have_body = !hyper::body::Body::is_end_stream(request.body());
                    let future = dispatch(Arc::clone(&app), request, Arc::clone(&options), Arc::clone(&requests), peer.ip());
                    let sending = service_sending.clone();
                    #[cfg(test)]
                    let probe = service_probe.clone();
                    async move {
                        let mut response = future.await;
                        let discard_remaining = may_have_body && response.as_ref().is_ok_and(|response| {
                            response.headers().get(names::CONNECTION).is_some_and(|value| value == "close")
                        });
                        let body_transferred = response.as_ref().is_ok_and(|response| response.body().bytes.is_empty());
                        let state = Sending { deadline: Some(tokio::time::Instant::now() + send_deadline), discard_remaining, body_transferred };
                        if let Ok(response) = &mut response {
                            if !body_transferred {
                                response.body_mut().sending = Some(sending.clone());
                            }
                        }
                        #[cfg(test)]
                        if let (Some(probe), Ok(response)) = (&probe, &response) {
                            probe.arm(state.deadline.unwrap(), response.status().as_u16(), response.body().bytes.len());
                        }
                        sending.send_replace(state);
                        response
                    }
                });
                let connection = builder.serve_connection(TimedIo {
                    inner: ClosingIo::new(TokioIo::new(stream), close_deadline, sent.clone()), sending,
                    #[cfg(test)]
                    probe: probe.clone(),
                    #[cfg(all(test, windows))]
                    windows_stalled_fixture,
                }, service);
                let mut stopped = shutdown.subscribe();
                tasks.spawn(async move {
                    let _permit = permit;
                    tokio::pin!(connection);
                    let mut stopping = false;
                    loop {
                        let deadline = sent.borrow_and_update().deadline;
                        let expires = async {
                            match deadline { Some(deadline) => tokio::time::sleep_until(deadline).await, None => std::future::pending::<()>().await }
                        };
                        tokio::select! {
                            biased;
                            _ = stopped.changed(), if !stopping => {
                                stopping = true;
                                #[cfg(test)]
                                if let Some(probe) = &probe { probe.graceful(deadline); }
                                connection.as_mut().graceful_shutdown();
                            }
                            _ = &mut connection => break,
                            _ = sent.changed() => {}
                            _ = expires => {
                                #[cfg(test)]
                                if let Some(probe) = &probe { probe.expired(deadline); }
                                break;
                            },
                        }
                    }
                });
            }
        }
    }
    drop(listener);
    #[cfg(test)]
    if let Some(probe) = &options.lifecycle_probe {
        probe.listener_closed();
    }
    let _ = shutdown.send(true);
    let drained = tokio::time::timeout(options.shutdown_deadline, async {
        while tasks.join_next().await.is_some() {
            #[cfg(test)]
            if let Some(probe) = &options.lifecycle_probe {
                probe.joined();
            }
        }
    })
    .await;
    if drained.is_err() {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {
            #[cfg(test)]
            if let Some(probe) = &options.lifecycle_probe {
                probe.joined();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

// Finite private impossible-state oracle; no production header setter/getter.
#[cfg(test)]
pub(crate) fn conflicting_cache_for_test(mut response: Response) -> Response {
    response
        .headers
        .insert(names::CACHE_CONTROL, HeaderValue::from_static("public"));
    response
}

#[cfg(test)]
#[path = "http_server/prelogin_tests.rs"]
mod prelogin_tests;

#[cfg(test)]
mod finalizer_tests;

#[cfg(test)]
mod finalizer_html_tests;

// Unit-only view of the actual destructive finalizer. No public route, policy
// bypass, authority mint or transport success is supplied by this adapter.
#[cfg(test)]
pub(crate) fn session_delivery_headers_for_test(
    response: Response,
    owner: &LeaseOwner,
) -> (StatusCode, axum::http::HeaderMap) {
    let wire = vary::finish(finalize_session_response(
        response,
        owner,
        PolicySource::Session,
        false,
    ));
    (wire.status(), wire.into_parts().0.headers)
}
