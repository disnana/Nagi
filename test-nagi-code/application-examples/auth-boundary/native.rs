use axum::{
    extract::{DefaultBodyLimit, FromRequestParts, Json, Path, Request},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use futures_util::FutureExt;
use nagi_runtime::{
    auth::{Grant, Principal},
    rusqlite::{params, Connection, OptionalExtension},
};
use std::{
    net::Ipv4Addr,
    panic::AssertUnwindSafe,
    sync::{Mutex, OnceLock},
    time::Duration,
};

const BODY_BYTES: usize = 4096;
const HANDLER_MS: u64 = 1000;
const MAX_REQUESTS: usize = 32;
static DATABASE: OnceLock<Mutex<Connection>> = OnceLock::new();
static RUST_POLICY: OnceLock<bool> = OnceLock::new();
static CAPACITY: OnceLock<tokio::sync::Semaphore> = OnceLock::new();

pub fn principal_subject(principal: &Principal) -> i64 {
    principal.subject()
}
pub async fn policy_pause() {
    tokio::task::yield_now().await;
}

fn drive_policy<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

fn microbench() {
    let principal = Principal::from_verified_subject(1);
    let access = || super::DocumentAccess {
        document_id: 1,
        owner_subject: 1,
        blocked: false,
    };
    println!(
        "{}",
        nagi_runtime::serde_json::json!({
            "name":"auth_future_shapes",
            "principal_bytes":std::mem::size_of::<Principal>(),
            "grant_bytes":std::mem::size_of::<Grant<super::Read>>(),
            "nagi_policy_future_bytes":std::mem::size_of_val(&super::read_policy(&principal, access())),
            "rust_policy_future_bytes":std::mem::size_of_val(&rust_read_policy(&principal, access())),
            "nagi_load_future_bytes":std::mem::size_of_val(&super::load_document(Principal::from_verified_subject(1),1)),
            "rust_load_future_bytes":std::mem::size_of_val(&rust_load_document(Principal::from_verified_subject(1),1)),
            "polling":"same calling thread, noop waker; policy contains yield_now only, no scheduler/I/O comparison",
        })
    );
    nagi_runtime::metrics::benchmark("auth_nagi_policy", 1, || {
        drive_policy(super::read_policy(
            std::hint::black_box(&principal),
            std::hint::black_box(access()),
        ))
    });
    nagi_runtime::metrics::benchmark("auth_rust_policy", 1, || {
        drive_policy(rust_read_policy(
            std::hint::black_box(&principal),
            std::hint::black_box(access()),
        ))
    });
    nagi_runtime::metrics::benchmark("auth_grant_issue_consume", 1, || {
        Grant::<super::Read>::from_authorized(
            std::hint::black_box(&principal),
            std::hint::black_box(1),
        )
        .into_authorized_parts()
    });
}

// Demo fixtures only. This verifies no JWS signature or production token.
// Replace this exact boundary with a reviewed existing Rust JWS verifier.
fn authenticate(headers: &HeaderMap) -> Result<Principal, super::AuthError> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next().ok_or(super::AuthError::MissingCredentials)?;
    if values.next().is_some() {
        return Err(super::AuthError::InvalidHeader);
    }
    let value = value
        .to_str()
        .map_err(|_| super::AuthError::InvalidHeader)?;
    let subject = match value {
        "Bearer demo-alice" => 1,
        "Bearer demo-bob" => 2,
        _ => return Err(super::AuthError::InvalidCredentials),
    };
    Ok(Principal::from_verified_subject(subject))
}

