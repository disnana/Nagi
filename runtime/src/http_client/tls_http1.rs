//! Private, single-use TLS/HTTP1 transport over a checked numeric socket.
//!
//! Trusted catalog and auth create the identity, host and operation budgets;
//! this module cannot issue a Grant or manufacture a checked DNS snapshot.
//! `establish` returns the Hyper connection future to the retained parent
//! ledger. It does not spawn a driver, and its response is provisional until
//! that ledger observes the driver's actual join and the final publish gate.

use super::*;
use hyper::{
    body::{Body, Bytes, Frame, SizeHint},
    client::conn::http1::{self, Connection, SendRequest},
    header::{ACCEPT_ENCODING, CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, HOST},
    HeaderMap, Method, Request, StatusCode, Uri,
};
use hyper_util::rt::TokioIo;
use rustls::{pki_types::ServerName, ClientConfig, RootCertStore};
use std::{
    convert::Infallible,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_rustls::{client::TlsStream, TlsConnector};
use url::Host;

const MAX_HEADERS: usize = 128;
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MIN_HYPER_BUF: usize = 8192;
const MAX_HYPER_BUF: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_HOST_BYTES: usize = 253;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransportLimits {
    max_headers: usize,
    max_header_bytes: usize,
    max_buf_size: usize,
    max_request_bytes: usize,
    max_response_bytes: usize,
    max_path_bytes: usize,
}

impl TransportLimits {
    fn checked(
        max_headers: usize,
        max_header_bytes: usize,
        max_buf_size: usize,
        max_request_bytes: usize,
        max_response_bytes: usize,
        max_path_bytes: usize,
    ) -> Result<Self, TransportError> {
        if !(1..=MAX_HEADERS).contains(&max_headers)
            || !(1..=MAX_HEADER_BYTES).contains(&max_header_bytes)
            || !(MIN_HYPER_BUF..=MAX_HYPER_BUF).contains(&max_buf_size)
            || !(1..=MAX_BODY_BYTES).contains(&max_request_bytes)
            || !(1..=MAX_BODY_BYTES).contains(&max_response_bytes)
            || !(1..=MAX_PATH_BYTES).contains(&max_path_bytes)
        {
            return Err(TransportError::InvalidProfile);
        }
        Ok(Self {
            max_headers,
            max_header_bytes,
            max_buf_size,
            max_request_bytes,
            max_response_bytes,
            max_path_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportError {
    Socket(SocketError),
    InvalidProfile,
    IdentityMismatch,
    Deadline,
    Tls,
    Http,
    Capacity,
    HeaderLimit,
    BodyLimit,
    UnsupportedEncoding,
    Upgrade,
}

impl From<SocketError> for TransportError {
    fn from(error: SocketError) -> Self {
        Self::Socket(error)
    }
}

struct TlsProfile {
    identity: ServiceIdentity,
    server_name: ServerName<'static>,
    authority: hyper::header::HeaderValue,
    port: u16,
    config: Arc<ClientConfig>,
    whole_end: Instant,
    tls_end: Instant,
    limits: TransportLimits,
}

impl TlsProfile {
    // `original_host` and `roots` must come from a trusted, frozen catalog.
    // Fixture-specific roots are constructed only inside this file's tests.
    fn checked(
        identity: ServiceIdentity,
        original_host: Host<String>,
        port: u16,
        roots: RootCertStore,
        whole_end: Instant,
        tls_end: Instant,
        limits: TransportLimits,
    ) -> Result<Self, TransportError> {
        if port == 0
            || Instant::now() >= whole_end
            || Instant::now() >= tls_end
            || tls_end > whole_end
        {
            return Err(TransportError::InvalidProfile);
        }
        let (name, host_header, expect_ip) = match original_host {
            Host::Domain(name) => {
                if name.is_empty()
                    || name.len() > MAX_HOST_BYTES
                    || !name.is_ascii()
                    || name.ends_with('.')
                    || Host::parse(&name).ok() != Some(Host::Domain(name.clone()))
                {
                    return Err(TransportError::InvalidProfile);
                }
                (name.clone(), name, false)
            }
            Host::Ipv4(address) => {
                let text = address.to_string();
                (text.clone(), text, true)
            }
            Host::Ipv6(address) => (address.to_string(), format!("[{address}]"), true),
        };
        let server_name = ServerName::try_from(name).map_err(|_| TransportError::InvalidProfile)?;
        // Rustls interprets a typed IP ServerName as an IP SAN identity and
        // suppresses SNI. DNS origins retain their original canonical name.
        if expect_ip != matches!(server_name, ServerName::IpAddress(_)) {
            return Err(TransportError::InvalidProfile);
        }
        let authority = if port == 443 {
            host_header
        } else {
            format!("{host_header}:{port}")
        };
        let authority = hyper::header::HeaderValue::from_str(&authority)
            .map_err(|_| TransportError::InvalidProfile)?;
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let mut config = ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
            .map_err(|_| TransportError::InvalidProfile)?
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        config.enable_sni = true;
        config.enable_early_data = false;
        config.resumption = rustls::client::Resumption::disabled();
        Ok(Self {
            identity,
            server_name,
            authority,
            port,
            config: Arc::new(config),
            whole_end,
            tls_end,
            limits,
        })
    }
}

struct GatedIo {
    socket: PinnedSocket,
    control: Arc<DialControl>,
    whole_end: Instant,
}

fn io_gate(
    control: &DialControl,
    whole_end: Instant,
) -> io::Result<std::sync::MutexGuard<'_, StopState>> {
    let state = control
        .checked_state()
        .map_err(|_| io::Error::new(io::ErrorKind::ConnectionAborted, "private socket closed"))?;
    if Instant::now() >= whole_end {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "private socket deadline",
        ));
    }
    Ok(state)
}

impl AsyncRead for GatedIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let _state = io_gate(&this.control, this.whole_end)?;
        Pin::new(
            this.socket
                .owned
                .stream
                .as_mut()
                .expect("pinned socket owns raw stream"),
        )
        .poll_read(cx, buf)
    }
}

