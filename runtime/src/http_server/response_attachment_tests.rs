use super::*;
use std::{sync::Mutex, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

const TEST_WAIT: Duration = Duration::from_secs(2);
fn limits() -> Limits {
    Limits {
        body_bytes: 256,
        user_header_bytes: 128,
        user_header_count: 4,
        managed_header_bytes: 512,
        managed_header_count: 12,
        serialization_writes: 128,
        live_responses: 1,
        retained_bytes: 131072,
    }
}
fn fallback() -> FallbackLimits {
    FallbackLimits {
        responses: 1,
        retained_bytes: 131072,
        managed_header_bytes: 512,
        managed_header_count: 12,
    }
}
fn driver() -> DriverConfig {
    DriverConfig::checked(16, 8192, TEST_WAIT, TEST_WAIT, TEST_WAIT, true).unwrap()
}
fn options() -> Arc<Options> {
    Arc::new(Options::checked(limits(), fallback(), 1, 256, driver()).unwrap())
}
fn blocked(options: &Options) {
    assert_eq!(options.producer.empty().err(), Some(Diagnostic::Capacity));
}
async fn sockets() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (server, _) = listener.accept().await.unwrap();
    (client, server)
}
async fn send(client: &mut TcpStream, head: bool, later: bool) {
    let raw: &[u8] = if head {
        b"HEAD / HTTP/1.1\r\nHost: localhost\r\n\r\n"
    } else if later {
        b"GET /later HTTP/1.1\r\nHost: localhost\r\n\r\n"
    } else {
        b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"
    };
    tokio::time::timeout(TEST_WAIT, client.write_all(raw))
        .await
        .unwrap()
        .unwrap();
}
async fn read_response(client: &mut TcpStream, head: bool) -> (u16, Vec<u8>) {
    tokio::time::timeout(TEST_WAIT, async {
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            assert!(headers.len() < 8192);
            let mut byte = [0; 1];
            client.read_exact(&mut byte).await.unwrap();
            headers.push(byte[0]);
        }
        let text = std::str::from_utf8(&headers).unwrap();
        let status = text
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert!(text.contains("content-security-policy:"));
        assert!(text.contains("x-content-type-options: nosniff"));
        let length = text
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map_or(0, |(_, value)| value.trim().parse::<usize>().unwrap());
        let mut body = vec![0; if head { 0 } else { length }];
        client.read_exact(&mut body).await.unwrap();
        (status, body)
    })
    .await
    .unwrap()
}
async fn eof(client: &mut TcpStream) {
    let mut byte = [0; 1];
    assert_eq!(
        tokio::time::timeout(TEST_WAIT, client.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}
#[test]
fn explicit_attachment_inputs_reject_invalid_profiles() {
    assert!(Options::checked(limits(), fallback(), 0, 256, driver()).is_err());
    assert!(Options::checked(limits(), fallback(), 1, 0, driver()).is_err());
    let mut invalid = limits();
    invalid.body_bytes = 0;
    assert!(Options::checked(invalid, fallback(), 1, 256, driver()).is_err());
    assert!(Options::checked(limits(), fallback(), 1, usize::MAX, driver()).is_err());
    assert!(DriverConfig::checked(0, 8192, TEST_WAIT, TEST_WAIT, TEST_WAIT, true).is_err());
    assert!(DriverConfig::checked(16, 8191, TEST_WAIT, TEST_WAIT, TEST_WAIT, true).is_err());
    assert!(DriverConfig::checked(16, 8192, Duration::ZERO, TEST_WAIT, TEST_WAIT, true).is_err());
    assert!(DriverConfig::checked(16, 8192, TEST_WAIT, Duration::MAX, TEST_WAIT, true).is_err());
}
#[test]
fn context_factories_keep_original_charge_and_enforce_serialized_limits() {
    let options = options();
    let context = ResponseContext {
        producer: Arc::clone(&options.producer),
    };
    let response = text(&context, Status::OK, "retained").unwrap();
    assert_eq!(response.produced.body(), b"retained");
    assert_eq!(
        empty(&context, Status::OK).err().unwrap(),
        Diagnostic::Capacity
    );
    drop(response);
    assert!(bytes(&context, Status::OK, &[0; 257]).is_err());
    assert_eq!(
        json(&context, Status::OK, &"\\".repeat(200)).err().unwrap(),
        Diagnostic::BodyLimit
    );
    let response = json(&context, Status::OK, &vec![1, 2]).unwrap();
    assert_eq!(response.produced.body(), b"[1,2]");
    drop(response);
    let policy = crate::html::policy("https://example.invalid").unwrap();
    let fragment = crate::html::text(&policy, "ok").unwrap();
    let doc = crate::html::document(&policy, "title", fragment).unwrap();
    let response = html_response(&context, Status::OK, doc).unwrap();
    assert!(response.produced.body().starts_with(b"<!doctype html>"));
    drop(response);
    assert!(empty(&context, Status::NO_CONTENT).is_ok());
}
#[tokio::test]
async fn actual_native_context_empty_body_and_head_retain_charge_through_join() {
    for (head, body) in [(false, ""), (false, "native"), (true, "native")] {
        let options = options();
        let (mut client, server) = sockets().await;
        let (mut native, _) = options
            .connection(server, move |request| async move {
                assert_eq!(request.path(), "/");
                let context = response_context(&request);
                if body.is_empty() {
                    empty(context, Status::OK)
                } else {
                    text(context, Status::OK, body)
                }
            })
            .unwrap();
        let task = tokio::spawn(async move {
            let result = (&mut native).await;
            (native, result)
        });
        send(&mut client, head, false).await;
        let (status, received) = read_response(&mut client, head).await;
        assert_eq!(status, 200);
        assert_eq!(received, if head { b"" } else { body.as_bytes() });
        blocked(&options); // Return/callback/body consumption/flush is not release.
        client.shutdown().await.unwrap();
        let (native, result) = tokio::time::timeout(TEST_WAIT, task)
            .await
            .unwrap()
            .unwrap();
        assert!(result.is_ok());
        blocked(&options);
        assert_eq!(options.connections.available_permits(), 0);
        drop(native);
        assert_eq!(options.connections.available_permits(), 1);
        assert!(options.producer.empty().is_ok());
    }
}
#[tokio::test]
async fn received_request_owner_keeps_prior_charge_after_native_close_and_join() {
    let options = options();
    let (mut client, server) = sockets().await;
    let (held_tx, held_rx) = oneshot::channel();
    let held_tx = Mutex::new(Some(held_tx));
    let (mut native, _) = options
        .connection(server, move |request| {
            let output = if request.path() == "/later" {
                assert!(held_tx
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(request)
                    .is_ok());
                Err(Diagnostic::Capacity)
            } else {
                text(response_context(&request), Status::OK, "owned")
            };
            async move { output }
        })
        .unwrap();
    let task = tokio::spawn(async move {
        let result = (&mut native).await;
        (native, result)
    });
    send(&mut client, false, false).await;
    assert_eq!(read_response(&mut client, false).await.1, b"owned");
    send(&mut client, false, true).await;
    assert_eq!(read_response(&mut client, false).await.0, 503);
    let held = tokio::time::timeout(TEST_WAIT, held_rx)
        .await
        .unwrap()
        .unwrap();
    client.shutdown().await.unwrap();
    let (native, result) = tokio::time::timeout(TEST_WAIT, task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_ok());
    drop(native);
    assert_eq!(options.connections.available_permits(), 1);
    blocked(&options);
    drop(held);
    assert!(options.producer.empty().is_ok());
}
#[tokio::test]
async fn exhausted_independent_fallback_closes_actual_native_before_permit_release() {
    let options = options();
    let context = ResponseContext {
        producer: Arc::clone(&options.producer),
    };
    let ordinary_held = empty(&context, Status::OK).unwrap();
    let (mut client, server) = sockets().await;
    let (held_tx, held_rx) = oneshot::channel();
    let held_tx = Mutex::new(Some(held_tx));
    let (mut native, _) = options
        .connection(server, move |request| {
            let output = empty(response_context(&request), Status::OK);
            if request.path() == "/later" {
                assert!(held_tx
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(request)
                    .is_ok());
            }
            async move { output }
        })
        .unwrap();
    let task = tokio::spawn(async move {
        let result = (&mut native).await;
        (native, result)
    });
    send(&mut client, false, false).await;
    assert_eq!(read_response(&mut client, false).await.0, 503);
    send(&mut client, false, true).await;
    eof(&mut client).await;
    let held = tokio::time::timeout(TEST_WAIT, held_rx)
        .await
        .unwrap()
        .unwrap();
    let (native, result) = tokio::time::timeout(TEST_WAIT, task)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(SendFault::Closed)));
    assert_eq!(options.connections.available_permits(), 0);
    drop(native);
    assert_eq!(options.connections.available_permits(), 1);
    assert!(options
        .fallback
        .produce(FallbackStatus::Internal)
        .is_close());
    drop(held);
    assert!(options
        .fallback
        .produce(FallbackStatus::Internal)
        .into_response()
        .is_some());
    blocked(&options);
    drop(ordinary_held);
    assert!(options.producer.empty().is_ok());
}
