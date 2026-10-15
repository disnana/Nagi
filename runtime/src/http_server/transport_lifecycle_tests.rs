//! Owned native terminal-close counterexample. No production clock changes.
use super::*;

#[tokio::test]
async fn complete_header_rejection_can_end_with_native_reset_without_reuse() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut client = BufReader::new(
        TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap(),
    );
    let (mut peer, _) = listener.accept().await.unwrap();
    peer.write_all(b"HTTP/1.1 431 Request Header Fields Too Large\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
    let received = response(&mut client, false).await;
    assert_eq!(received.status, 431);
    assert!(received.body.is_empty());
    peer.set_zero_linger().unwrap();
    drop(peer);
    // A complete rejected response remains exactly 431; only terminal TCP
    // disposition may be reset rather than FIN. No further bytes/reuse pass.
    assert!(terminal_close_after_abort(&mut client, 0).await.is_empty());
}

#[test]
fn timed_io_writes_and_pending_flush_preserve_the_original_send_end() {
    use hyper::rt::Write;
    struct Io {
        ready: bool,
    }
    impl Write for Io {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Ready(Ok(bytes.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            if self.ready {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }
    let end = tokio::time::Instant::now() + Duration::from_millis(80);
    let (sending, sent) = tokio::sync::watch::channel(Sending {
        deadline: Some(end),
        discard_remaining: true,
        body_transferred: true,
    });
    let mut io = TimedIo {
        inner: Io { ready: false },
        sending,
        probe: None,
        #[cfg(all(test, windows))]
        windows_stalled_fixture: false,
    };
    let mut context = Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        Pin::new(&mut io).poll_write(&mut context, b"headers/body"),
        Poll::Ready(Ok(12))
    ));
    assert_eq!(sent.borrow().deadline, Some(end));
    assert!(Pin::new(&mut io).poll_flush(&mut context).is_pending());
    assert_eq!(sent.borrow().deadline, Some(end));
    io.inner.ready = true;
    assert!(matches!(
        Pin::new(&mut io).poll_flush(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(sent.borrow().deadline, None);
    assert!(sent.borrow().discard_remaining);
}
