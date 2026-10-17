//! Actual bootstrap/SQLite transfer; final Bytes destruction is the oracle.
use super::*;

#[tokio::test]
async fn actual_prelogin_wire_fixed_owner_outlives_issuer_until_last_bytes_clone() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let o = owner();
    p.bootstrap(&o, None).await.unwrap();
    let seal = o.seal().unwrap();
    let wire = p.take_wire(&o, seal).unwrap();
    assert!(wire.body.len() == crate::auth::csrf::FIXED_TOKEN_BODY_BYTES);
    assert!(wire.body.starts_with(b"{\"csrf_token\":\"") && wire.body.ends_with(b"\"}"));
    assert!(wire.cookie.is_sensitive());
    assert!(o.cell.lock().unwrap().pair.is_none() && o.seal().is_err());
    assert!(count(&p).await == 1);
    let probe = crate::auth::csrf::fixed_body::observation::register(&wire.body);
    let pointer = wire.body.as_ptr();
    let last = hyper::body::Frame::data(wire.body.clone());
    drop(wire.body);
    drop(wire.cookie);
    assert_eq!(probe.drops(), 0);
    drop(o);
    close(&p, 2000).await.unwrap();
    f.stop().await;
    assert!(last.data_ref().unwrap().as_ptr() == pointer);
    assert_eq!(probe.drops(), 0);
    drop(last);
    assert_eq!(probe.drops(), 1);
}
