//! Actual Hyper/native socket counterexample to treating a header flush as
//! completion of the response's buffered body. Observers only delegate IO.
use super::*;
use hyper::rt::{Read, Write};
use std::sync::atomic::AtomicBool;

// Closed native facts, not payloads or inferred socket readiness. Values are
// saturated solely for diagnostics; every delegated IO result is unchanged.
#[derive(Default)]
struct NativeFacts {
    body_polls: AtomicUsize,
    write_calls: AtomicUsize,
    vectored_calls: AtomicUsize,
    write_ready_bytes: AtomicUsize,
    write_pending: AtomicUsize,
    write_errors: AtomicUsize,
    flush_ready: AtomicUsize,
    flush_pending: AtomicUsize,
    flush_errors: AtomicUsize,
}
impl NativeFacts {
    fn add(counter: &AtomicUsize, value: usize, maximum: usize) {
        let mut old = counter.load(Ordering::SeqCst);
        loop {
            let next = old.saturating_add(value).min(maximum);
            match counter.compare_exchange_weak(old, next, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => break,
                Err(current) => old = current,
            }
        }
    }
    fn write(&self, result: &Poll<std::io::Result<usize>>, vectored: bool) {
        Self::add(&self.write_calls, 1, 65535);
        if vectored {
            Self::add(&self.vectored_calls, 1, 65535);
        }
        match result {
            Poll::Pending => Self::add(&self.write_pending, 1, 65535),
            Poll::Ready(Ok(count)) => Self::add(&self.write_ready_bytes, *count, 64 * 1024 * 1024),
            Poll::Ready(Err(_)) => Self::add(&self.write_errors, 1, 65535),
        }
    }
}

