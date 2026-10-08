use std::{
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use nagi_runtime::{
    auth::{AuthScope, Failure, Grant, VerifiedIdentity},
    http_server as http,
    rusqlite::{params, Connection, OptionalExtension},
    Error,
};

static DATABASE: OnceLock<Mutex<Connection>> = OnceLock::new();
static READ_CAPACITY: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();

// Demo fixtures only. Replace this exact comparison with a reviewed token
// verifier that checks signature, audience, expiry, and revocation.
pub async fn verify(
    request: http::Request,
    _state: Arc<super::State>,
) -> Result<VerifiedIdentity, Failure> {
    if !request.body().is_empty() {
        return Err(Failure::invalid_request());
    }
    let authorization = http::header_text(&request, "authorization")
        .map_err(|_| Failure::invalid_request())?
        .ok_or_else(Failure::invalid_credential)?;
    let subject = match authorization {
        "Bearer demo-alice" => 1,
        "Bearer demo-bob" => 2,
        "Bearer demo-expired" => return Err(Failure::expired()),
        _ => return Err(Failure::invalid_credential()),
    };
    VerifiedIdentity::from_verified(subject, Instant::now() + Duration::from_secs(30))
}

pub async fn policy_pause() {
    tokio::task::yield_now().await;
}

fn access(document_id: i64) -> Result<super::DocumentAccess, super::AuthError> {
    let database = DATABASE.get().ok_or(super::AuthError::Internal)?;
    let database = database.lock().map_err(|_| super::AuthError::Internal)?;
    database
        .query_row(
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
    scope: &AuthScope,
    access: super::DocumentAccess,
) -> Result<(), super::AuthError> {
    if scope.subject() != access.owner_subject || access.blocked {
        return Err(super::AuthError::Denied);
    }
    if access.document_id < 1 || access.document_id > 3 {
        return Err(super::AuthError::Denied);
    }
    policy_pause().await;
    Ok(())
}

pub async fn authorize_read(
    scope: AuthScope,
    document_id: i64,
    state: Arc<super::State>,
) -> Result<Grant<super::Read>, super::AuthError> {
    let access = access(document_id)?;
    if state.use_rust_policy {
        rust_read_policy(&scope, access).await?;
    } else {
        super::read_policy(&scope, access).await?;
    }
    Grant::from_authorized(scope, document_id).map_err(|_| super::AuthError::Internal)
}

pub async fn read_document(
    grant: Grant<super::Read>,
) -> Result<super::Document, super::AuthError> {
    let capacity = READ_CAPACITY
        .get()
        .cloned()
        .ok_or(super::AuthError::Internal)?;
    let reservation = tokio::time::timeout(Duration::from_secs(1), capacity.acquire_owned())
        .await
        .map_err(|_| super::AuthError::Internal)?
        .map_err(|_| super::AuthError::Internal)?;

    // The proof is admitted only after capacity is reserved. Its consumed
    // target and subject bind the synchronous database command; the query
    // rechecks the mutable owner/block state before returning a document.
    match grant.submit(reservation, |subject, resource, _reservation| {
        let Some(database) = DATABASE.get() else {
            return Err(super::AuthError::Internal);
        };
        let database = match database.lock() {
            Ok(database) => database,
            Err(_) => return Err(super::AuthError::Internal),
        };
        database
            .query_row(
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
    }) {
        Ok(result) => result,
        Err(_) => Err(super::AuthError::Internal),
    }
}

pub fn decode_read_input(body: &[u8]) -> Result<super::ReadInput, super::AuthError> {
    match nagi_runtime::serde_json::from_slice(body) {
        Ok(value) => Ok(value),
        Err(error) => match error.classify() {
            nagi_runtime::serde_json::error::Category::Syntax
            | nagi_runtime::serde_json::error::Category::Eof => {
                Err(super::AuthError::InvalidRequest)
            }
            nagi_runtime::serde_json::error::Category::Data => {
                Err(super::AuthError::Unprocessable)
            }
            nagi_runtime::serde_json::error::Category::Io => Err(super::AuthError::Internal),
        },
    }
}

pub async fn initialize() -> Result<(), Error> {
    let database = Connection::open_in_memory()?;
    database.execute_batch(
        "CREATE TABLE documents(id INTEGER PRIMARY KEY, owner_subject INTEGER NOT NULL, blocked INTEGER NOT NULL, title TEXT NOT NULL); INSERT INTO documents VALUES(1,1,0,'Alice document'),(2,2,0,'Bob document'),(3,1,1,'Blocked document');",
    )?;
    DATABASE
        .set(Mutex::new(database))
        .map_err(|_| Error::invalid("database already initialized"))?;
    READ_CAPACITY
        .set(Arc::new(tokio::sync::Semaphore::new(8)))
        .map_err(|_| Error::invalid("read capacity already initialized"))?;
    Ok(())
}
