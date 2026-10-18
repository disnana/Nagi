//! Actual issuer/SQLite path, with the original fixed Bytes owner observed.
use super::*;

#[tokio::test]
async fn actual_csrf_wire_fixed_owner_outlives_issuer_until_last_bytes_clone() {
    let f = Fixture::with_limits(2, 176, 10000, 100000, 10000).await;
    let (o, b, digest, expiry) = f.issue().await;
    let seal = o.seal().unwrap();
    let replay = TokenSeal {
        request: seal.request,
        cell: seal.cell.clone(),
        context: seal.context,
    };
    let body = f.inner.take_wire(&o, seal, &b).unwrap();
    assert!(body.len() == FIXED_TOKEN_BODY_BYTES);
    assert!(body.starts_with(b"{\"csrf_token\":\"") && body.ends_with(b"\"}"));
    assert!(o.cell.lock().unwrap().token.is_none());
    assert!(f.inner.take_wire(&o, replay, &b).is_err());
    f.assert_original_token_row(digest, expiry).await;
    let probe = fixed_body::observation::register(&body);
    let pointer = body.as_ptr();
    let last = hyper::body::Frame::data(body.clone());
    drop(body);
    assert_eq!(probe.drops(), 0);
    drop(o);
    f.stop().await;
    assert!(last.data_ref().unwrap().as_ptr() == pointer);
    assert_eq!(probe.drops(), 0);
    drop(last);
    assert_eq!(probe.drops(), 1);
}
