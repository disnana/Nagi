//! Body-first BrowserLogin dispatch. Actual owner stays here; callbacks receive
//! only sanitized Request, application state and dispatcher-bound AuthScope.
use super::*;
#[cfg(nagi_browser_login_diag)]
use crate::auth::browser_login_observer::{self as diag, Exit};
#[cfg(nagi_browser_login_diag)]
use crate::auth::browser_observe;
use crate::auth::prelogin::{self, HttpOwner, Pool};

fn finish(
    mut wire: axum::http::Response<BufferedBody>,
    pool: &Pool,
    owner: &HttpOwner,
) -> axum::http::Response<BufferedBody> {
    wire = finalize_authenticated(wire, PolicySource::BrowserLogin, false);
    if owner.was_consumed() {
        match pool.deletion_cookie() {
            Ok(cookie) => wire.headers_mut().append(names::SET_COOKIE, cookie),
            Err(_) => {
                crate::auth::session::retire_http_delivery(owner.lease_owner());
                wire = finalize_authenticated(
                    transport(Status::INTERNAL_SERVER_ERROR, false, true),
                    PolicySource::BrowserLogin,
                    false,
                );
                false
            }
        };
    }
    #[cfg(nagi_browser_login_diag)]
    diag::emit(wire.status().as_u16(), owner.was_consumed());
    wire
}
fn terminal(status: Status, pool: &Pool, owner: &HttpOwner) -> axum::http::Response<BufferedBody> {
    crate::auth::session::retire_http_delivery(owner.lease_owner());
    finish(transport(status, false, true), pool, owner)
}
fn failure(
    error: prelogin::Failure,
    pool: &Pool,
    owner: &HttpOwner,
) -> axum::http::Response<BufferedBody> {
    let challenge = error.login_challenge();
    let mut wire = terminal(error.login_status(), pool, owner);
    if let Some(value) = challenge {
        wire.headers_mut().insert(names::WWW_AUTHENTICATE, value);
    }
    wire
}
// Keep the stock dispatcher body and owning locals exactly as before diagnostics.
// Observation scopes belong only to the explicit diagnostic build below.
#[cfg(not(nagi_browser_login_diag))]
pub(super) async fn dispatch_browser<S>(
    pool: &Pool,
    prepare: &BrowserPrepare<S>,
    state: Arc<S>,
    request: axum::http::Request<hyper::body::Incoming>,
    options: &Options,
    lease: LeaseOwner,
    admitted: Instant,
) -> axum::http::Response<BufferedBody>
where
    S: Send + Sync + 'static,
{
    let (mut parts, body) = request.into_parts();
    let owner = HttpOwner::new(lease);
    let Some(security_until) = admitted.checked_add(options.security_deadline) else {
        return terminal(Status::SERVICE_UNAVAILABLE, pool, &owner);
    };
    if let Err(error) = owner.constrain_until(security_until) {
        return failure(error, pool, &owner);
    }
    let input = match pool.login_input(&parts.headers) {
        Ok(input) => input,
        Err(error) => return failure(error, pool, &owner),
    };
    // Remove client prelogin material before constructing any callback Request
    // and before receiving body across await. Only digests remain in ClaimInput.
    parts.headers.remove(names::COOKIE);
    parts.headers.remove("x-csrf-token");
    let mut request = Request {
        method: parts.method,
        uri: parts.uri,
        headers: parts.headers,
        body: Bytes::new(),
    };
    if !matches!(is_json_content_type(&request), Ok(true)) {
        return terminal(Status::BAD_REQUEST, pool, &owner);
    }
    let Some(body_until) = admitted.checked_add(options.body_deadline) else {
        return terminal(Status::SERVICE_UNAVAILABLE, pool, &owner);
    };
    let until = body_until.min(security_until);
    let body_status = if security_until <= body_until {
        Status::GATEWAY_TIMEOUT
    } else {
        Status::REQUEST_TIMEOUT
    };
    request.body = match tokio::time::timeout_at(
        tokio::time::Instant::from_std(until),
        receive_body(body, options.body_bytes),
    )
    .await
    {
        Ok(_) if Instant::now() >= until => return terminal(body_status, pool, &owner),
        Ok(Ok(body)) => body,
        Ok(Err(BodyFailure::TooLarge)) => return terminal(Status::CONTENT_TOO_LARGE, pool, &owner),
        Ok(Err(BodyFailure::Invalid)) => return terminal(Status::BAD_REQUEST, pool, &owner),
        Err(_) => return terminal(body_status, pool, &owner),
    };
    if let Err(error) = pool.login_json(&request.body) {
        return failure(error, pool, &owner);
    }
    let claim = AssertUnwindSafe(pool.claim_http(&owner, input)).catch_unwind();
    let context = match tokio::time::timeout_at(
        tokio::time::Instant::from_std(security_until),
        claim,
    )
    .await
    {
        Ok(_) if Instant::now() >= security_until => {
            return terminal(Status::GATEWAY_TIMEOUT, pool, &owner)
        }
        Ok(Ok(Ok(context))) => context,
        Ok(Ok(Err(error))) => return failure(error, pool, &owner),
        Ok(Err(_)) => return terminal(Status::INTERNAL_SERVER_ERROR, pool, &owner),
        Err(_) => return terminal(Status::GATEWAY_TIMEOUT, pool, &owner),
    };
    // Claim tightened the actual lease by the original prelogin expiry. This
    // is the same admission budget; it is not a new verifier timeout allowance.
    let until = match owner.request_until() {
        Ok(until) => until.min(security_until),
        Err(error) => return failure(error, pool, &owner),
    };
    let future = AssertUnwindSafe(prepare(
        request.security_snapshot(),
        Arc::clone(&state),
        context,
    ))
    .catch_unwind();
    let prepared = match tokio::time::timeout_at(tokio::time::Instant::from_std(until), future)
        .await
    {
        Ok(_) if Instant::now() >= until => return terminal(Status::GATEWAY_TIMEOUT, pool, &owner),
        Ok(Ok(Ok(prepared))) => prepared,
        Ok(Ok(Err(error))) => return failure(error, pool, &owner),
        Ok(Err(_)) => return terminal(Status::INTERNAL_SERVER_ERROR, pool, &owner),
        Err(_) => return terminal(Status::GATEWAY_TIMEOUT, pool, &owner),
    };
    let Some(handler_until) = Instant::now().checked_add(options.handler_deadline) else {
        return terminal(Status::SERVICE_UNAVAILABLE, pool, &owner);
    };
    let until = match owner.request_until() {
        Ok(until) => until.min(handler_until).min(security_until),
        Err(error) => return failure(error, pool, &owner),
    };
    // Construction, polling and mapper run within one unwind/absolute boundary.
    let future = AssertUnwindSafe(async { prepared(request, state).await }).catch_unwind();
    match tokio::time::timeout_at(tokio::time::Instant::from_std(until), future).await {
        Ok(_) if Instant::now() >= until => terminal(Status::GATEWAY_TIMEOUT, pool, &owner),
        Ok(Ok(PreparedCompletion::HandlerOk(response))) => {
            let wire = finalize_session_response(
                response,
                owner.lease_owner(),
                PolicySource::BrowserLogin,
                false,
            );
            finish(wire, pool, &owner)
        }
        Ok(Ok(PreparedCompletion::Replacement(response))) => {
            crate::auth::session::retire_http_delivery(owner.lease_owner());
            finish(response.into_http(false), pool, &owner)
        }
        Ok(Err(_)) => terminal(Status::INTERNAL_SERVER_ERROR, pool, &owner),
        Err(_) => terminal(Status::GATEWAY_TIMEOUT, pool, &owner),
    }
}

