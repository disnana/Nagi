//! Actual Hyper/native socket counterexample to treating a header flush as
//! completion of the response's buffered body. Observers only delegate IO.
use super::*;
use hyper::rt::{Read, Write};
use std::sync::atomic::AtomicBool;

struct BodyWitness {
    inner: BufferedBody,
    transferred: Arc<AtomicBool>,
}
impl hyper::body::Body for BodyWitness {
    type Data = Bytes;
    type Error = Infallible;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Bytes>, Infallible>>> {
        let result = Pin::new(&mut self.inner).poll_frame(cx);
        if matches!(&result, Poll::Ready(Some(Ok(frame))) if frame.is_data()) {
            self.transferred.store(true, Ordering::SeqCst);
        }
        result
    }
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> hyper::body::SizeHint {
        self.inner.size_hint()
    }
}
struct IoWitness {
    inner: TokioIo<TcpStream>,
    transferred: Arc<AtomicBool>,
    header_flush: Arc<AtomicBool>,
    pending: Arc<AtomicBool>,
    changed: Arc<Notify>,
    sending: tokio::sync::watch::Receiver<Sending>,
}
impl Read for IoWitness {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
impl Write for IoWitness {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, bytes);
        if result.is_pending() && self.transferred.load(Ordering::SeqCst) {
            self.pending.store(true, Ordering::SeqCst);
            self.changed.notify_one();
        }
        result
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffers: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write_vectored(cx, buffers);
        if result.is_pending() && self.transferred.load(Ordering::SeqCst) {
            self.pending.store(true, Ordering::SeqCst);
            self.changed.notify_one();
        }
        result
    }
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        if matches!(result, Poll::Ready(Ok(())))
            && self.sending.borrow().deadline.is_some()
            && !self.transferred.load(Ordering::SeqCst)
        {
            self.header_flush.store(true, Ordering::SeqCst);
        }
        result
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[tokio::test]
async fn header_flush_before_buffered_frame_keeps_original_send_end() {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_send_buffer_size(4096).unwrap();
    socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let listener = socket.listen(128).unwrap();
    let client_socket = tokio::net::TcpSocket::new_v4().unwrap();
    client_socket.set_recv_buffer_size(4096).unwrap();
    let mut client = BufReader::new(
        client_socket
            .connect(listener.local_addr().unwrap())
            .await
            .unwrap(),
    );
    let (peer, _) = listener.accept().await.unwrap();
    let reply = append_header(
        representation(
            Status::OK,
            BodyKind::Binary,
            Bytes::from(vec![b'x'; STALLED_BODY_LENGTH]),
        ),
        "x-padding",
        &vec![b'p'; 8192],
    )
    .unwrap()
    .into_http(false);
    let transferred = Arc::new(AtomicBool::new(false));
    let header_flush = Arc::new(AtomicBool::new(false));
    let pending = Arc::new(AtomicBool::new(false));
    let changed = Arc::new(Notify::new());
    let end = tokio::time::Instant::now() + Duration::from_millis(80);
    let (sending, sent) = tokio::sync::watch::channel(Sending::default());
    let service_sending = sending.clone();
    let reply = std::sync::Mutex::new(Some(reply.map(|inner| BodyWitness {
        inner,
        transferred: Arc::clone(&transferred),
    })));
    let service = service_fn(move |_| {
        let mut reply = reply.lock().unwrap().take().unwrap();
        let sending = service_sending.clone();
        async move {
            sending.send_replace(Sending {
                deadline: Some(end),
                discard_remaining: false,
                body_transferred: false,
            });
            reply.body_mut().inner.sending = Some(sending);
            Ok::<_, Infallible>(reply)
        }
    });
    let io = TimedIo {
        inner: IoWitness {
            inner: TokioIo::new(peer),
            transferred: Arc::clone(&transferred),
            header_flush: Arc::clone(&header_flush),
            pending: Arc::clone(&pending),
            changed: Arc::clone(&changed),
            sending: sent.clone(),
        },
        sending,
        probe: None,
    };
    let mut builder = http1::Builder::new();
    builder.max_buf_size(8192);
    let task = tokio::spawn(builder.serve_connection(io, service));
    send(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    // Read only the actual headers, leaving the large body backpressured.
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut total = 0;
        loop {
            let mut line = Vec::new();
            assert_ne!(client.read_until(b'\n', &mut line).await.unwrap(), 0);
            total += line.len();
            assert!(total < 16 * 1024);
            if line == b"\r\n" {
                break;
            }
        }
        while !pending.load(Ordering::SeqCst) {
            changed.notified().await;
        }
    })
    .await
    .unwrap();
    let observed_end = sent.borrow().deadline;
    let saw_header_flush = header_flush.load(Ordering::SeqCst);
    let saw_frame = transferred.load(Ordering::SeqCst);
    let saw_native_pending = pending.load(Ordering::SeqCst);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(client);
    drop(listener);
    assert!(
        saw_header_flush && saw_frame && saw_native_pending,
        "actual header-only flush, frame transfer and native Pending required"
    );
    assert_eq!(
        observed_end,
        Some(end),
        "header-only flush cleared original response send end before the body write completed"
    );
}

#[tokio::test]
async fn empty_response_completion_does_not_expire_idle_keep_alive() {
    async fn reply(_: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        Ok(empty(Status::NO_CONTENT))
    }
    let app = route(app_default(()), Method::GET, "/", public_policy(), reply).unwrap();
    let probe = Arc::new(lifecycle_probe::Probe::default());
    let mut options = send_timeout(default_options(), 80).unwrap();
    options.lifecycle_probe = Some(Arc::clone(&probe));
    let server = Server::new(app, options).await;
    let mut client = server.connect().await;
    send(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    let first = response(&mut client, false).await;
    assert_eq!(first.status, 204);
    assert!(first.body.is_empty());
    let original_end = probe.first.get().unwrap().end;
    tokio::time::sleep_until(original_end + Duration::from_millis(1)).await;
    assert!(!probe.expired.load(Ordering::SeqCst));
    send(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    assert_eq!(response(&mut client, false).await.status, 204);
    drop(client);
    server.stop().await;
}

#[tokio::test]
async fn buffered_response_completion_does_not_expire_idle_keep_alive() {
    async fn reply(_: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        Ok(text(Status::OK, "ok"))
    }
    let app = route(app_default(()), Method::GET, "/", public_policy(), reply).unwrap();
    let probe = Arc::new(lifecycle_probe::Probe::default());
    let mut options = send_timeout(default_options(), 80).unwrap();
    options.lifecycle_probe = Some(Arc::clone(&probe));
    let server = Server::new(app, options).await;
    let mut client = server.connect().await;
    send(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    let first = response(&mut client, false).await;
    assert_eq!(first.status, 200);
    assert_eq!(first.body, b"ok");
    let original_end = probe.first.get().unwrap().end;
    tokio::time::sleep_until(original_end + Duration::from_millis(1)).await;
    assert!(!probe.expired.load(Ordering::SeqCst));
    send(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    let second = response(&mut client, false).await;
    assert_eq!(second.status, 200);
    assert_eq!(second.body, b"ok");
    drop(client);
    server.stop().await;
}
