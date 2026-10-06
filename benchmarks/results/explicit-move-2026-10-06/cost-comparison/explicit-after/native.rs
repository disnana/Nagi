pub async fn pause() { std::future::pending::<()>().await; }
pub fn observe() {
    use std::future::Future;
    let mut future = std::pin::pin!(super::arrange("Nagi"));
    let size = std::mem::size_of_val(future.as_ref().get_ref());
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(future.as_mut().poll(&mut cx), std::task::Poll::Pending));
    println!("{{\"future_bytes\":{size}}}");
}