fn access(document_id: i64) -> Result<super::DocumentAccess, super::AuthError> {
    let db = DATABASE
        .get()
        .ok_or(super::AuthError::Internal)?
        .lock()
        .map_err(|_| super::AuthError::Internal)?;
    db.query_row(
        "SELECT id, owner_subject, blocked FROM documents WHERE id = ?1",
        [document_id],
        |row| {
            Ok(super::DocumentAccess {
                document_id: row.get(0)?,
                owner_subject: row.get(1)?,
                blocked: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|_| super::AuthError::Internal)?
    .ok_or(super::AuthError::NotFound)
}

async fn rust_read_policy(
    principal: &Principal,
    access: super::DocumentAccess,
) -> Result<(), super::AuthError> {
    if principal_subject(principal) != access.owner_subject {
        return Err(super::AuthError::Denied);
    }
    if access.blocked {
        return Err(super::AuthError::Denied);
    }
    if access.document_id < 1 || access.document_id > 3 {
        return Err(super::AuthError::Denied);
    }
    policy_pause().await;
    Ok(())
}

pub async fn authorize_read(
    principal: &Principal,
    document_id: i64,
) -> Result<Grant<super::Read>, super::AuthError> {
    let access = access(document_id)?;
    // The permission marker is associated with this named policy by reviewed
    // adapter code. A boolean supplied by an HTTP client cannot mint a Grant.
    if RUST_POLICY.get().copied().unwrap_or(false) {
        rust_read_policy(principal, access).await?;
    } else {
        super::read_policy(principal, access).await?;
    }
    Ok(Grant::from_authorized(principal, document_id))
}

pub async fn read_document(grant: Grant<super::Read>) -> Result<super::Document, super::AuthError> {
    let (subject, resource) = grant.into_authorized_parts();
    let db = DATABASE
        .get()
        .ok_or(super::AuthError::Internal)?
        .lock()
        .map_err(|_| super::AuthError::Internal)?;
    // Recheck the same bound owner/block conditions in the data query. The
    // policy snapshot alone does not promise atomicity or revocation safety.
    db.query_row(
        "SELECT id, title FROM documents WHERE id = ?1 AND owner_subject = ?2 AND blocked = 0",
        params![resource, subject],
        |row| {
            Ok(super::Document {
                id: row.get(0)?,
                title: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(|_| super::AuthError::Internal)?
    .ok_or(super::AuthError::Denied)
}

async fn rust_load_document(
    principal: Principal,
    document_id: i64,
) -> Result<super::Document, super::AuthError> {
    let grant = authorize_read(&principal, document_id).await?;
    read_document(grant).await
}

async fn dispatch_load(
    principal: Principal,
    document_id: i64,
) -> Result<super::Document, super::AuthError> {
    if RUST_POLICY.get().copied().unwrap_or(false) {
        rust_load_document(principal, document_id).await
    } else {
        super::load_document(principal, document_id).await
    }
}

fn failure(error: super::AuthError) -> Response {
    let (status, code) = match error {
        super::AuthError::MissingCredentials | super::AuthError::InvalidCredentials => {
            (StatusCode::UNAUTHORIZED, "authentication_required")
        }
        super::AuthError::InvalidHeader => (StatusCode::BAD_REQUEST, "invalid_header"),
        super::AuthError::Denied => (StatusCode::FORBIDDEN, "access_denied"),
        super::AuthError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        super::AuthError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    };
    let mut response =
        (status, Json(nagi_runtime::serde_json::json!({"code":code}))).into_response();
    if status == StatusCode::UNAUTHORIZED {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, "Bearer".parse().unwrap());
    }
    response
}

struct VerifiedPrincipal(Principal);
impl<S: Send + Sync> FromRequestParts<S> for VerifiedPrincipal {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        authenticate(&parts.headers).map(Self).map_err(failure)
    }
}

async fn authenticated(VerifiedPrincipal(principal): VerifiedPrincipal) -> Response {
    Json(nagi_runtime::serde_json::json!({"subject":principal.subject()})).into_response()
}

async fn document(
    VerifiedPrincipal(principal): VerifiedPrincipal,
    Path(document_id): Path<i64>,
) -> Response {
    match dispatch_load(principal, document_id).await {
        Ok(document) => Json(document).into_response(),
        Err(error) => failure(error),
    }
}

#[derive(nagi_runtime::serde::Deserialize)]
#[serde(crate = "nagi_runtime::serde", deny_unknown_fields)]
struct ReadInput {
    document_id: i64,
}

async fn read_input(
    VerifiedPrincipal(principal): VerifiedPrincipal,
    body: Result<Json<ReadInput>, axum::extract::rejection::JsonRejection>,
) -> Response {
    // FromRequestParts authentication runs before the body-consuming Json
    // extractor. All modes share this order and the same body/deadline limits.
    let input = match body {
        Ok(Json(input)) => input,
        Err(error) => return error.into_response(),
    };
    match dispatch_load(principal, input.document_id).await {
        Ok(document) => Json(document).into_response(),
        Err(error) => failure(error),
    }
}

async fn boundary(request: Request, next: Next) -> Response {
    let permit = match CAPACITY.get().expect("initialized capacity").try_acquire() {
        Ok(permit) => permit,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "busy").into_response(),
    };
    let result = tokio::time::timeout(
        Duration::from_millis(HANDLER_MS),
        AssertUnwindSafe(next.run(request)).catch_unwind(),
    )
    .await;
    drop(permit);
    match result {
        Ok(Ok(response)) => response,
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error").into_response(),
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "timeout").into_response(),
    }
}

pub async fn run_server(port: i64) -> Result<(), nagi_runtime::Error> {
    let port = u16::try_from(port)
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| nagi_runtime::Error::invalid("port must be between 1 and 65535"))?;
    if std::env::var("NAGI_AUTH_MICRO").as_deref() == Ok("1") {
        microbench();
        return Ok(());
    }
    let mode = std::env::var("NAGI_AUTH_POLICY_MODE").unwrap_or_else(|_| "nagi".into());
    let rust_policy = match mode.as_str() {
        "nagi" => false,
        "rust" => true,
        _ => {
            return Err(nagi_runtime::Error::invalid(
                "policy mode must be nagi or rust",
            ))
        }
    };
    RUST_POLICY
        .set(rust_policy)
        .map_err(|_| nagi_runtime::Error::invalid("server already initialized"))?;
    CAPACITY
        .set(tokio::sync::Semaphore::new(MAX_REQUESTS))
        .map_err(|_| nagi_runtime::Error::invalid("capacity already initialized"))?;
    let db = Connection::open_in_memory()?;
    db.execute_batch("CREATE TABLE documents(id INTEGER PRIMARY KEY, owner_subject INTEGER NOT NULL, blocked INTEGER NOT NULL, title TEXT NOT NULL); INSERT INTO documents VALUES(1,1,0,'Alice document'),(2,2,0,'Bob document'),(3,1,1,'Blocked document');")?;
    DATABASE
        .set(Mutex::new(db))
        .map_err(|_| nagi_runtime::Error::invalid("database already initialized"))?;
    let mut router = Router::new()
        .route("/health", get(|| async { "ok\n" }))
        .route("/me", get(authenticated))
        .route("/documents/{id}", get(document))
        .route("/documents/read", post(read_input));
    // Test-only routes exercise the identical middleware and release paths.
    if std::env::var("NAGI_AUTH_PROBES").as_deref() == Ok("1") {
        router = router
            .route(
                "/probe/panic",
                get(|_: VerifiedPrincipal| async {
                    panic!("auth boundary test panic");
                    #[allow(unreachable_code)]
                    "unreachable"
                }),
            )
            .route(
                "/probe/timeout",
                get(|_: VerifiedPrincipal| async {
                    tokio::time::sleep(Duration::from_millis(HANDLER_MS * 2)).await;
                    "late"
                }),
            );
    }
    let router = router
        .layer(DefaultBodyLimit::max(BODY_BYTES))
        .layer(middleware::from_fn(boundary));
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    println!("Auth boundary listening http://127.0.0.1:{port} policy={mode}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))
}
