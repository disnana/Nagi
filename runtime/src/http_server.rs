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

pub struct Response {
    status: Status,
    headers: HeaderMap,
    body: Bytes,
}
impl Response {
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    fn into_http(mut self, head: bool) -> axum::http::Response<BufferedBody> {
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
        let mut response = axum::http::Response::new(BufferedBody { bytes: body });
        *response.status_mut() = code;
        *response.headers_mut() = self.headers;
        response
    }
}

/// This API returns one owned buffered frame. Keeping its concrete native
/// representation avoids a dynamic body box on every response.
struct BufferedBody {
    bytes: Bytes,
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
            Poll::Ready(Some(Ok(hyper::body::Frame::data(std::mem::take(
                &mut self.bytes,
            )))))
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
        status,
        headers: HeaderMap::new(),
        body: Bytes::new(),
    }
}
fn representation(status: Status, content_type: &'static str, body: Bytes) -> Response {
    let mut response = empty(status);
    response
        .headers
        .insert(names::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.body = body;
    response
}
pub fn text(status: Status, body: &str) -> Response {
    representation(
        status,
        "text/plain; charset=utf-8",
        Bytes::copy_from_slice(body.as_bytes()),
    )
}
pub fn bytes(status: Status, body: &[u8]) -> Response {
    representation(
        status,
        "application/octet-stream",
        Bytes::copy_from_slice(body),
    )
}
pub fn json<T: Serialize + ?Sized>(status: Status, body: &T) -> Result<Response, Error> {
    let body = serde_json::to_vec(body).map_err(|error| Error::internal(error.to_string()))?;
    Ok(representation(
        status,
        "application/json",
        Bytes::from(body),
    ))
}
pub fn append_header(mut response: Response, name: &str, value: &[u8]) -> Result<Response, Error> {
    let name = header_name(name)?;
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
}
impl Default for Options {
    fn default() -> Self {
        Self {
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

type ResponseFuture = Pin<Box<dyn Future<Output = Response> + Send>>;
type PolicyFuture<A> = Pin<Box<dyn Future<Output = Result<A, Failure>> + Send>>;
type PolicyFactory<S, A> = dyn Fn(Request, Arc<S>, Arc<Lease>) -> PolicyFuture<A> + Send + Sync;
/// A sealed route policy. A is its callback output, not a stored proof.
pub struct Policy<S, A> {
    factory: Arc<PolicyFactory<S, A>>,
}
impl<S, A> fmt::Debug for Policy<S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Policy { .. }")
    }
}
pub fn public_policy<S: Send + Sync + 'static>() -> Policy<S, ()> {
    Policy {
        factory: Arc::new(|_, _, _| Box::pin(async { Ok(()) })),
    }
}
fn security_response(failure: Failure) -> Response {
    let status = match failure.kind() {
        FailureKind::InvalidCredential | FailureKind::Expired => Status::UNAUTHORIZED,
        FailureKind::Denied => Status::FORBIDDEN,
        FailureKind::InvalidRequest => Status::BAD_REQUEST,
        FailureKind::Unavailable => Status::SERVICE_UNAVAILABLE,
        FailureKind::Internal => Status::INTERNAL_SERVER_ERROR,
    };
    let mut response = text(status, failure.message());
    if status == Status::UNAUTHORIZED {
        response
            .headers
            .insert(names::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
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
        factory: Arc::new(move |head, state, lease| {
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
        }),
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
        factory: Arc::new(move |head, state, lease| {
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
        }),
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
struct Route<S> {
    method: Method,
    prepare: RoutePrepare<S>,
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
    routes.push(Route {
        method,
        prepare: Box::new(move |head, state, lease| {
            let handler = Arc::clone(&handler);
            let factory = Arc::clone(&policy.factory);
            Box::pin(async move {
                let authority = factory(head, state, Arc::clone(&lease)).await?;
                let prepared: PreparedHandler<S> = Box::new(move |request, state| {
                    Box::pin(async move {
                        if let Err(failure) = lease.validate() {
                            return security_response(failure);
                        }
                        match handler(request, state, authority).await {
                            Ok(response) => response,
                            Err(error) => mapper(error),
                        }
                    })
                });
                Ok(prepared)
            })
        }),
    });
    Ok(app)
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
    if close {
        response
            .headers_mut()
            .insert(names::CONNECTION, HeaderValue::from_static("close"));
    }
    response
}
async fn dispatch<S, E>(
    app: Arc<App<S, E>>,
    request: axum::http::Request<hyper::body::Incoming>,
    options: Arc<Options>,
    requests: Arc<Semaphore>,
) -> Result<axum::http::Response<BufferedBody>, Infallible>
where
    S: Send + Sync + 'static,
    E: 'static,
{
    let head = request.method() == Method::HEAD;
    let connect = request.method() == Method::CONNECT;
    let Ok(matched) = app.paths.at(request.uri().path()) else {
        return Ok(transport(Status::NOT_FOUND, head, true));
    };
    let Some(routes) = app.routes.get(matched.value) else {
        return Ok(transport(Status::INTERNAL_SERVER_ERROR, head, true));
    };
    let selected = routes
        .iter()
        .find(|route| route.method == *request.method())
        .or_else(|| {
            if head {
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
        return Ok(response);
    };
    let Ok(_permit) = requests.try_acquire_owned() else {
        return Ok(transport(Status::SERVICE_UNAVAILABLE, head, true));
    };
    if let Some(length) = request.headers().get(names::CONTENT_LENGTH) {
        if length
            .to_str()
            .ok()
            .and_then(|length| length.parse::<u64>().ok())
            .is_some_and(|length| length > options.body_bytes as u64)
        {
            return Ok(transport(Status::CONTENT_TOO_LARGE, head, true));
        }
    }
    let (parts, body) = request.into_parts();
    let total = options
        .security_deadline
        .checked_add(options.body_deadline)
        .and_then(|t| t.checked_add(options.handler_deadline));
    let Some(deadline) = total.and_then(|t| Instant::now().checked_add(t)) else {
        return Ok(transport(Status::SERVICE_UNAVAILABLE, head, true));
    };
    let owner = match LeaseOwner::new(deadline) {
        Ok(owner) => owner,
        Err(_) => return Ok(transport(Status::SERVICE_UNAVAILABLE, head, true)),
    };
    let snapshot = Request {
        method: parts.method.clone(),
        uri: parts.uri.clone(),
        headers: parts.headers.clone(),
        body: Bytes::new(),
    };
    let security_started = Instant::now();
    let prepare = AssertUnwindSafe(async {
        (route.prepare)(snapshot, Arc::clone(&app.state), Arc::clone(&owner.lease)).await
    })
    .catch_unwind();
    let prepared = match tokio::time::timeout(options.security_deadline, prepare).await {
        Ok(_) if security_started.elapsed() >= options.security_deadline => {
            return Ok(transport(Status::GATEWAY_TIMEOUT, head, true))
        }
        Ok(Ok(Ok(prepared))) => prepared,
        Ok(Ok(Err(error))) => {
            let mut response = security_response(error).into_http(head);
            response
                .headers_mut()
                .insert(names::CONNECTION, HeaderValue::from_static("close"));
            return Ok(response);
        }
        Ok(Err(_)) => return Ok(transport(Status::INTERNAL_SERVER_ERROR, head, true)),
        Err(_) => return Ok(transport(Status::GATEWAY_TIMEOUT, head, true)),
    };
    let body = match tokio::time::timeout(
        options.body_deadline,
        receive_body(body, options.body_bytes),
    )
    .await
    {
        Ok(Ok(body)) => body,
        Ok(Err(BodyFailure::TooLarge)) => {
            return Ok(transport(Status::CONTENT_TOO_LARGE, head, true))
        }
        Ok(Err(BodyFailure::Invalid)) => return Ok(transport(Status::BAD_REQUEST, head, true)),
        Err(_) => return Ok(transport(Status::REQUEST_TIMEOUT, head, true)),
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
    let future =
        AssertUnwindSafe(async { prepared(request, Arc::clone(&app.state)).await }).catch_unwind();
    let handler_started = tokio::time::Instant::now();
    let response = match tokio::time::timeout(options.handler_deadline, future).await {
        // Tokio polls the operation before its timer. A non-yielding poll can
        // return Ready after the deadline; do not accept that late response.
        Ok(Ok(_)) if handler_started.elapsed() >= options.handler_deadline => {
            transport(Status::GATEWAY_TIMEOUT, head, false)
        }
        // A successful CONNECT changes the connection into a byte tunnel.
        // This resource API has no tunnel operation or upgrade owner.
        Ok(Ok(response)) if connect && response.status.is_success() => {
            transport(Status::NOT_IMPLEMENTED, head, true)
        }
        Ok(Ok(response)) => response.into_http(head),
        Ok(Err(_)) => transport(Status::INTERNAL_SERVER_ERROR, head, true),
        Err(_) => transport(Status::GATEWAY_TIMEOUT, head, false),
    };
    Ok(response)
}

// All responses in this API contain one complete Bytes frame. Hyper calls
// the underlying flush only after its pending headers/body buffers have been
// written, so clearing the deadline here leaves idle keep-alive timing to its
// independent header timer. Sending bytes does not reset the absolute limit.
struct TimedIo<T> {
    inner: T,
    sending: tokio::sync::watch::Sender<Option<tokio::time::Instant>>,
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
        Pin::new(&mut self.inner).poll_write(cx, buffer)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffers: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, buffers)
    }
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        if matches!(result, Poll::Ready(Ok(()))) && self.sending.borrow().is_some() {
            self.sending.send_replace(None);
        }
        result
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

pub async fn serve<S, E>(app: App<S, E>, port: i64, options: Options) -> Result<(), Error>
where
    S: Send + Sync + 'static,
    E: 'static,
{
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
            result = tasks.join_next(), if !tasks.is_empty() => { let _ = result; }
            accepted = listener.accept() => {
                let (stream, _) = match accepted {
                    Ok(accepted) => accepted,
                    Err(_) => {
                        tokio::select! { _ = &mut shutdown_signal => break, _ = tokio::time::sleep(Duration::from_millis(100)) => {} }
                        continue;
                    }
                };
                let Ok(permit) = Arc::clone(&connections).try_acquire_owned() else { drop(stream); continue; };
                let app = Arc::clone(&app);
                let options = Arc::clone(&options);
                let requests = Arc::clone(&requests);
                let (sending, mut sent) = tokio::sync::watch::channel(None);
                let send_deadline = options.send_deadline;
                let service_sending = sending.clone();
                let service = service_fn(move |request| {
                    let future = dispatch(Arc::clone(&app), request, Arc::clone(&options), Arc::clone(&requests));
                    let sending = service_sending.clone();
                    async move {
                        let response = future.await;
                        sending.send_replace(Some(tokio::time::Instant::now() + send_deadline));
                        response
                    }
                });
                let connection = builder.serve_connection(TimedIo { inner: TokioIo::new(stream), sending }, service);
                let mut stopped = shutdown.subscribe();
                tasks.spawn(async move {
                    let _permit = permit;
                    tokio::pin!(connection);
                    let mut stopping = false;
                    loop {
                        let deadline = *sent.borrow_and_update();
                        let expires = async {
                            match deadline { Some(deadline) => tokio::time::sleep_until(deadline).await, None => std::future::pending::<()>().await }
                        };
                        tokio::select! {
                            biased;
                            _ = stopped.changed(), if !stopping => {
                                stopping = true;
                                connection.as_mut().graceful_shutdown();
                            }
                            _ = &mut connection => break,
                            _ = sent.changed() => {}
                            _ = expires => break,
                        }
                    }
                });
            }
        }
    }
    drop(listener);
    let _ = shutdown.send(true);
    let drained = tokio::time::timeout(options.shutdown_deadline, async {
        while tasks.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
