//! Closed authenticated token delivery and one-budget guarded Session dispatch.
use super::*;
#[cfg(nagi_auth_stage_diag)]
use crate::auth::session_deadline_observer::{
    self as deadline_observer, Exit, Observer, Phase, TokenTerminal,
};
use crate::auth::{csrf, session::CsrfBinding};

#[path = "csrf_body_deadline.rs"]
mod body_deadline;
use body_deadline::select_body_deadline;

pub(super) fn strip_credentials(headers: &mut HeaderMap) {
    headers.remove(names::COOKIE);
    headers.remove("x-csrf-token");
}
pub(super) fn register<S, E>(
    mut app: App<S, E>,
    path: &str,
    store: Arc<csrf::Inner>,
) -> Result<App<S, E>, Error> {
    if !path.starts_with('/')
        || path
            .bytes()
            .any(|b| b <= b' ' || b == 127 || matches!(b, b'?' | b'#'))
    {
        return Err(Error::invalid(
            "route must be an absolute path without query or fragment",
        ));
    }
    if app.routes.get(path).is_some_and(|routes| {
        routes
            .iter()
            .any(|r| r.method == Method::GET || r.method == Method::HEAD)
    }) {
        return Err(Error::invalid(
            "HTTP method and route path are already registered",
        ));
    }
    if !app.routes.contains_key(path) {
        app.paths
            .insert(path, path.to_owned())
            .map_err(|_| Error::invalid("invalid HTTP route"))?;
    }
    let routes = app.routes.entry(path.to_owned()).or_default();
    for method in [Method::GET, Method::HEAD] {
        routes.push(Route {
            cors: None,
            source: PolicySource::Session,
            method,
            kind: RouteKind::CsrfTokens(Arc::clone(&store)),
        });
    }
    Ok(app)
}
pub(super) fn check_startup<S, E>(app: &App<S, E>, options: &Options) -> Result<(), Error> {
    let origin = options
        .authority
        .as_ref()
        .ok_or_else(|| {
            Error::invalid("HTTP authority must be explicitly configured before serving")
        })?
        .origin();
    for route in app.routes.values().flatten() {
        if let RouteKind::Guarded { store, .. } | RouteKind::CsrfTokens(store) = &route.kind {
            if !store.bound_to(origin) {
                return Err(Error::invalid(
                    "CSRF origin does not match checked startup authority",
                ));
            }
        }
    }
    Ok(())
}
fn managed(
    mut response: axum::http::Response<BufferedBody>,
    head: bool,
    guarded: bool,
) -> axum::http::Response<BufferedBody> {
    response = finalize_authenticated(response, PolicySource::Session, head);
    vary::add(
        &mut response,
        if guarded {
            vary::Dependencies::GUARDED
        } else {
            vary::Dependencies::LOGIN
        },
    );
    response
}
pub(super) fn finalize_route<S>(
    response: axum::http::Response<BufferedBody>,
    route: &Route<S>,
    head: bool,
) -> axum::http::Response<BufferedBody> {
    match &route.kind {
        RouteKind::Guarded { .. } => managed(response, head, true),
        RouteKind::CsrfTokens(_) => managed(response, head, false),
        _ => finalize_authenticated(response, route.source, head),
    }
}
pub(super) fn finalize_path<S, E>(
    response: axum::http::Response<BufferedBody>,
    app: &App<S, E>,
    method: &Method,
    path: &str,
    head: bool,
) -> axum::http::Response<BufferedBody> {
    if let Ok(m) = app.paths.at(path) {
        if let Some(r) = app.routes.get(m.value).and_then(|rs| {
            rs.iter().find(|r| r.method == *method).or_else(|| {
                if head {
                    rs.iter().find(|r| r.method == Method::GET)
                } else {
                    None
                }
            })
        }) {
            return finalize_route(response, r, head);
        }
    }
    response
}
fn terminal(
    owner: &LeaseOwner,
    status: Status,
    head: bool,
    guarded: bool,
) -> axum::http::Response<BufferedBody> {
    crate::auth::session::retire_http_delivery(owner);
    managed(transport(status, head, true), head, guarded)
}
fn token_terminal(
    owner: &csrf::TokenOwner,
    status: Status,
    head: bool,
) -> axum::http::Response<BufferedBody> {
    owner.retire();
    terminal(&owner.lease, status, head, false)
}
fn live_until(owner: &LeaseOwner, until: Instant) -> Result<Instant, Status> {
    csrf::live_deadline(&owner.lease)
        .map(|d| d.min(until))
        .map_err(|f| f.http_status())
}
// Binding happens inside a preparation poll, before the authorizer can yield.
// Re-read its actual gate after that poll and arm the earlier absolute timer.
// Check the gate on both sides of polling so expiry wins every completion kind.
async fn await_preparation<T>(
    owner: &LeaseOwner,
    until: Instant,
    future: impl Future<Output = T>,
) -> Result<T, Status> {
    tokio::pin!(future);
    let timer = tokio::time::sleep_until(tokio::time::Instant::from_std(until));
    tokio::pin!(timer);
    std::future::poll_fn(|cx| {
        if let Err(status) = live_until(owner, until) {
            return Poll::Ready(Err(status));
        }
        let completion = future.as_mut().poll(cx);
        let deadline = match live_until(owner, until) {
            Ok(deadline) => deadline,
            Err(status) => return Poll::Ready(Err(status)),
        };
        if let Poll::Ready(value) = completion {
            return Poll::Ready(Ok(value));
        }
        timer
            .as_mut()
            .reset(tokio::time::Instant::from_std(deadline));
        match timer.as_mut().poll(cx) {
            Poll::Ready(()) => Poll::Ready(Err(Status::GATEWAY_TIMEOUT)),
            Poll::Pending => Poll::Pending,
        }
    })
    .await
}

