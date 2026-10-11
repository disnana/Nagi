//! Closed GET/explicitHEAD profile; fixed successful JSON bypasses public payloads.
use super::*;
use crate::auth::prelogin::{HttpOwner, HttpSeal, Pool};

pub(super) enum PreloginCompletion {
    Ready(HttpSeal),
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
    pool: &Pool,
    owner: &HttpOwner,
    status: Status,
    head: bool,
) -> axum::http::Response<BufferedBody> {
    let mut response = text(status, "pre-login request failed");
    response
        .headers
        .insert(names::CONNECTION, HeaderValue::from_static("close"));
    finalize_prelogin(
        PreloginCompletion::Replacement(None, response),
        owner,
        head,
        pool,
    )
}
pub(super) fn finalize_prelogin(
    completion: PreloginCompletion,
    owner: &HttpOwner,
    head: bool,
    pool: &Pool,
) -> axum::http::Response<BufferedBody> {
    match completion {
        PreloginCompletion::Replacement(seal, response) => {
            drop(seal);
            owner.retire();
            finalize_authenticated(response.into_http(head), PolicySource::Prelogin, head)
        }
        PreloginCompletion::Ready(seal) => {
            if head {
                drop(seal);
                return terminal(pool, owner, Status::INTERNAL_SERVER_ERROR, head);
            }
            let material = match pool.take_wire(owner, seal) {
                Ok(material) => material,
                Err(failure) => return terminal(pool, owner, failure.http_status(), head),
            };
            let mut response = empty(Status::OK).into_http(false);
            response.headers_mut().insert(
                names::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            response
                .headers_mut()
                .insert(names::SET_COOKIE, material.cookie);
            response.body_mut().bytes = material.body;
            finalize_authenticated(response, PolicySource::Prelogin, false)
        }
    }
}
pub(super) async fn dispatch_bootstrap(
    pool: &Pool,
    parts: axum::http::request::Parts,
    body: hyper::body::Incoming,
    options: &Options,
    lease: LeaseOwner,
    head: bool,
    admitted: Instant,
) -> axum::http::Response<BufferedBody> {
    let owner = HttpOwner::new(lease);
    let Some(until) = admitted.checked_add(options.security_deadline) else {
        return terminal(pool, &owner, Status::SERVICE_UNAVAILABLE, head);
    };
    if let Err(failure) = owner.constrain_until(until) {
        return terminal(pool, &owner, failure.http_status(), head);
    }
    // Body input is not part of this fixed safe profile. Refuse before any
    // entropy/DB work; ordinary routes retain their existing receive_body path.
    if !hyper::body::Body::is_end_stream(&body)
        || parts.headers.contains_key(names::TRANSFER_ENCODING)
    {
        return terminal(pool, &owner, Status::BAD_REQUEST, head);
    }
    let Some(authority) = options.authority.as_ref() else {
        return terminal(pool, &owner, Status::INTERNAL_SERVER_ERROR, head);
    };
    if let Err(status) = authority.check_safe_origin(&parts.headers) {
        return terminal(pool, &owner, status, head);
    }
    let old = match pool.previous_cookie(&parts.headers) {
        Ok(old) => old,
        Err(failure) => return terminal(pool, &owner, failure.http_status(), head),
    };
    if head {
        // Corresponding fixed GET representation has canonical43ASCII +17B.
        // No entropy, native session/table query, mutation, cookie or token.
        let mut response = empty(Status::OK).into_http(true);
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
        pool.bootstrap_http(&owner, old).await?;
        owner.seal()
    };
    let Some(handler_until) = Instant::now().checked_add(options.handler_deadline) else {
        return terminal(pool, &owner, Status::SERVICE_UNAVAILABLE, false);
    };
    complete_bootstrap(pool, &owner, handler_until.min(until), future).await
}

// One absolute timer and post-poll check, including a finite non-yielding Ready.
pub(super) async fn complete_bootstrap(
    pool: &Pool,
    owner: &HttpOwner,
    until: Instant,
    future: impl std::future::Future<Output = Result<HttpSeal, crate::auth::prelogin::Failure>>,
) -> axum::http::Response<BufferedBody> {
    let result = tokio::time::timeout_at(
        tokio::time::Instant::from_std(until),
        AssertUnwindSafe(future).catch_unwind(),
    )
    .await;
    match result {
        Ok(_) if Instant::now() >= until => terminal(pool, owner, Status::GATEWAY_TIMEOUT, false),
        Ok(Ok(Ok(seal))) => finalize_prelogin(PreloginCompletion::Ready(seal), owner, false, pool),
        Ok(Ok(Err(failure))) => terminal(pool, owner, failure.http_status(), false),
        Ok(Err(_)) => terminal(pool, owner, Status::INTERNAL_SERVER_ERROR, false),
        Err(_) => terminal(pool, owner, Status::GATEWAY_TIMEOUT, false),
    }
}
