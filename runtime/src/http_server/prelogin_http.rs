//! Closed GET/explicitHEAD profile; fixed successful JSON bypasses public payloads.
use super::*;
use crate::auth::prelogin::{HttpOwner, HttpSeal, Pool};
#[cfg(nagi_auth_stage_diag)]
use crate::auth::session_deadline_observer::{self as diag, Exit, Observer, Phase};

#[cfg_attr(
    test,
    allow(
        clippy::large_enum_variant,
        reason = "Test-only inline replacement preserves the production no-allocation completion."
    )
)]
pub(super) enum PreloginCompletion {
    Ready(HttpSeal),
    #[cfg(test)]
    Replacement(Option<HttpSeal>, Response),
}
pub(super) fn register_prelogin_bootstrap<S, E>(
    mut app: App<S, E>,
    path: &str,
    pool: Arc<Pool>,
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
            .any(|route| route.method == Method::GET || route.method == Method::HEAD)
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
            rate: None,
            rate_index: None,
            cors: None,
            source: PolicySource::Prelogin,
            method,
            kind: RouteKind::Bootstrap(Arc::clone(&pool)),
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
        if let RouteKind::Bootstrap(pool) | RouteKind::Browser { pool, .. } = &route.kind {
            if !pool.bound_to(origin) {
                return Err(Error::invalid(
                    "pre-login origin does not match checked startup authority",
                ));
            }
        }
    }
    Ok(())
}
fn terminal(
    context: &ResponseContext,
    _pool: &Pool,
    owner: &HttpOwner,
    status: Status,
    head: bool,
) -> HttpResponse {
    owner.retire();
    finalize_authenticated(
        transport(context, status, head, true),
        PolicySource::Prelogin,
        head,
    )
}
pub(super) fn finalize_prelogin(
    context: &ResponseContext,
    completion: PreloginCompletion,
    owner: &HttpOwner,
    head: bool,
    pool: &Pool,
) -> HttpResponse {
    match completion {
        #[cfg(test)]
        PreloginCompletion::Replacement(seal, response) => {
            drop(seal);
            owner.retire();
            finalize_authenticated(response.into_http(head), PolicySource::Prelogin, head)
        }
        PreloginCompletion::Ready(seal) => {
            if head {
                drop(seal);
                return terminal(context, pool, owner, Status::INTERNAL_SERVER_ERROR, head);
            }
            #[cfg(nagi_auth_stage_diag)]
            diag::phase(Phase::Delivery);
            use response_budget::finalize::{FrameworkBody, FrameworkError};
            let prepared = context
                .framework(FrameworkBody::Prelogin, Some(512))
                .and_then(|ticket| ticket.take_prelogin(pool, owner, seal))
                .and_then(|prepared| prepared.finish().map_err(FrameworkError::Budget));
            match prepared {
                Ok(wire) => finalize_authenticated(
                    HttpResponse::from_wire(StatusCode::OK, wire, Arc::clone(&context.fallback)),
                    PolicySource::Prelogin,
                    false,
                ),
                Err(error) => {
                    #[cfg(nagi_auth_stage_diag)]
                    diag::exit(Exit::Wire);
                    owner.retire();
                    terminal(context, pool, owner, error.status(), head)
                }
            }
        }
    }
}
pub(super) async fn dispatch_bootstrap(
    pool: &Pool,
    parts: RequestParts,
    body: hyper::body::Incoming,
    options: &Options,
    lease: LeaseOwner,
    head: bool,
    admitted: Instant,
) -> HttpResponse {
    #[cfg(nagi_auth_stage_diag)]
    if let Some(until) = admitted.checked_add(options.security_deadline) {
        let observer = Observer::new(admitted, until);
        let response = observer
            .scope(async {
                diag::phase(Phase::Bootstrap);
                dispatch_bootstrap_inner(pool, parts, body, options, lease, head, admitted).await
            })
            .await;
        observer.emit_bootstrap(response.status().as_u16());
        return response;
    }
    dispatch_bootstrap_inner(pool, parts, body, options, lease, head, admitted).await
}
async fn dispatch_bootstrap_inner(
    pool: &Pool,
    parts: RequestParts,
    body: hyper::body::Incoming,
    options: &Options,
    lease: LeaseOwner,
    head: bool,
    admitted: Instant,
) -> HttpResponse {
    let context = parts.headers.response_context();
    let owner = HttpOwner::new(lease);
    let Some(until) = admitted.checked_add(options.security_deadline) else {
        return terminal(context, pool, &owner, Status::SERVICE_UNAVAILABLE, head);
    };
    if let Err(failure) = owner.constrain_until(until) {
        #[cfg(nagi_auth_stage_diag)]
        diag::exit(Exit::Gate);
        return terminal(context, pool, &owner, failure.http_status(), head);
    }
    // Body input is not part of this fixed safe profile. Refuse before any
    // entropy/DB work; ordinary routes retain their existing receive_body path.
    if !hyper::body::Body::is_end_stream(&body)
        || parts.headers.contains_key(names::TRANSFER_ENCODING)
    {
        return terminal(context, pool, &owner, Status::BAD_REQUEST, head);
    }
    let Some(authority) = options.authority.as_ref() else {
        return terminal(context, pool, &owner, Status::INTERNAL_SERVER_ERROR, head);
    };
    if let Err(status) = authority.check_safe_origin(parts.headers.authentication_headers()) {
        return terminal(context, pool, &owner, status, head);
    }
    let old = match pool.previous_cookie(parts.headers.authentication_headers()) {
        Ok(old) => old,
        Err(failure) => return terminal(context, pool, &owner, failure.http_status(), head),
    };
    if head {
        // Corresponding fixed GET representation has canonical43ASCII +17B.
        // No entropy, native session/table query, mutation, cookie or token.
        let mut response = match empty(context, Status::OK) {
            Ok(response) => response.into_http(true),
            Err(_) => return terminal(context, pool, &owner, Status::SERVICE_UNAVAILABLE, true),
        };
        response.headers_mut().insert(
            names::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
            .headers_mut()
            .insert(names::CONTENT_LENGTH, HeaderValue::from_static("60"));
        owner.retire();
        return finalize_authenticated(response, PolicySource::Prelogin, true);
    }
    let future = async {
        let work = pool.bootstrap_http(&owner, old);
        #[cfg(nagi_auth_stage_diag)]
        let work = diag::observe(Phase::Bootstrap, work);
        work.await?;
        #[cfg(nagi_auth_stage_diag)]
        diag::phase(Phase::Delivery);
        owner.seal()
    };
    let Some(handler_until) = Instant::now().checked_add(options.handler_deadline) else {
        return terminal(context, pool, &owner, Status::SERVICE_UNAVAILABLE, false);
    };
    complete_bootstrap(context, pool, &owner, handler_until.min(until), future).await
}

// One absolute timer and post-poll check, including a finite non-yielding Ready.
pub(super) async fn complete_bootstrap(
    context: &ResponseContext,
    pool: &Pool,
    owner: &HttpOwner,
    until: Instant,
    future: impl std::future::Future<Output = Result<HttpSeal, crate::auth::prelogin::Failure>>,
) -> HttpResponse {
    #[cfg(nagi_auth_stage_diag)]
    diag::select_until(until);
    let result = tokio::time::timeout_at(
        tokio::time::Instant::from_std(until),
        AssertUnwindSafe(future).catch_unwind(),
    )
    .await;
    match result {
        Ok(_) if Instant::now() >= until => {
            #[cfg(nagi_auth_stage_diag)]
            diag::exit(Exit::LateReady);
            terminal(context, pool, owner, Status::GATEWAY_TIMEOUT, false)
        }
        Ok(Ok(Ok(seal))) => {
            finalize_prelogin(context, PreloginCompletion::Ready(seal), owner, false, pool)
        }
        Ok(Ok(Err(failure))) => {
            #[cfg(nagi_auth_stage_diag)]
            if crate::auth::prelogin::kind(&failure) == crate::auth::FailureKind::Expired {
                diag::exit(Exit::Gate);
            }
            terminal(context, pool, owner, failure.http_status(), false)
        }
        Ok(Err(_)) => terminal(context, pool, owner, Status::INTERNAL_SERVER_ERROR, false),
        Err(_) => {
            #[cfg(nagi_auth_stage_diag)]
            diag::exit(Exit::Timer);
            terminal(context, pool, owner, Status::GATEWAY_TIMEOUT, false)
        }
    }
}
