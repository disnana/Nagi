//! Explicit test-tool only. Auth/cookie/CSRF behavior is the shared public-API fixture.
#[path = "../../../runtime/tests/support/cors.rs"]
mod cors;
mod expiry;
#[path = "../../../runtime/tests/support/browser_login.rs"]
mod login;
use nagi_runtime::{
    http_server as http,
    serde_json::{self, json},
};
use std::{
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

fn wall_ceil_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
    .checked_add(1)
    .unwrap()
}
#[tokio::main]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert!(args.len() == 3, "fixed test-tool arguments required");
    let directory = PathBuf::from(&args[0]);
    let app_origin = &args[1];
    let ui_origin = &args[2];
    assert!(
        app_origin.starts_with("https://localhost:")
            && ui_origin.starts_with("https://app.example.invalid:")
    );
    let options = http::authority(
        http::options(4096, 5000, 5000, 2000).unwrap(),
        app_origin,
        vec![app_origin.strip_prefix("https://").unwrap().to_owned()],
        1,
        128,
    )
    .unwrap();
    let fixture = login::Fixture::owned_browser(directory, &options).await;
    let login_server = login::Server::with_options(&fixture, options).await;
    let cors_server = cors::Server::browser(8, 5000, app_origin, ui_origin).await;
    println!(
        "{}",
        json!({"ready":true, "login_port":login_server.address.port(), "cors_port":cors_server.address.port()})
    );
    let mut input = BufReader::new(tokio::io::stdin());
    let mut expiry_witness = expiry::ExpiryWitness::default();
    loop {
        let mut line = Vec::new();
        (&mut input)
            .take(129)
            .read_until(b'\n', &mut line)
            .await
            .unwrap();
        if line == b"stop\n" {
            break;
        }
        assert!(
            line.len() <= 128 && line.ends_with(b"\n"),
            "bounded command required"
        );
        let value = match line.as_slice() {
            b"snapshot login\n" => {
                // Anchor before asynchronous reads; SQL latency cannot extend the bound.
                let sampled_at = Instant::now();
                let rows = fixture.rows().await;
                let first_ready_expiry = if rows > 0 {
                    Some(
                        login::number(
                            &fixture.auth,
                            "SELECT min(expiry_ms) AS n FROM __nagi_prelogin_ready",
                        )
                        .await,
                    )
                } else {
                    None
                };
                let persisted = login::number(
                    &fixture.auth,
                    "SELECT last_wall AS n FROM __nagi_prelogin_meta WHERE id=1",
                )
                .await;
                // One raw wall sample, paired with an earlier monotonic instant.
                // Observe empty snapshots too: rollback cannot be forgotten before bootstrap.
                let raw_wall = wall_ceil_ms();
                let unexpired = expiry_witness.observe(
                    raw_wall,
                    persisted,
                    first_ready_expiry,
                    sampled_at,
                    Instant::now(),
                );
                json!({"route_calls":fixture.stats.handler.load(Ordering::SeqCst),
                    "verifier_calls":fixture.stats.verifier.load(Ordering::SeqCst),
                    "claim_committed":fixture.stats.claim_committed.load(Ordering::SeqCst),
                    "ready_rows":rows, "original_prelogin_unexpired":unexpired})
            }
            b"snapshot cors\n" => {
                json!({"route_calls":cors_server.state.calls.load(Ordering::SeqCst),
                "verifier_calls":cors_server.state.verifier.load(Ordering::SeqCst),
                "claim_committed":null,"ready_rows":null,"original_prelogin_unexpired":null})
            }
            _ => panic!("unknown fixed command"),
        };
        println!("{}", serde_json::to_string(&value).unwrap());
    }
    login_server.stop().await;
    cors_server.stop().await;
    fixture.stop().await;
    // These booleans follow actual HTTP joins and both public native close acknowledgements.
    println!(
        "{}",
        json!({"stopped":true,"http_joined":true,"prelogin_closed":true,"auth_closed":true})
    );
}
