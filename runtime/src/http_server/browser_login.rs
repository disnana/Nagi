//! Body-first BrowserLogin dispatch. Actual owner stays here; callbacks receive
//! only sanitized Request, application state and dispatcher-bound AuthScope.
use super::*;
use crate::auth::prelogin::{self, ClaimContext, HttpOwner, Pool};

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
// The stock wrapper retains the original native remainder and actual owner.
// Test continuations carry only real one-shot claim/handler values.
#[cfg_attr(
    test,
    allow(
        clippy::large_enum_variant,
        reason = "Test-only continuation must not allocate the stock borrowed wire input."
    )
)]
enum Input<'a, B> {
    Wire {
        body: &'a mut Option<B>,
        parts: &'a mut axum::http::request::Parts,
    },
    #[cfg(test)]
    Claimed {
        request: Request,
        context: ClaimContext,
    },
    #[cfg(test)]
    Handler(ResponseFuture),
}
#[cfg_attr(
    test,
    allow(
        clippy::large_enum_variant,
        reason = "The smaller test-only continuation must not allocate the stock owned request."
    )
)]
enum Stage {
    Claimed {
        request: Request,
        context: ClaimContext,
    },
    #[cfg(test)]
    Handler(ResponseFuture),
}
#[cfg_attr(
    test,
    allow(
        clippy::large_enum_variant,
        reason = "The smaller test-only continuation must not allocate the stock owned request."
    )
)]
enum HandlerStep<S> {
    Prepared {
        request: Request,
        prepared: PreparedHandler<S>,
    },
    #[cfg(test)]
    Running(ResponseFuture),
}

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
    let mut body = Some(body);
    let owner = HttpOwner::new(lease);
    // Keep native remainder/unused body alive past actual owner Drop, exactly
    // as the original wrapper. The shared core only borrows these one-shot parts.
    dispatch_owned(
        pool,
        prepare,
        state,
        Input::Wire {
            parts: &mut parts,
            body: &mut body,
        },
        options,
        &owner,
        admitted,
    )
    .await
}

