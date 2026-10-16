//! Native wrappers compile independently of Hickory; never a resolver substitute.
use super::*;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpStream, UdpSocket},
};
pub(super) struct TrackedUdp {
    socket: Option<UdpSocket>,
    ticket: Option<IoTicket>,
    phase: DnsPhase,
}
impl TrackedUdp {
    pub(super) fn new(socket: UdpSocket, ticket: IoTicket, phase: DnsPhase) -> Self {
        Self {
            socket: Some(socket),
            ticket: Some(ticket),
            phase,
        }
    }
    pub(super) fn poll_recv_from(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        self.phase.poll_io(|book| {
            let size = buf.len().min(4096).min(WIRE_BYTES - book.received);
            if size == 0 {
                return Poll::Ready(Err(io::Error::other("DNS wire limit")));
            }
            let mut read = ReadBuf::new(&mut buf[..size]);
            match self
                .socket
                .as_ref()
                .expect("live socket")
                .poll_recv_from(cx, &mut read)
            {
                Poll::Ready(Ok(peer)) => {
                    let n = read.filled().len();
                    book.received += n;
                    if peer != self.phase.0.endpoint {
                        return Poll::Ready(Err(io::Error::other("DNS peer mismatch")));
                    }
                    Poll::Ready(Ok((n, peer)))
                }
                Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
                Poll::Pending => Poll::Pending,
            }
        })
    }
    pub(super) fn poll_send_to(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.phase.poll_io(|book| {
            if target != self.phase.0.endpoint
                || buf.len() > 4096
                || buf.len() > WIRE_BYTES - book.sent
            {
                return Poll::Ready(Err(io::Error::other("DNS send limit/endpoint")));
            }
            let result = self
                .socket
                .as_ref()
                .expect("live socket")
                .poll_send_to(cx, buf, target);
            if let Poll::Ready(Ok(n)) = result {
                book.sent += n;
            }
            result
        })
    }
}
impl Drop for TrackedUdp {
    fn drop(&mut self) {
        drop(self.socket.take());
        drop(self.ticket.take());
    }
}
pub(super) struct TrackedTcp {
    socket: Option<TcpStream>,
    ticket: Option<IoTicket>,
    phase: DnsPhase,
}
impl TrackedTcp {
    pub(super) fn new(socket: TcpStream, ticket: IoTicket, phase: DnsPhase) -> Self {
        Self {
            socket: Some(socket),
            ticket: Some(ticket),
            phase,
        }
    }
}
impl AsyncRead for TrackedTcp {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let phase = this.phase.clone();
        phase.poll_io(|book| {
            let count = buf.remaining().min(WIRE_BYTES - book.received);
            if count == 0 {
                return Poll::Ready(Err(io::Error::other("DNS wire limit")));
            }
            let mut native = ReadBuf::new(buf.initialize_unfilled_to(count));
            let result =
                Pin::new(this.socket.as_mut().expect("live socket")).poll_read(cx, &mut native);
            let n = native.filled().len();
            book.received += n;
            buf.advance(n);
            result
        })
    }
}
impl AsyncWrite for TrackedTcp {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let phase = this.phase.clone();
        phase.poll_io(|book| {
            if buf.len() > WIRE_BYTES - book.sent {
                return Poll::Ready(Err(io::Error::other("DNS wire limit")));
            }
            let result = Pin::new(this.socket.as_mut().expect("live socket")).poll_write(cx, buf);
            if let Poll::Ready(Ok(n)) = result {
                book.sent += n;
            }
            result
        })
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let phase = this.phase.clone();
        phase.poll_io(|_| Pin::new(this.socket.as_mut().expect("live socket")).poll_flush(cx))
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let phase = this.phase.clone();
        phase.poll_io(|_| Pin::new(this.socket.as_mut().expect("live socket")).poll_shutdown(cx))
    }
}
impl Drop for TrackedTcp {
    fn drop(&mut self) {
        drop(self.socket.take());
        drop(self.ticket.take());
    }
}
