//! Test-owned DB fixture orchestration, not a production raw-digest API.
//! Fixed row/read/clock/CAS/snapshot/rollback semantics are the parent's actual
//! read_rows, fresh_plan, finish_written and finish; no request proof is minted.
use super::*;
// Trusted foundation entrypoints share the same fixed recipes, after-reserve
// sample and actual finish. They never manufacture AuthScope or request proof.
async fn primitive(
    store: &Foundation,
    operation: Operation,
    digest: [u8; 32],
) -> Result<Snapshot, Failure> {
    let (tx, c, m) = store.transaction().await?;
    let mut progress = Progress {
        wall: c.ceil_ms.max(m.last_wall),
        sent: false,
    };
    let result = async {
        let rows = read_rows(store, &tx, &operation, &mut progress).await?;
        if collision(&tx, digest).await? {
            return Err(Failure::unavailable());
        }
        let reservation = tx.reserve_exec().await.map_err(Failure::database)?;
        let (statement, completion) = fresh_plan(store, &operation, digest, rows, &mut progress)?;
        let (query, params) = statement.into_native();
        let reply = reservation.enqueue(query, params);
        progress.sent = true;
        finish_written(store, &tx, Box::pin(reply), completion).await
    }
    .await;
    let result = match exec(
        &tx,
        "UPDATE __nagi_session_meta SET last_wall=MAX(last_wall,?) WHERE id=1",
        sqlite::bind_i64(sqlite::parameters(), progress.wall),
    )
    .await
    {
        Ok(_) => result,
        Err(e) => Err(e),
    };
    finish(tx, result, progress.sent.then_some((store, progress.wall))).await
}
pub(in crate::auth::session) async fn insert_primitive(
    store: &Foundation,
    subject: i64,
    digest: [u8; 32],
    original: Instant,
) -> Result<Snapshot, Failure> {
    primitive(store, Operation::Issue { subject, original }, digest).await
}
pub(in crate::auth::session) async fn rotate_primitive(
    store: &Foundation,
    old: &Snapshot,
    digest: [u8; 32],
) -> Result<Snapshot, Failure> {
    primitive(
        store,
        Operation::Rotate {
            old: old.private_copy(),
        },
        digest,
    )
    .await
}

pub(in crate::auth::session) async fn logout(
    store: &Foundation,
    old: &Snapshot,
) -> Result<(), Failure> {
    let (tx, _, _) = store.transaction().await?;
    let result = async {
        if old.incarnation != store.incarnation {
            return Err(Failure::denied());
        }
        let present = sqlite::query::<super::super::super::Count>(
            &tx,
            sqlite::literal(
                "SELECT count(*) AS n FROM __nagi_session_rows WHERE lineage=? AND subject<>?",
            ),
            sqlite::bind_i64(
                sqlite::bind_i64(sqlite::parameters(), old.lineage),
                old.subject,
            ),
        )
        .await
        .map_err(Failure::database)?
        .ok_or_else(Failure::unavailable)?
        .0;
        if present != 0 {
            return Err(Failure::denied());
        }
        exec(
            &tx,
            "DELETE FROM __nagi_session_rows WHERE lineage=? AND subject=?",
            sqlite::bind_i64(
                sqlite::bind_i64(sqlite::parameters(), old.lineage),
                old.subject,
            ),
        )
        .await?;
        Ok(()) // only returned after finish confirms this same Tx commit.
    }
    .await;
    finish(tx, result, None).await
}
