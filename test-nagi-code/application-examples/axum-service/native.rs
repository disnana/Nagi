use axum::{
    extract::{DefaultBodyLimit, Json},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::{net::Ipv4Addr, time::Duration};

// This timer demonstrates Nagi awaiting a Rust async operation. It does not
// simulate a database or enforce a request deadline.
pub async fn pause() {
    tokio::time::sleep(Duration::from_millis(1)).await;
}

async fn quote(Json(input): Json<super::QuoteInput>) -> Response {
    // The adapter calls one known generated function by name, then awaits it.
    // It is not a generic async callback passed through Nagi extern.
    match super::calculate(input).await {
        Ok(output) => Json(output).into_response(),
        Err(super::QuoteError::InvalidQuantity) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(nagi_runtime::serde_json::json!({
                "code": "invalid_quantity",
                "message": "Quantity must be between 1 and 100",
            })),
        )
            .into_response(),
    }
}

pub async fn run_server(port: i64) -> Result<(), nagi_runtime::Error> {
    let port = u16::try_from(port)
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| nagi_runtime::Error::invalid("port must be between 1 and 65535"))?;
    let router = Router::new()
        .route("/health", get(|| async { "ok\n" }))
        .route("/quotes", post(quote))
        .layer(DefaultBodyLimit::max(4096));
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    println!("Axum service listening http://127.0.0.1:{port}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("Ctrl+C listener failed: {error}");
            }
        })
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body, Bytes},
        extract::Request,
        http::header::{CONNECTION, CONTENT_TYPE},
    };
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll},
    };
    use tokio::sync::oneshot;

    // Observe the owned read future, not Hyper's private tasks or OS resources.
    // Pin<Box<F>> lets this wrapper delegate without unsafe or new dependencies.
    struct ObservedRead<F> {
        future: Pin<Box<F>>,
        polls: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
        first_poll: Option<oneshot::Sender<()>>,
    }

    impl<F: Future> Future for ObservedRead<F> {
        type Output = F::Output;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.polls.fetch_add(1, Ordering::SeqCst);
            if let Some(signal) = self.first_poll.take() {
                let _ = signal.send(());
            }
            self.future.as_mut().poll(cx)
        }
    }

    impl<F> Drop for ObservedRead<F> {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct ReadObservation {
        polls: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
        first_poll: oneshot::Receiver<()>,
    }

    fn observe<F: Future>(future: F) -> (ObservedRead<F>, ReadObservation) {
        let polls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let (sender, first_poll) = oneshot::channel();
        (
            ObservedRead {
                future: Box::pin(future),
                polls: Arc::clone(&polls),
                drops: Arc::clone(&drops),
                first_poll: Some(sender),
            },
            ReadObservation {
                polls,
                drops,
                first_poll,
            },
        )
    }

    async fn assert_original_415(response: Response, closes: bool) {
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        if closes {
            assert_eq!(response.headers().get(CONNECTION).unwrap(), "close");
        } else {
            assert!(response.headers().get(CONNECTION).is_none());
        }
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(
            bytes.as_ref(),
            b"Expected request with `Content-Type: application/json`"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_content_type_reads_eof_and_4096_without_forcing_close() {
        for bytes in [Vec::new(), b"{\"quantity\":1}".to_vec(), vec![b'x'; 4096]] {
            let (read, observed) = observe(to_bytes(Body::from(bytes), 4096));
            let response = missing_content_type_response(read).await;
            assert!(observed.polls.load(Ordering::SeqCst) > 0);
            assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
            assert_original_415(response, false).await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_content_type_keeps_415_on_size_limit_and_drops_read() {
        let (read, observed) = observe(to_bytes(Body::from(vec![b'x'; 4097]), 4096));
        let response = missing_content_type_response(read).await;
        assert!(observed.polls.load(Ordering::SeqCst) > 0);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        assert_original_415(response, true).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_content_type_keeps_415_on_read_error_and_drops_read() {
        let (read, observed) = observe(async {
            Err::<Bytes, _>(axum::Error::new(std::io::Error::other(
                "controlled read failure",
            )))
        });
        let response = missing_content_type_response(read).await;
        assert!(observed.polls.load(Ordering::SeqCst) > 0);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        assert_original_415(response, true).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_content_type_waits_for_eof_before_returning_415() {
        let (complete, completion) = oneshot::channel();
        let (read, observed) = observe(async { completion.await.unwrap() });
        let task = tokio::spawn(missing_content_type_response(read));
        tokio::time::timeout(Duration::from_secs(3), observed.first_poll)
            .await
            .unwrap()
            .unwrap();
        assert!(observed.polls.load(Ordering::SeqCst) > 0);
        assert!(
            !task.is_finished(),
            "response returned before controlled EOF"
        );
        assert_eq!(observed.drops.load(Ordering::SeqCst), 0);
        complete
            .send(to_bytes(Body::from(b"{\"quantity\":1}".as_slice()), 4096).await)
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        assert_original_415(response, false).await;
    }

    fn pending_owned_body() -> impl Future<Output = Result<Bytes, axum::Error>> {
        // The canceled future owns an actual Body across the pending await.
        let body = Body::from("{\"quantity\":1}");
        async move {
            let result = std::future::pending::<Result<Bytes, axum::Error>>().await;
            drop(body);
            result
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_content_type_deadline_keeps_415_and_drops_pending_read() {
        let (read, observed) = observe(pending_owned_body());
        let task = tokio::spawn(missing_content_type_response(read));
        tokio::time::timeout(Duration::from_secs(3), observed.first_poll)
            .await
            .unwrap()
            .unwrap();
        assert!(observed.polls.load(Ordering::SeqCst) > 0);
        assert!(!task.is_finished());
        assert_eq!(observed.drops.load(Ordering::SeqCst), 0);
        let response = tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .expect("bounded read did not return before the outer test deadline")
            .unwrap();
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        assert_original_415(response, true).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelling_missing_content_type_drops_pending_read() {
        let (read, observed) = observe(pending_owned_body());
        let task = tokio::spawn(missing_content_type_response(read));
        tokio::time::timeout(Duration::from_secs(3), observed.first_poll)
            .await
            .unwrap()
            .unwrap();
        assert!(observed.polls.load(Ordering::SeqCst) > 0);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 0);
        task.abort();
        let error = tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap_err();
        assert!(error.is_cancelled());
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn present_content_type_retains_json_and_business_responses() {
        // Direct handler calls exercise extraction/business status. The real
        // router smoke covers its DefaultBodyLimit and HTTP transport separately.
        for (body, status) in [
            ("{\"quantity\":1}", StatusCode::OK),
            ("{\"quantity\":0}", StatusCode::UNPROCESSABLE_ENTITY),
            ("{", StatusCode::BAD_REQUEST),
            ("{}", StatusCode::UNPROCESSABLE_ENTITY),
            ("{\"quantity\":\"1\"}", StatusCode::UNPROCESSABLE_ENTITY),
            (
                "{\"quantity\":1,\"coupon\":\"demo\"}",
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
        ] {
            let request = Request::builder()
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap();
            let response = quote(request).await;
            assert_eq!(response.status(), status, "{body}");
            assert!(response.headers().get(CONNECTION).is_none());
        }
        let unsupported = Request::builder()
            .header(CONTENT_TYPE, "text/plain")
            .body(Body::from("{\"quantity\":1}"))
            .unwrap();
        assert_original_415(quote(unsupported).await, false).await;
    }
}