struct BodyWitness {
    inner: BufferedBody,
    transferred: Arc<AtomicBool>,
    facts: Arc<NativeFacts>,
}
impl hyper::body::Body for BodyWitness {
    type Data = Bytes;
    type Error = Infallible;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Bytes>, Infallible>>> {
        NativeFacts::add(&self.facts.body_polls, 1, 65535);
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
    facts: Arc<NativeFacts>,
}
#[cfg(windows)]
const WINDOWS_NATIVE_WRITE_LIMIT: usize = 4096;

// Keep each Windows native poll to one bounded prefix and remove only Tokio's
// cooperative-budget yield from that poll. Return the inner poll result as-is.
#[cfg(windows)]
fn poll_windows_scalar_once(
    inner: &mut TokioIo<TcpStream>,
    cx: &mut Context<'_>,
    bytes: &[u8],
) -> Poll<std::io::Result<usize>> {
    let prefix = &bytes[..bytes.len().min(WINDOWS_NATIVE_WRITE_LIMIT)];
    assert!(prefix.len() <= WINDOWS_NATIVE_WRITE_LIMIT);
    let native = tokio::task::coop::unconstrained(std::future::poll_fn(|cx| {
        Pin::new(&mut *inner).poll_write(cx, prefix)
    }));
    tokio::pin!(native);
    native.as_mut().poll(cx)
}

#[cfg(windows)]
fn poll_windows_vectored_native_once(
    inner: &mut TokioIo<TcpStream>,
    cx: &mut Context<'_>,
    buffers: &[std::io::IoSlice<'_>],
) -> Poll<std::io::Result<usize>> {
    assert!(
        buffers
            .iter()
            .all(|buffer| buffer.len() <= WINDOWS_NATIVE_WRITE_LIMIT)
            && (buffers.len() <= 1 || buffers.iter().all(|buffer| buffer.is_empty())),
        "Windows vectored native write must be a single bounded prefix"
    );
    let native = tokio::task::coop::unconstrained(std::future::poll_fn(|cx| {
        Pin::new(&mut *inner).poll_write_vectored(cx, buffers)
    }));
    tokio::pin!(native);
    native.as_mut().poll(cx)
}

#[cfg(windows)]
fn poll_windows_vectored_once(
    inner: &mut TokioIo<TcpStream>,
    cx: &mut Context<'_>,
    buffers: &[std::io::IoSlice<'_>],
) -> Poll<std::io::Result<usize>> {
    let Some(first) = buffers.iter().find(|buffer| !buffer.is_empty()) else {
        // Empty input delegates zero bytes and preserves the native empty-write result.
        return poll_windows_vectored_native_once(inner, cx, buffers);
    };
    let prefix = &first[..first.len().min(WINDOWS_NATIVE_WRITE_LIMIT)];
    let bounded = [std::io::IoSlice::new(prefix)];
    poll_windows_vectored_native_once(inner, cx, &bounded)
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
        #[cfg(windows)]
        let result = poll_windows_scalar_once(&mut self.inner, cx, bytes);
        #[cfg(not(windows))]
        let result = Pin::new(&mut self.inner).poll_write(cx, bytes);
        self.facts.write(&result, false);
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
        #[cfg(windows)]
        let result = poll_windows_vectored_once(&mut self.inner, cx, buffers);
        #[cfg(not(windows))]
        let result = Pin::new(&mut self.inner).poll_write_vectored(cx, buffers);
        self.facts.write(&result, true);
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
        match &result {
            Poll::Pending => NativeFacts::add(&self.facts.flush_pending, 1, 65535),
            Poll::Ready(Ok(())) => NativeFacts::add(&self.facts.flush_ready, 1, 65535),
            Poll::Ready(Err(_)) => NativeFacts::add(&self.facts.flush_errors, 1, 65535),
        }
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

// Configure the live accepted socket without taking or rebuilding ownership.
// Listener inheritance alone is not a measured accepted-socket premise.
#[cfg(windows)]
fn accepted_send_buffer(peer: &TcpStream) -> std::io::Result<(i32, i32)> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        getsockopt, setsockopt, WSAGetLastError, SOCKET, SOCKET_ERROR, SOL_SOCKET, SO_SNDBUF,
    };
    fn read(socket: SOCKET) -> std::io::Result<i32> {
        let mut value = 0_i32;
        let mut length = std::mem::size_of_val(&value) as i32;
        // SAFETY: the borrowed live socket is valid; value/length are writable
        // for the indicated sizes and remain live for this synchronous call.
        if unsafe {
            getsockopt(
                socket,
                SOL_SOCKET,
                SO_SNDBUF,
                (&mut value as *mut i32).cast(),
                &mut length,
            )
        } == SOCKET_ERROR
        {
            return Err(std::io::Error::from_raw_os_error(unsafe {
                WSAGetLastError()
            }));
        }
        if length != std::mem::size_of_val(&value) as i32 || value <= 0 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        Ok(value)
    }
    let socket = peer.as_raw_socket() as SOCKET;
    let before = read(socket)?;
    let requested = 4096_i32;
    // SAFETY: peer retains socket ownership and the initialized option value
    // remains live for this synchronous call; setsockopt retains no pointer.
    if unsafe {
        setsockopt(
            socket,
            SOL_SOCKET,
            SO_SNDBUF,
            (&requested as *const i32).cast(),
            std::mem::size_of_val(&requested) as i32,
        )
    } == SOCKET_ERROR
    {
        return Err(std::io::Error::from_raw_os_error(unsafe {
            WSAGetLastError()
        }));
    }
    Ok((before, read(socket)?))
}

#[tokio::test]
async fn header_flush_before_buffered_frame_keeps_original_send_end() {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_send_buffer_size(4096).unwrap();
    let listener_send = socket.send_buffer_size().unwrap().min(262145);
    socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let listener = socket.listen(128).unwrap();
    let client_socket = tokio::net::TcpSocket::new_v4().unwrap();
    client_socket.set_recv_buffer_size(4096).unwrap();
    let client_recv = client_socket.recv_buffer_size().unwrap().min(262145);
    let mut client = BufReader::new(
        client_socket
            .connect(listener.local_addr().unwrap())
            .await
            .unwrap(),
    );
    let (peer, _) = listener.accept().await.unwrap();
    #[cfg(windows)]
    let (accepted_send_before, accepted_send_after) = {
        let values = accepted_send_buffer(&peer).unwrap();
        assert_eq!(values.1, 4096, "accepted socket send buffer readback");
        values
    };
    #[cfg(not(windows))]
    let (accepted_send_before, accepted_send_after) = (0_i32, 0_i32);
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
    let facts = Arc::new(NativeFacts::default());
    let end = tokio::time::Instant::now() + Duration::from_millis(80);
    let (sending, sent) = tokio::sync::watch::channel(Sending::default());
    let service_sending = sending.clone();
    let reply = std::sync::Mutex::new(Some(reply.map(|inner| BodyWitness {
        inner,
        transferred: Arc::clone(&transferred),
        facts: Arc::clone(&facts),
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
            facts: Arc::clone(&facts),
        },
        sending,
        probe: None,
    };
    let mut builder = http1::Builder::new();
    builder.max_buf_size(8192);
    let task = tokio::spawn(builder.serve_connection(io, service));
    send(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    // Read only the actual headers, leaving the large body backpressured.
    // Preserve the 2s watchdog and all required native premises. Cleanup and
    // actual task join precede every assertion, including a premise timeout.
    let mut phase = 1_u8; // 1=headers, 2=native Pending, 3=all premises reached.
    let mut header_bytes = 0_usize;
    let observation = tokio::time::timeout(Duration::from_secs(2), async {
        let mut total = 0;
        loop {
            let mut line = Vec::new();
            let count = match client.read_until(b'\n', &mut line).await {
                Ok(count) => count,
                Err(_) => return Err(1_u8), // Native read error, never its text.
            };
            if count == 0 {
                return Err(2_u8); // Native EOF before all headers.
            }
            total += line.len();
            header_bytes = total.min(16384);
            if total >= 16 * 1024 {
                return Err(3_u8); // Same original strict header bound.
            }
            if line == b"\r\n" {
                break;
            }
        }
        phase = 2;
        while !pending.load(Ordering::SeqCst) {
            changed.notified().await;
        }
        phase = 3;
        Ok(())
    })
    .await;
    let observed_end = sent.borrow().deadline;
    let saw_header_flush = header_flush.load(Ordering::SeqCst);
    let saw_frame = transferred.load(Ordering::SeqCst);
    let saw_native_pending = pending.load(Ordering::SeqCst);
    let task_finished = task.is_finished();
    let end_spent = tokio::time::Instant::now() >= end;
    task.abort();
    let joined = task.await;
    let actual_cancelled = joined.as_ref().is_err_and(|error| error.is_cancelled());
    drop(client);
    drop(listener);
    if !matches!(observation, Ok(Ok(())))
        || !actual_cancelled
        || !(saw_header_flush && saw_frame && saw_native_pending)
        || observed_end != Some(end)
    {
        let outcome = match &observation {
            Ok(Ok(())) => 0,
            Ok(Err(kind)) => *kind,
            Err(_) => 4, // Watchdog elapsed; not the response's send deadline.
        };
        let end_state = match observed_end {
            None => 0,
            Some(value) if value == end => 1,
            Some(_) => 2,
        };
        let join_state = match &joined {
            Err(error) if error.is_cancelled() => 0,
            Err(error) if error.is_panic() => 1,
            Err(_) => 2,
            Ok(Ok(())) => 3,
            Ok(Err(_)) => 4,
        };
        eprintln!(
            "send_deadline_native_diag phase={} outcome={} header_bytes={} listener_send={} client_recv={} accepted_send_verified={} accepted_send_before={} accepted_send_after={} body_polls={} frame={} header_flush={} write_calls={} vectored_calls={} write_ready_bytes={} write_pending={} write_errors={} flush_ready={} flush_pending={} flush_errors={} native_pending={} end_state={} end_spent={} task_finished={} join={}",
            phase, outcome, header_bytes, listener_send, client_recv,
            u8::from(cfg!(windows)), accepted_send_before.min(262145), accepted_send_after.min(262145),
            facts.body_polls.load(Ordering::SeqCst), u8::from(saw_frame), u8::from(saw_header_flush),
            facts.write_calls.load(Ordering::SeqCst), facts.vectored_calls.load(Ordering::SeqCst),
            facts.write_ready_bytes.load(Ordering::SeqCst), facts.write_pending.load(Ordering::SeqCst),
            facts.write_errors.load(Ordering::SeqCst), facts.flush_ready.load(Ordering::SeqCst),
            facts.flush_pending.load(Ordering::SeqCst), facts.flush_errors.load(Ordering::SeqCst),
            u8::from(saw_native_pending), end_state, u8::from(end_spent),
            u8::from(task_finished), join_state,
        );
    }
    assert!(
        matches!(observation, Ok(Ok(()))),
        "actual socket premise observation failed"
    );
    assert!(actual_cancelled);
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