impl AsyncWrite for GatedIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let _state = io_gate(&this.control, this.whole_end)?;
        Pin::new(
            this.socket
                .owned
                .stream
                .as_mut()
                .expect("pinned socket owns raw stream"),
        )
        .poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let _state = io_gate(&this.control, this.whole_end)?;
        Pin::new(
            this.socket
                .owned
                .stream
                .as_mut()
                .expect("pinned socket owns raw stream"),
        )
        .poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let _state = io_gate(&this.control, this.whole_end)?;
        Pin::new(
            this.socket
                .owned
                .stream
                .as_mut()
                .expect("pinned socket owns raw stream"),
        )
        .poll_shutdown(cx)
    }
}

struct RequestBody(Option<Bytes>);

impl Body for RequestBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(self.0.take().map(|bytes| Ok(Frame::data(bytes))))
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.0.as_ref().map_or(0, Bytes::len) as u64)
    }
}

struct WireRequest {
    method: Method,
    uri: Uri,
    body: Vec<u8>,
}

impl WireRequest {
    // `uri` is the already encoded origin-form URI of a trusted frozen
    // operation. This layer does not parse an untrusted URL or follow Location.
    fn checked(
        method: Method,
        uri: Uri,
        body: Vec<u8>,
        limits: &TransportLimits,
    ) -> Result<Self, TransportError> {
        let path = uri
            .path_and_query()
            .ok_or(TransportError::InvalidProfile)?
            .as_str();
        if path.len() > limits.max_path_bytes
            || !path.starts_with('/')
            || uri.scheme().is_some()
            || uri.authority().is_some()
            || body.len() > limits.max_request_bytes
        {
            return Err(TransportError::InvalidProfile);
        }
        Ok(Self { method, uri, body })
    }

