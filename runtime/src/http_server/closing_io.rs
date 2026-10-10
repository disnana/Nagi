//! Finite receive-side cleanup after Hyper has flushed a rejection response.
//! Only rejected requests with a possibly unread body enter cleanup. Discard
//! at most 8 KiB for min(100 ms, body deadline), then close the write side.
//! The original connection task retains its permit until cleanup completes;
//! normal responses and idle keep-alive do not enter this path. Discarded bytes
//! are never parsed or dispatched. This cannot guarantee response delivery.
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

pub(super) struct ClosingIo<T> {
    inner: T,
    timeout: Duration,
    remaining: usize,
    timer: Option<Pin<Box<tokio::time::Sleep>>>,
    state: tokio::sync::watch::Receiver<super::Sending>,
    drained: bool,
    finished: bool,
}
impl<T> ClosingIo<T> {
    pub(super) fn new(
        inner: T,
        timeout: Duration,
        state: tokio::sync::watch::Receiver<super::Sending>,
    ) -> Self {
        Self {
            inner,
            timeout: timeout.min(Duration::from_millis(100)),
            remaining: 8192,
            timer: None,
            state,
            drained: false,
            finished: false,
        }
    }
}
impl<T: hyper::rt::Read + Unpin> hyper::rt::Read for ClosingIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buffer)
    }
}
impl<T: hyper::rt::Read + hyper::rt::Write + Unpin> hyper::rt::Write for ClosingIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buffer)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffers: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, buffers)
    }
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if self.finished {
            return Poll::Ready(Ok(()));
        }
        if !self.drained && !self.state.borrow().discard_remaining {
            self.drained = true;
        }
        if !self.drained {
            if self.timer.is_none() {
                self.timer = Some(Box::pin(tokio::time::sleep(self.timeout)));
            }
            // Limit work per poll as well as total bytes; the absolute timer
            // keeps its original deadline across every Pending result.
            for _ in 0..8 {
                if self.remaining == 0 || self.timer.as_mut().unwrap().as_mut().poll(cx).is_ready()
                {
                    self.drained = true;
                    break;
                }
                let mut scratch = [0; 1024];
                let n = self.remaining.min(scratch.len());
                let mut buffer = hyper::rt::ReadBuf::new(&mut scratch[..n]);
                match Pin::new(&mut self.inner).poll_read(cx, buffer.unfilled()) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(_)) => {
                        self.drained = true;
                        break;
                    }
                    Poll::Ready(Ok(())) => {
                        let read = buffer.filled().len();
                        if read == 0 {
                            self.drained = true;
                            break;
                        }
                        self.remaining -= read;
                    }
                }
            }
            if !self.drained {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
        let result = Pin::new(&mut self.inner).poll_shutdown(cx);
        if result.is_ready() {
            self.finished = true;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::rt::Write;
    enum Reading {
        Bytes(usize),
        Pending,
        Error,
    }
    struct Probe {
        reading: Reading,
        shut: bool,
        reads: usize,
        bytes: usize,
        writes: usize,
    }
    impl Probe {
        fn new(reading: Reading) -> Self {
            Self {
                reading,
                shut: false,
                reads: 0,
                bytes: 0,
                writes: 0,
            }
        }
    }
    impl hyper::rt::Read for Probe {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            mut buffer: hyper::rt::ReadBufCursor<'_>,
        ) -> Poll<std::io::Result<()>> {
            assert!(
                !self.shut,
                "cleanup must finish before write-side shutdown exposes EOF"
            );
            self.reads += 1;
            match self.reading {
                Reading::Bytes(left) => {
                    let n = left.min(buffer.remaining());
                    buffer.put_slice(&vec![b'x'; n]);
                    self.bytes += n;
                    self.reading = Reading::Bytes(left - n);
                    Poll::Ready(Ok(()))
                }
                Reading::Pending => Poll::Pending,
                Reading::Error => Poll::Ready(Err(std::io::Error::from(
                    std::io::ErrorKind::ConnectionReset,
                ))),
            }
        }
    }
    impl hyper::rt::Write for Probe {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.writes += 1;
            Poll::Ready(Ok(buffer.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            self.shut = true;
            Poll::Ready(Ok(()))
        }
    }
    fn probing(reading: Reading, timeout: Duration, discard_remaining: bool) -> ClosingIo<Probe> {
        let (_, state) = tokio::sync::watch::channel(super::super::Sending {
            deadline: None,
            discard_remaining,
        });
        ClosingIo::new(Probe::new(reading), timeout, state)
    }
    async fn close(io: &mut ClosingIo<Probe>) {
        futures_util::future::poll_fn(|cx| Pin::new(&mut *io).poll_shutdown(cx))
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn completed_response_discards_small_remaining_input_before_exposing_eof() {
        let mut io = probing(Reading::Bytes(3), Duration::from_millis(100), true);
        close(&mut io).await;
        assert!(io.inner.shut);
        assert_eq!(io.inner.bytes, 3);
        assert_eq!(io.inner.reads, 2);
        assert_eq!(io.inner.writes, 0);
        close(&mut io).await;
        assert_eq!(io.inner.reads, 2, "completed cleanup is not restarted");
    }
    #[tokio::test]
    async fn cleanup_stops_at_its_byte_bound_without_waiting_for_full_upload() {
        let mut io = probing(Reading::Bytes(3), Duration::from_millis(100), true);
        io.remaining = 2;
        close(&mut io).await;
        assert_eq!(io.inner.bytes, 2);
        assert_eq!(io.inner.reads, 1);
    }
    #[tokio::test]
    async fn incomplete_input_does_not_restart_the_absolute_cleanup_deadline() {
        let mut io = probing(Reading::Pending, Duration::from_millis(10), true);
        let started = std::time::Instant::now();
        tokio::time::timeout(Duration::from_secs(1), close(&mut io))
            .await
            .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(10));
        assert!(io.inner.reads > 0);
        assert_eq!(io.timeout, Duration::from_millis(10));
    }
    #[tokio::test]
    async fn cleanup_read_error_does_not_generate_another_response() {
        let mut io = probing(Reading::Error, Duration::from_millis(100), true);
        close(&mut io).await;
        assert_eq!(io.inner.reads, 1);
        assert_eq!(io.inner.writes, 0);
    }
    #[tokio::test]
    async fn normal_shutdown_has_no_cleanup_reads_or_timer_allocation() {
        let mut io = probing(Reading::Pending, Duration::from_millis(100), false);
        let (result, counts) = crate::metrics::measure(|| {
            let waker = futures_util::task::noop_waker();
            Pin::new(&mut io).poll_shutdown(&mut Context::from_waker(&waker))
        });
        assert!(matches!(result, Poll::Ready(Ok(()))));
        assert!(io.inner.shut);
        assert_eq!(io.inner.reads, 0);
        assert!(io.timer.is_none());
        assert_eq!(counts.allocations, 0);
        assert_eq!(counts.reallocations, 0);
    }
}