#[cfg(nagi_browser_login_diag)]
pub(super) async fn dispatch_browser<S>(
    pool: &Pool,
    prepare: &BrowserPrepare<S>,
    state: Arc<S>,
    request: axum::http::Request<hyper::body::Incoming>,
    options: &Options,
    lease: LeaseOwner,
    admitted: Instant,
) -> axum::http::Response<BufferedBody>
where
    S: Send + Sync + 'static,
{
    let (mut parts, body) = request.into_parts();
    let owner = HttpOwner::new(lease);
    #[cfg(nagi_browser_login_diag)]
    let observer = diag::Observer::new(&owner.lease_owner().lease);
    let dispatch = async {
        let Some(security_until) = admitted.checked_add(options.security_deadline) else {
            return terminal(Status::SERVICE_UNAVAILABLE, pool, &owner);
        };
        if let Err(error) = owner.constrain_until(security_until) {
            return failure(error, pool, &owner);
        }
        let input = match pool.login_input(&parts.headers) {
            Ok(input) => input,
            Err(error) => return failure(error, pool, &owner),
        };
        // Remove client prelogin material before constructing any callback Request
        // and before receiving body across await. Only digests remain in ClaimInput.
        parts.headers.remove(names::COOKIE);
        parts.headers.remove("x-csrf-token");
        let mut request = Request {
            method: parts.method,
            uri: parts.uri,
            headers: parts.headers,
            body: Bytes::new(),
        };
        if !matches!(is_json_content_type(&request), Ok(true)) {
            return terminal(Status::BAD_REQUEST, pool, &owner);
        }
        let Some(body_until) = admitted.checked_add(options.body_deadline) else {
            return terminal(Status::SERVICE_UNAVAILABLE, pool, &owner);
        };
        let until = body_until.min(security_until);
        let body_status = if security_until <= body_until {
            Status::GATEWAY_TIMEOUT
        } else {
            Status::REQUEST_TIMEOUT
        };
        request.body = match tokio::time::timeout_at(tokio::time::Instant::from_std(until), async {
            browser_observe!(Body, receive_body(body, options.body_bytes))
        })
        .await
        {
            Ok(_) if Instant::now() >= until => {
                #[cfg(nagi_browser_login_diag)]
                diag::exit(Exit::BodyLateReady);
                return terminal(body_status, pool, &owner);
            }
            Ok(Ok(body)) => body,
            Ok(Err(BodyFailure::TooLarge)) => {
                return terminal(Status::CONTENT_TOO_LARGE, pool, &owner)
            }
            Ok(Err(BodyFailure::Invalid)) => return terminal(Status::BAD_REQUEST, pool, &owner),
            Err(_) => {
                #[cfg(nagi_browser_login_diag)]
                diag::exit(Exit::BodyTimer);
                return terminal(body_status, pool, &owner);
            }
        };
        if let Err(error) = pool.login_json(&request.body) {
            return failure(error, pool, &owner);
        }
        let claim = AssertUnwindSafe(pool.claim_http(&owner, input)).catch_unwind();
        let context =
            match tokio::time::timeout_at(tokio::time::Instant::from_std(security_until), claim)
                .await
            {
                Ok(_) if Instant::now() >= security_until => {
                    #[cfg(nagi_browser_login_diag)]
                    diag::exit(Exit::ClaimLateReady);
                    return terminal(Status::GATEWAY_TIMEOUT, pool, &owner);
                }
                Ok(Ok(Ok(context))) => context,
                Ok(Ok(Err(error))) => return failure(error, pool, &owner),
                Ok(Err(_)) => return terminal(Status::INTERNAL_SERVER_ERROR, pool, &owner),
                Err(_) => {
                    #[cfg(nagi_browser_login_diag)]
                    diag::exit(Exit::ClaimTimer);
                    return terminal(Status::GATEWAY_TIMEOUT, pool, &owner);
                }
            };
        // Claim tightened the actual lease by the original prelogin expiry. This
        // is the same admission budget; it is not a new verifier timeout allowance.
        let until = match owner.request_until() {
            Ok(until) => until.min(security_until),
            Err(error) => return failure(error, pool, &owner),
        };
        let future = AssertUnwindSafe(prepare(
            request.security_snapshot(),
            Arc::clone(&state),
            context,
        ))
        .catch_unwind();
        let prepared = match tokio::time::timeout_at(tokio::time::Instant::from_std(until), async {
            browser_observe!(Prepare, future)
        })
        .await
        {
            Ok(_) if Instant::now() >= until => {
                #[cfg(nagi_browser_login_diag)]
                diag::exit(Exit::PrepareLateReady);
                return terminal(Status::GATEWAY_TIMEOUT, pool, &owner);
            }
            Ok(Ok(Ok(prepared))) => prepared,
            Ok(Ok(Err(error))) => return failure(error, pool, &owner),
            Ok(Err(_)) => return terminal(Status::INTERNAL_SERVER_ERROR, pool, &owner),
            Err(_) => {
                #[cfg(nagi_browser_login_diag)]
                diag::exit(Exit::PrepareTimer);
                return terminal(Status::GATEWAY_TIMEOUT, pool, &owner);
            }
        };
        let Some(handler_until) = Instant::now().checked_add(options.handler_deadline) else {
            return terminal(Status::SERVICE_UNAVAILABLE, pool, &owner);
        };
        let until = match owner.request_until() {
            Ok(until) => until.min(handler_until).min(security_until),
            Err(error) => return failure(error, pool, &owner),
        };
        // Construction, polling and mapper run within one unwind/absolute boundary.
        let future = AssertUnwindSafe(async { prepared(request, state).await }).catch_unwind();
        match tokio::time::timeout_at(tokio::time::Instant::from_std(until), async {
            browser_observe!(Handler, future)
        })
        .await
        {
            Ok(_) if Instant::now() >= until => {
                #[cfg(nagi_browser_login_diag)]
                diag::exit(Exit::HandlerLateReady);
                terminal(Status::GATEWAY_TIMEOUT, pool, &owner)
            }
            Ok(Ok(PreparedCompletion::HandlerOk(response))) => {
                let wire = finalize_session_response(
                    response,
                    owner.lease_owner(),
                    PolicySource::BrowserLogin,
                    false,
                );
                finish(wire, pool, &owner)
            }
            Ok(Ok(PreparedCompletion::Replacement(response))) => {
                crate::auth::session::retire_http_delivery(owner.lease_owner());
                finish(response.into_http(false), pool, &owner)
            }
            Ok(Err(_)) => terminal(Status::INTERNAL_SERVER_ERROR, pool, &owner),
            Err(_) => {
                #[cfg(nagi_browser_login_diag)]
                diag::exit(Exit::HandlerTimer);
                terminal(Status::GATEWAY_TIMEOUT, pool, &owner)
            }
        }
    };
    #[cfg(nagi_browser_login_diag)]
    {
        observer.scope(dispatch).await
    }
    #[cfg(not(nagi_browser_login_diag))]
    {
        dispatch.await
    }
}