    fn into_hyper(
        self,
        authority: hyper::header::HeaderValue,
    ) -> Result<Request<RequestBody>, TransportError> {
        let len = self.body.len();
        Request::builder()
            .method(self.method)
            .uri(self.uri)
            .header(HOST, authority)
            .header(CONNECTION, "close")
            .header(ACCEPT_ENCODING, "identity")
            .header(CONTENT_LENGTH, len.to_string())
            .body(RequestBody(Some(Bytes::from(self.body))))
            .map_err(|_| TransportError::InvalidProfile)
    }
}

type RawDriver = Connection<TokioIo<TlsStream<GatedIo>>, RequestBody>;

struct Http1Parts {
    client: Http1Client,
    // The caller must register/spawn this through its retained ledger gate and
    // observe the actual JoinHandle before publishing any provisional result.
    driver: RawDriver,
}

struct Http1Client {
    sender: SendRequest<RequestBody>,
    control: Arc<DialControl>,
    authority: hyper::header::HeaderValue,
    whole_end: Instant,
    limits: TransportLimits,
}

struct BoundedResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

fn phase_gate(control: &DialControl, end: Instant) -> Result<(), TransportError> {
    let _state = control.checked_state()?;
    if Instant::now() >= end {
        return Err(TransportError::Deadline);
    }
    Ok(())
}

async fn establish(
    pinned: PinnedSocket,
    profile: &TlsProfile,
    control: Arc<DialControl>,
) -> Result<Http1Parts, TransportError> {
    if pinned.identity != profile.identity
        || pinned.operation_end != profile.whole_end
        || pinned.selected.port() != profile.port
    {
        return Err(TransportError::IdentityMismatch);
    }
    pinned.ready_for_tls(&control)?;
    let tls_end = profile.tls_end.min(profile.whole_end);
    phase_gate(&control, tls_end)?;
    let connector = TlsConnector::from(Arc::clone(&profile.config));
    let gated_io = GatedIo {
        socket: pinned,
        control: Arc::clone(&control),
        whole_end: profile.whole_end,
    };
    let mut stopped = control.changed.subscribe();
    // `PinnedSocket` is moved whole into the TLS future: the raw stream and
    // its quota ticket cannot be split or released on successful handshake.
    let tls = tokio::select! {
        biased;
        reason = wait_stop(&mut stopped) => return Err(reason.into()),
        _ = tokio::time::sleep_until(tls_end.into()) => return Err(TransportError::Deadline),
        result = connector.connect(profile.server_name.clone(), gated_io) => result.map_err(|_| TransportError::Tls)?,
    };
    phase_gate(&control, profile.whole_end)?;
    let mut builder = http1::Builder::new();
    builder.max_headers(profile.limits.max_headers);
    // Constructor validated >=8192; do not call read_buf_exact_size, which
    // would silently unset this parser buffer bound.
    builder.max_buf_size(profile.limits.max_buf_size);
    let (sender, driver) = tokio::select! {
        biased;
        reason = wait_stop(&mut stopped) => return Err(reason.into()),
        _ = tokio::time::sleep_until(profile.whole_end.into()) => return Err(TransportError::Deadline),
        result = builder.handshake(TokioIo::new(tls)) => result.map_err(|_| TransportError::Http)?,
    };
    phase_gate(&control, profile.whole_end)?;
    Ok(Http1Parts {
        client: Http1Client {
            sender,
            control,
            authority: profile.authority.clone(),
            whole_end: profile.whole_end,
            limits: profile.limits,
        },
        driver,
    })
}

fn count_headers(
    headers: &HeaderMap,
    limits: &TransportLimits,
    used_count: &mut usize,
    used_bytes: &mut usize,
) -> Result<(), TransportError> {
    *used_count = used_count
        .checked_add(headers.len())
        .ok_or(TransportError::HeaderLimit)?;
    if *used_count > limits.max_headers {
        return Err(TransportError::HeaderLimit);
    }
    for (name, value) in headers {
        *used_bytes = used_bytes
            .checked_add(name.as_str().len())
            .and_then(|sum| sum.checked_add(value.as_bytes().len()))
            .ok_or(TransportError::HeaderLimit)?;
        if *used_bytes > limits.max_header_bytes {
            return Err(TransportError::HeaderLimit);
        }
    }
    Ok(())
}

