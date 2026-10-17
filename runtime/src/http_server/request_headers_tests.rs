//! Native/source checkpoint oracles; all real execution remains a CI gate.
use super::super::*;
use super::*;
use crate::security_sf07_private::quota::{self, Claim, Quota, QuotaError, ResourceLimit};
use std::{
    panic::AssertUnwindSafe,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Notify,
};

fn request(headers: RequestHeaders, body: Bytes) -> Request {
    Request {
        method: Method::POST,
        uri: "/owned?x=1".parse().unwrap(),
        headers,
        body,
    }
}
fn one_claim() -> (Arc<Quota>, Claim) {
    let quota = Quota::checked_catalog(&[ResourceLimit::new(1)], 1).unwrap();
    let claim = Claim::new(quota.resource_id(0).unwrap(), 1);
    (quota, claim)
}
fn charged(quota: &Arc<Quota>, claim: Claim) -> Request {
    let mut map = HeaderMap::new();
    map.insert("x-owned", HeaderValue::from_static("retained"));
    request(
        RequestHeaders::from_received(ReceivedHeaders::new(
            map,
            Some(Arc::new(quota::try_reserve(quota, &[claim]).unwrap())),
        )),
        Bytes::new(),
    )
}
fn blocked(quota: &Arc<Quota>, claim: Claim) {
    assert!(matches!(
        quota::try_reserve(quota, &[claim]),
        Err(QuotaError::Capacity)
    ));
}
struct BodyOwner(Arc<AtomicUsize>);
impl AsRef<[u8]> for BodyOwner {
    fn as_ref(&self) -> &[u8] {
        b"owned-body"
    }
}
impl Drop for BodyOwner {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn snapshots_share_actual_map_and_never_retain_original_body() {
    let drops = Arc::new(AtomicUsize::new(0));
    let original = request(
        HeaderMap::new().into(),
        Bytes::from_owner(BodyOwner(Arc::clone(&drops))),
    );
    let snapshot = original.security_snapshot();
    let child = snapshot.security_snapshot();
    assert!(original.headers.same_owner(&snapshot.headers));
    assert!(snapshot.headers.same_owner(&child.headers));
    assert!(snapshot.body().is_empty() && child.body().is_empty());
    assert_eq!(snapshot.path(), original.path());
    assert_eq!(snapshot.query(), original.query());
    drop(original);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(child.body().is_empty());
}

#[test]
fn sanitized_views_hide_every_credential_accessor_debug_and_child_snapshot() {
    let mut map = HeaderMap::new();
    map.append("cookie", HeaderValue::from_static("first-secret"));
    map.append("cookie", HeaderValue::from_static("second-secret"));
    map.append("x-csrf-token", HeaderValue::from_static("csrf-secret"));
    map.append("x-keep", HeaderValue::from_static("one"));
    map.append("x-keep", HeaderValue::from_static("two"));
    let original = request(map.into(), Bytes::new());
    let mut hidden = original.security_snapshot();
    hidden.headers.sanitize();
    let child = hidden.security_snapshot();
    assert!(original.headers.same_owner(&hidden.headers));
    assert!(hidden.headers.same_owner(&child.headers));
    assert_eq!(headers(&original, "Cookie").unwrap().len(), 2);
    for view in [&hidden, &child] {
        for name in ["Cookie", "COOKIE", "cOoKiE", "X-CSRF-Token", "x-CsRf-ToKeN"] {
            assert!(headers(view, name).unwrap().is_empty());
            assert_eq!(header(view, name).unwrap(), None);
            assert_eq!(header_text(view, name).unwrap(), None);
            assert!(!view.headers.contains_key(name));
        }
        assert_eq!(
            headers(view, "X-Keep").unwrap(),
            [b"one".as_slice(), b"two".as_slice()]
        );
        assert!(header(view, "x-keep").is_err());
        let debug = format!("{view:?}");
        assert!(debug.contains("header_count: 2"));
        assert!(!debug.contains("secret") && !debug.contains("Cookie") && !debug.contains("CSRF"));
        for name in ["Cookie\r\n", "クッキー", "", "x csrf"] {
            assert!(headers(view, name).is_err());
            assert!(header(view, name).is_err());
            assert!(header_text(view, name).is_err());
        }
    }
}

#[test]
fn ordinary_lookup_keeps_duplicate_case_utf8_and_invalid_name_contracts() {
    let mut map = HeaderMap::new();
    map.append("x-dupe", HeaderValue::from_static("first"));
    map.append("x-dupe", HeaderValue::from_static("second"));
    map.insert("x-raw", HeaderValue::from_bytes(b"\xff").unwrap());
    map.insert(
        "x-unicode",
        HeaderValue::from_bytes("日本語".as_bytes()).unwrap(),
    );
    let original = request(map.into(), Bytes::new());
    assert_eq!(headers(&original, "X-DuPe").unwrap().len(), 2);
    assert!(header(&original, "X-DUPE").is_err());
    assert_eq!(
        header(&original, "x-raw").unwrap(),
        Some(b"\xff".as_slice())
    );
    assert!(header_text(&original, "x-raw").is_err());
    assert_eq!(header_text(&original, "X-Unicode").unwrap(), Some("日本語"));
    for name in ["あ", "x bad", "x\0bad", "x\r\nbad"] {
        assert!(header(&original, name).is_err());
    }
}

#[test]
fn header_only_last_shared_owner_retains_prior_response_reservation() {
    let (quota, claim) = one_claim();
    let original = charged(&quota, claim);
    let snapshot = original.security_snapshot();
    let last = snapshot.headers.share();
    assert!(original.headers.same_owner(&snapshot.headers));
    blocked(&quota, claim);
    drop(original);
    blocked(&quota, claim);
    drop(snapshot);
    blocked(&quota, claim);
    assert_eq!(last.get_all("x-owned").iter().next().unwrap(), "retained");
    drop(last);
    assert!(quota::try_reserve(&quota, &[claim]).is_ok());
}

#[test]
fn unpolled_future_and_unwind_release_only_their_last_request_owner() {
    let (quota, claim) = one_claim();
    let original = charged(&quota, claim);
    let pending = async move {
        std::future::pending::<()>().await;
        drop(original);
    };
    blocked(&quota, claim);
    drop(pending);
    assert!(quota::try_reserve(&quota, &[claim]).is_ok());
    let original = charged(&quota, claim);
    let view = original.security_snapshot();
    let panic = std::panic::catch_unwind(AssertUnwindSafe(move || {
        let _held = original;
        panic!("fixed request unwind control");
    }));
    assert!(panic.is_err());
    blocked(&quota, claim);
    drop(view);
    assert!(quota::try_reserve(&quota, &[claim]).is_ok());
}

#[tokio::test]
async fn cancelled_pending_request_keeps_charge_until_actual_task_join() {
    let (quota, claim) = one_claim();
    let original = charged(&quota, claim);
    let entered = Arc::new(Notify::new());
    let signal = Arc::clone(&entered);
    let task = tokio::spawn(async move {
        signal.notify_one();
        std::future::pending::<()>().await;
        drop(original);
    });
    tokio::time::timeout(Duration::from_secs(2), entered.notified())
        .await
        .unwrap();
    blocked(&quota, claim);
    task.abort();
    let outcome = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap();
    assert!(outcome.unwrap_err().is_cancelled());
    assert!(quota::try_reserve(&quota, &[claim]).is_ok());
}

#[test]
fn fixture_mutation_cannot_copy_shared_map_or_mutate_charged_native_owner() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Request>();
    send_sync::<ReceivedHeaders>();
    let mut original = request(HeaderMap::new().into(), Bytes::new());
    let snapshot = original.security_snapshot();
    assert!(std::panic::catch_unwind(AssertUnwindSafe(|| original
        .headers
        .insert("x-test", HeaderValue::from_static("x"))))
    .is_err());
    drop(snapshot);
    assert!(original
        .headers
        .insert("x-test", HeaderValue::from_static("x"))
        .is_none());
    let (quota, claim) = one_claim();
    let mut original = charged(&quota, claim);
    assert!(std::panic::catch_unwind(AssertUnwindSafe(|| original
        .headers
        .insert("x-test", HeaderValue::from_static("x"))))
    .is_err());
    blocked(&quota, claim);
    drop(original);
    assert!(quota::try_reserve(&quota, &[claim]).is_ok());
}