pub(super) async fn dispatch_tokens(
    store: &csrf::Inner,
    parts: axum::http::request::Parts,
    body: hyper::body::Incoming,
    options: &Options,
    lease: LeaseOwner,
    head: bool,
    admitted: Instant,
) -> axum::http::Response<BufferedBody> {
    #[cfg(nagi_auth_stage_diag)]
    {
        if let Some(until) = admitted.checked_add(options.security_deadline) {
            let observer = Observer::new(admitted, until);
            let response = observer
                .scope(dispatch_tokens_inner(
                    store, parts, body, options, lease, head, admitted,
                ))
                .await;
            observer.emit_token(response.status().as_u16());
            return response;
        }
    }
    dispatch_tokens_inner(store, parts, body, options, lease, head, admitted).await
}
async fn dispatch_tokens_inner(
    store: &csrf::Inner,
    parts: axum::http::request::Parts,
    body: hyper::body::Incoming,
    options: &Options,
    lease: LeaseOwner,
    head: bool,
    admitted: Instant,
) -> axum::http::Response<BufferedBody> {
    let owner = csrf::TokenOwner::new(lease);
    let Some(until) = admitted.checked_add(options.security_deadline) else {
        return token_terminal(&owner, Status::SERVICE_UNAVAILABLE, head);
    };
    if let Err(e) = csrf::constrain(&owner.lease.lease, until) {
        #[cfg(nagi_auth_stage_diag)]
        deadline_observer::token_terminal(TokenTerminal::Constrain);
        return token_terminal(&owner, e.http_status(), head);
    }
    if !hyper::body::Body::is_end_stream(&body)
        || parts.headers.contains_key(names::TRANSFER_ENCODING)
    {
        return token_terminal(&owner, Status::BAD_REQUEST, head);
    }
    if let Err(status) = options
        .authority
        .as_ref()
        .ok_or(Status::INTERNAL_SERVER_ERROR)
        .and_then(|a| a.check_safe_origin(&parts.headers))
    {
        return token_terminal(&owner, status, head);
    }
    let future = async {
        let identity_future = store.identity(&parts.headers);
        #[cfg(nagi_auth_stage_diag)]
        let identity_future = deadline_observer::observe(Phase::Identity, identity_future);
        let identity = identity_future
            .await
            .map_err(|e| security_response(e, PolicySource::Session))?;
        let binding = CsrfBinding::from_identity(&identity, &owner.lease.lease)
            .map_err(|e| security_response(e, PolicySource::Session))?;
        let _scope = AuthScope::bind(identity, Arc::clone(&owner.lease.lease))
            .map_err(|e| security_response(e, PolicySource::Session))?;
        if head {
            return Ok((binding, None));
        }
        let issue_future = store.issue(&owner, &binding);
        #[cfg(nagi_auth_stage_diag)]
        let issue_future = deadline_observer::observe(Phase::Issue, issue_future);
        issue_future
            .await
            .map_err(|e| text(e.http_status(), csrf::message(&e)))?;
        #[cfg(nagi_auth_stage_diag)]
        deadline_observer::phase(Phase::Seal);
        let seal = owner
            .seal()
            .map_err(|e| text(e.http_status(), csrf::message(&e)))?;
        Ok::<_, Response>((binding, Some(seal)))
    };
    let result = tokio::time::timeout_at(
        tokio::time::Instant::from_std(until),
        AssertUnwindSafe(future).catch_unwind(),
    )
    .await;
    match result {
        Ok(_) if Instant::now() >= until => {
            #[cfg(nagi_auth_stage_diag)]
            deadline_observer::exit(Exit::LateReady);
            #[cfg(nagi_auth_stage_diag)]
            deadline_observer::token_terminal(TokenTerminal::LateCompletion);
            token_terminal(&owner, Status::GATEWAY_TIMEOUT, head)
        }
        Ok(Ok(Ok((binding, seal)))) => {
            if let Err(status) = live_until(&owner.lease, until) {
                #[cfg(nagi_auth_stage_diag)]
                deadline_observer::exit(Exit::Gate);
                #[cfg(nagi_auth_stage_diag)]
                deadline_observer::token_terminal(TokenTerminal::LiveGate);
                return token_terminal(&owner, status, head);
            }
            let body = if let Some(seal) = seal {
                if head {
                    return token_terminal(&owner, Status::INTERNAL_SERVER_ERROR, true);
                }
                #[cfg(nagi_auth_stage_diag)]
                deadline_observer::phase(Phase::Wire);
                match store.take_wire(&owner, seal, &binding) {
                    Ok(body) => body,
                    Err(e) => {
                        #[cfg(nagi_auth_stage_diag)]
                        deadline_observer::exit(Exit::Wire);
                        #[cfg(nagi_auth_stage_diag)]
                        deadline_observer::token_terminal(TokenTerminal::TakeWire);
                        return token_terminal(&owner, e.http_status(), false);
                    }
                }
            } else {
                Bytes::new()
            };
            let mut response = empty(Status::OK).into_http(head);
            response.body_mut().bytes = body;
            response.headers_mut().insert(
                names::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            response
                .headers_mut()
                .insert(names::CONTENT_LENGTH, HeaderValue::from_static("60"));
            response.headers_mut().insert(
                names::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            owner.retire();
            managed(response, head, false)
        }
        Ok(Ok(Err(response))) => {
            #[cfg(nagi_auth_stage_diag)]
            deadline_observer::token_terminal(TokenTerminal::InnerRefusal);
            owner.retire();
            managed(response.into_http(head), head, false)
        }
        Ok(Err(_)) => token_terminal(&owner, Status::INTERNAL_SERVER_ERROR, head),
        Err(_) => {
            #[cfg(nagi_auth_stage_diag)]
            deadline_observer::exit(Exit::Timer);
            #[cfg(nagi_auth_stage_diag)]
            deadline_observer::token_terminal(TokenTerminal::Timer);
            token_terminal(&owner, Status::GATEWAY_TIMEOUT, head)
        }
    }
}
// Use the original common bound after preparation; body timing never renews it.
async fn receive_guarded_body<B>(
    body: B,
    maximum: usize,
    common: Instant,
    body_started: Instant,
    body_budget: Duration,
) -> Result<Bytes, Status>
where
    B: hyper::body::Body<Data = Bytes> + Unpin,
{
    let deadline = select_body_deadline(common, body_started, body_budget)
        .ok_or(Status::SERVICE_UNAVAILABLE)?;
    let status = if deadline.body_first {
        Status::REQUEST_TIMEOUT
    } else {
        Status::GATEWAY_TIMEOUT
    };
    match tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline.until),
        receive_body(body, maximum),
    )
    .await
    {
        Ok(_) if Instant::now() >= deadline.until => Err(status),
        Ok(Ok(body)) => Ok(body),
        Ok(Err(BodyFailure::TooLarge)) => Err(Status::CONTENT_TOO_LARGE),
        Ok(Err(BodyFailure::Invalid)) => Err(Status::BAD_REQUEST),
        Err(_) => Err(status),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "closed guarded dispatch retains the actual route, body, options and request owner"
)]
pub(super) async fn dispatch_guarded<S: Send + Sync + 'static>(
    store: &csrf::Inner,
    prepare: &GuardedPrepare<S>,
    state: Arc<S>,
    parts: axum::http::request::Parts,
    body: hyper::body::Incoming,
    options: &Options,
    owner: LeaseOwner,
    head: bool,
    admitted: Instant,
) -> axum::http::Response<BufferedBody> {
    let Some(until) = admitted.checked_add(options.security_deadline) else {
        return terminal(&owner, Status::SERVICE_UNAVAILABLE, head, true);
    };
    if let Err(e) = csrf::constrain(&owner.lease, until) {
        return terminal(&owner, e.http_status(), head, true);
    }
    if let Err(status) = options
        .authority
        .as_ref()
        .ok_or(Status::INTERNAL_SERVER_ERROR)
        .and_then(|a| a.check_unsafe_origin(&parts.headers))
    {
        return terminal(&owner, status, head, true);
    }
    let digest = match csrf::token_digest(&parts.headers) {
        Ok(d) => d,
        Err(e) => return terminal(&owner, e.http_status(), head, true),
    };
    let snapshot = Request {
        method: parts.method.clone(),
        uri: parts.uri.clone(),
        headers: parts.headers.clone(),
        body: Bytes::new(),
    };
    let future = AssertUnwindSafe(async {
        prepare(snapshot, Arc::clone(&state), Arc::clone(&owner.lease)).await
    })
    .catch_unwind();
    let (prepared, binding) = match await_preparation(&owner, until, future).await {
        Ok(Ok(Ok(v))) => v,
        Ok(Ok(Err(e))) => {
            return managed(
                security_response(e, PolicySource::Session).into_http(head),
                head,
                true,
            );
        }
        Ok(Err(_)) => return terminal(&owner, Status::INTERNAL_SERVER_ERROR, head, true),
        Err(status) => return terminal(&owner, status, head, true),
    };
    let common = match live_until(&owner, until) {
        Ok(d) => d,
        Err(s) => return terminal(&owner, s, head, true),
    };
    let body = match receive_guarded_body(
        body,
        options.body_bytes,
        common,
        Instant::now(),
        options.body_deadline,
    )
    .await
    {
        Ok(body) => body,
        Err(status) => return terminal(&owner, status, head, true),
    };
    let common = match live_until(&owner, until) {
        Ok(d) => d,
        Err(s) => return terminal(&owner, s, head, true),
    };
    let checkpoint = tokio::time::timeout_at(
        tokio::time::Instant::from_std(common),
        AssertUnwindSafe(store.checkpoint(&owner.lease, &binding, digest)).catch_unwind(),
    )
    .await;
    match checkpoint {
        Ok(_) if Instant::now() >= common => {
            return terminal(&owner, Status::GATEWAY_TIMEOUT, head, true);
        }
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(e))) => return terminal(&owner, e.http_status(), head, true),
        Ok(Err(_)) => return terminal(&owner, Status::INTERNAL_SERVER_ERROR, head, true),
        Err(_) => return terminal(&owner, Status::GATEWAY_TIMEOUT, head, true),
    }
    let common = match live_until(&owner, until) {
        Ok(d) => d,
        Err(s) => return terminal(&owner, s, head, true),
    };
    let mut headers = parts.headers;
    strip_credentials(&mut headers);
    let connect = parts.method == Method::CONNECT;
    let request = Request {
        method: parts.method,
        uri: parts.uri,
        headers,
        body,
    };
    let handler_until = Instant::now()
        .checked_add(options.handler_deadline)
        .map(|d| d.min(common))
        .unwrap_or(common);
    let completion = tokio::time::timeout_at(
        tokio::time::Instant::from_std(handler_until),
        AssertUnwindSafe(async { prepared(request, state).await }).catch_unwind(),
    )
    .await;
    let response = match completion {
        Ok(_) if Instant::now() >= handler_until || owner.lease.validate().is_err() => {
            return terminal(&owner, Status::GATEWAY_TIMEOUT, head, true);
        }
        Ok(Ok(completion)) if connect && completion.is_success() => {
            return terminal(&owner, Status::NOT_IMPLEMENTED, head, true);
        }
        Ok(Ok(PreparedCompletion::HandlerOk(response))) => {
            finalize_session_response(response, &owner, PolicySource::Session, head)
        }
        Ok(Ok(PreparedCompletion::Replacement(response))) => {
            crate::auth::session::retire_http_delivery(&owner);
            response.into_http(head)
        }
        Ok(Err(_)) => return terminal(&owner, Status::INTERNAL_SERVER_ERROR, head, true),
        Err(_) => return terminal(&owner, Status::GATEWAY_TIMEOUT, head, true),
    };
    crate::auth::session::retire_http_delivery(&owner);
    managed(response, head, true)
}

#[cfg(test)]
#[path = "csrf_deadline_tests.rs"]
mod deadline_tests;