fn enqueue_under_gate<T>(
    control: &DialControl,
    end: Instant,
    enqueue: impl FnOnce() -> T,
) -> Result<T, TransportError> {
    let _state = control.checked_state().map_err(TransportError::Socket)?;
    if Instant::now() >= end {
        return Err(TransportError::Deadline);
    }
    Ok(enqueue())
}

async fn send_request_gated(
    sender: &mut SendRequest<RequestBody>,
    request: Request<RequestBody>,
    control: &DialControl,
    end: Instant,
) -> Result<hyper::Response<hyper::body::Incoming>, TransportError> {
    // Hyper 1.11.1 calls dispatch.send(req) *inside this method*, before its
    // returned Future is polled. This async helper is lazy; its first poll
    // invokes the synchronous enqueue under the close/deadline gate.
    let response = enqueue_under_gate(control, end, || sender.send_request(request))?;
    response.await.map_err(|_| TransportError::Http)
}

impl Http1Client {
    // One request only; a 3xx Location is returned as metadata, never followed.
    // Completion here is provisional until the retained parent joins `driver`.
    async fn send_once(mut self, request: WireRequest) -> Result<BoundedResponse, TransportError> {
        phase_gate(&self.control, self.whole_end)?;
        let request = request.into_hyper(self.authority.clone())?;
        let mut stopped = self.control.changed.subscribe();
        let response = tokio::select! {
            biased;
            reason = wait_stop(&mut stopped) => return Err(reason.into()),
            _ = tokio::time::sleep_until(self.whole_end.into()) => return Err(TransportError::Deadline),
            result = send_request_gated(&mut self.sender, request, &self.control, self.whole_end) => result?,
        };
        phase_gate(&self.control, self.whole_end)?;
        if response.status() == StatusCode::SWITCHING_PROTOCOLS {
            return Err(TransportError::Upgrade);
        }
        let (parts, incoming) = response.into_parts();
        tokio::pin!(incoming);
        let mut header_count = 0;
        let mut header_bytes = 0;
        count_headers(
            &parts.headers,
            &self.limits,
            &mut header_count,
            &mut header_bytes,
        )?;
        for value in parts.headers.get_all(CONTENT_ENCODING).iter() {
            if !value.as_bytes().eq_ignore_ascii_case(b"identity") {
                return Err(TransportError::UnsupportedEncoding);
            }
        }
        let mut body = Vec::new();
        loop {
            let next = tokio::select! {
                biased;
                reason = wait_stop(&mut stopped) => return Err(reason.into()),
                _ = tokio::time::sleep_until(self.whole_end.into()) => return Err(TransportError::Deadline),
                next = std::future::poll_fn(|cx| incoming.as_mut().poll_frame(cx)) => next,
            };
            let Some(frame) = next else {
                break;
            };
            let frame = frame.map_err(|_| TransportError::Http)?;
            match frame.into_data() {
                Ok(data) => {
                    let total = body
                        .len()
                        .checked_add(data.len())
                        .ok_or(TransportError::BodyLimit)?;
                    if total > self.limits.max_response_bytes {
                        return Err(TransportError::BodyLimit);
                    }
                    body.try_reserve(data.len())
                        .map_err(|_| TransportError::Capacity)?;
                    body.extend_from_slice(&data);
                }
                Err(frame) => {
                    if let Ok(trailers) = frame.into_trailers() {
                        count_headers(
                            &trailers,
                            &self.limits,
                            &mut header_count,
                            &mut header_bytes,
                        )?;
                    }
                }
            }
        }
        phase_gate(&self.control, self.whole_end)?;
        Ok(BoundedResponse {
            status: parts.status,
            headers: parts.headers,
            body,
        })
    }
}

#[cfg(test)]
#[path = "tls_http1_tests.rs"]
mod tests;