async fn dispatch_owned<S, B>(
    pool: &Pool,
    prepare: &BrowserPrepare<S>,
    state: Arc<S>,
    input: Input<'_, B>,
    options: &Options,
    owner: &HttpOwner,
    admitted: Instant,
) -> axum::http::Response<BufferedBody>
where
    S: Send + Sync + 'static,
    B: hyper::body::Body<Data = Bytes> + Unpin,
{
    let Some(security_until) = admitted.checked_add(options.security_deadline) else {
        return terminal(Status::SERVICE_UNAVAILABLE, pool, owner);
    };
    if let Err(error) = owner.constrain_until(security_until) {
        return failure(error, pool, owner);
    }
    let stage = match input {
        Input::Wire { parts, body } => {
            let input = match pool.login_input(&parts.headers) {
                Ok(input) => input,
                Err(error) => return failure(error, pool, owner),
            };
            // Remove client prelogin material before constructing any callback Request
            // and before receiving body across await. Only digests remain in ClaimInput.
            parts.headers.remove(names::COOKIE);
            parts.headers.remove("x-csrf-token");
            let mut request = Request {
                method: std::mem::take(&mut parts.method),
                uri: std::mem::take(&mut parts.uri),
                headers: std::mem::take(&mut parts.headers),
                body: Bytes::new(),
            };
            if !matches!(is_json_content_type(&request), Ok(true)) {
                return terminal(Status::BAD_REQUEST, pool, owner);
            }
            let Some(body_until) = admitted.checked_add(options.body_deadline) else {
                return terminal(Status::SERVICE_UNAVAILABLE, pool, owner);
            };
            let until = body_until.min(security_until);
            let body_status = if security_until <= body_until {
                Status::GATEWAY_TIMEOUT
            } else {
                Status::REQUEST_TIMEOUT
            };
            request.body = match tokio::time::timeout_at(
                tokio::time::Instant::from_std(until),
                receive_body(
                    body.take().expect("owned wire body present"),
                    options.body_bytes,
                ),
            )
            .await
            {
                Ok(_) if Instant::now() >= until => return terminal(body_status, pool, owner),
                Ok(Ok(body)) => body,
                Ok(Err(BodyFailure::TooLarge)) => {
                    return terminal(Status::CONTENT_TOO_LARGE, pool, owner)
                }
                Ok(Err(BodyFailure::Invalid)) => return terminal(Status::BAD_REQUEST, pool, owner),
                Err(_) => return terminal(body_status, pool, owner),
            };
            if let Err(error) = pool.login_json(&request.body) {
                return failure(error, pool, owner);
            }
            let claim = AssertUnwindSafe(pool.claim_http(owner, input)).catch_unwind();
            let context = match tokio::time::timeout_at(
                tokio::time::Instant::from_std(security_until),
                claim,
            )
            .await
            {
                Ok(_) if Instant::now() >= security_until => {
                    return terminal(Status::GATEWAY_TIMEOUT, pool, owner)
                }
                Ok(Ok(Ok(context))) => context,
                Ok(Ok(Err(error))) => return failure(error, pool, owner),
                Ok(Err(_)) => return terminal(Status::INTERNAL_SERVER_ERROR, pool, owner),
                Err(_) => return terminal(Status::GATEWAY_TIMEOUT, pool, owner),
            };
            Stage::Claimed { request, context }
        }
        #[cfg(test)]
        Input::Claimed { request, context } => {
            assert!(Arc::ptr_eq(&context.lease(), &owner.lease_owner().lease));
            assert!(owner.was_consumed());
            Stage::Claimed { request, context }
        }
        #[cfg(test)]
        Input::Handler(future) => {
            assert!(owner.was_consumed());
            Stage::Handler(future)
        }
    };
    let handler =
        match stage {
            Stage::Claimed { request, context } => {
                // Claim tightened the actual lease by the original prelogin expiry. This
                // is the same admission budget; it is not a new verifier timeout allowance.
                let until = match owner.request_until() {
                    Ok(until) => until.min(security_until),
                    Err(error) => return failure(error, pool, owner),
                };
                let future = AssertUnwindSafe(prepare(
                    request.security_snapshot(),
                    Arc::clone(&state),
                    context,
                ))
                .catch_unwind();
                let prepared =
                    match tokio::time::timeout_at(tokio::time::Instant::from_std(until), future)
                        .await
                    {
                        Ok(_) if Instant::now() >= until => {
                            return terminal(Status::GATEWAY_TIMEOUT, pool, owner)
                        }
                        Ok(Ok(Ok(prepared))) => prepared,
                        Ok(Ok(Err(error))) => return failure(error, pool, owner),
                        Ok(Err(_)) => return terminal(Status::INTERNAL_SERVER_ERROR, pool, owner),
                        Err(_) => return terminal(Status::GATEWAY_TIMEOUT, pool, owner),
                    };
                HandlerStep::Prepared { request, prepared }
            }
            #[cfg(test)]
            Stage::Handler(future) => HandlerStep::Running(future),
        };
    let Some(handler_until) = Instant::now().checked_add(options.handler_deadline) else {
        return terminal(Status::SERVICE_UNAVAILABLE, pool, owner);
    };
    let until = match owner.request_until() {
        Ok(until) => until.min(handler_until).min(security_until),
        Err(error) => return failure(error, pool, owner),
    };
    // Construction, polling and mapper run within one unwind/absolute boundary.
    let future = AssertUnwindSafe(async {
        match handler {
            HandlerStep::Prepared { request, prepared } => prepared(request, state).await,
            #[cfg(test)]
            HandlerStep::Running(future) => future.await,
        }
    })
    .catch_unwind();
    match tokio::time::timeout_at(tokio::time::Instant::from_std(until), future).await {
        Ok(_) if Instant::now() >= until => terminal(Status::GATEWAY_TIMEOUT, pool, owner),
        Ok(Ok(PreparedCompletion::HandlerOk(response))) => {
            let wire = finalize_session_response(
                response,
                owner.lease_owner(),
                PolicySource::BrowserLogin,
                false,
            );
            finish(wire, pool, owner)
        }
        Ok(Ok(PreparedCompletion::Replacement(response))) => {
            crate::auth::session::retire_http_delivery(owner.lease_owner());
            finish(response.into_http(false), pool, owner)
        }
        Ok(Err(_)) => terminal(Status::INTERNAL_SERVER_ERROR, pool, owner),
        Err(_) => terminal(Status::GATEWAY_TIMEOUT, pool, owner),
    }
}

#[cfg(test)]
#[path = "browser_login_stage_tests.rs"]
mod stage_tests;