#[derive(Clone)]
struct ExtensionProbe(Arc<AtomicUsize>);
impl Drop for ExtensionProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct ExtensionState {
    drops: Arc<AtomicUsize>,
    entered: Notify,
    release: Notify,
    panic: bool,
}
async fn extension_handler(
    request: Request,
    state: Arc<ExtensionState>,
    _: (),
) -> Result<Response, Error> {
    let snapshot = request.security_snapshot();
    assert!(request.headers.same_owner(&snapshot.headers));
    assert!(snapshot.body().is_empty());
    assert_eq!(state.drops.load(Ordering::SeqCst), 0);
    state.entered.notify_one();
    state.release.notified().await;
    assert_eq!(state.drops.load(Ordering::SeqCst), 0);
    if state.panic {
        panic!("fixed extension handler panic");
    }
    Ok(text(Status::OK, "owned"))
}

#[tokio::test]
async fn actual_native_parts_extensions_live_through_handler_and_drop_after_join() {
    // Exercise the SAME production dispatcher with a real Incoming, not a copied
    // normalization helper. Cancellation is joined before observing disposal.
    for mode in 0..3 {
        let drops = Arc::new(AtomicUsize::new(0));
        let state = ExtensionState {
            drops: Arc::clone(&drops),
            entered: Notify::new(),
            release: Notify::new(),
            panic: mode == 1,
        };
        let app = route(
            app_default(state),
            Method::GET,
            "/probe",
            public_policy(),
            extension_handler,
        )
        .unwrap();
        let state = Arc::clone(&app.state);
        let app = Arc::new(app);
        let options = Arc::new(
            authority(
                options(256, 1000, 1000, 1000).unwrap(),
                "https://localhost",
                vec!["localhost".to_owned()],
                1,
                128,
            )
            .unwrap(),
        );
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let native_drops = Arc::clone(&drops);
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let service = service_fn(move |mut request: hyper::Request<hyper::body::Incoming>| {
                request
                    .extensions_mut()
                    .insert(ExtensionProbe(Arc::clone(&native_drops)));
                dispatch(
                    Arc::clone(&app),
                    request,
                    Arc::clone(&options),
                    Arc::clone(&permits),
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                )
            });
            http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
        });
        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"GET /probe HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), state.entered.notified())
            .await
            .unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        if mode == 2 {
            task.abort();
            assert!(tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap_err()
                .is_cancelled());
        } else {
            state.release.notify_one();
            let mut raw = Vec::new();
            tokio::time::timeout(
                Duration::from_secs(2),
                (&mut client).take(8193).read_to_end(&mut raw),
            )
            .await
            .unwrap()
            .unwrap();
            assert!(raw.len() <= 8192);
            assert!(raw.starts_with(if mode == 1 {
                b"HTTP/1.1 500"
            } else {
                b"HTTP/1.1 200"
            }));
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
