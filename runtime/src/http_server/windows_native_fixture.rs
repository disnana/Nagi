//! Shared Windows native socket premise for test-only send fixtures.
//! Prefixes alter write scheduling, never the result of the one delegated poll.
use hyper::rt::Write;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::TcpStream;

const WINDOWS_NATIVE_WRITE_LIMIT: usize = 4096;

// Preserve full native header writes before the actual buffered-frame handoff.
// After handoff, limit each Windows native write to one bounded prefix so body
// backpressure is observable. Remove only the single poll's cooperative yield;
// never manufacture an IO result. The separate Windows body gate controls only
// frame availability until the observed native header flush.
pub(super) fn poll_windows_scalar_once<T: Write + Unpin>(
    inner: &mut T,
    cx: &mut Context<'_>,
    bytes: &[u8],
    body_transferred: bool,
) -> Poll<std::io::Result<usize>> {
    let prefix = if body_transferred {
        &bytes[..bytes.len().min(WINDOWS_NATIVE_WRITE_LIMIT)]
    } else {
        bytes
    };
    assert!(!body_transferred || prefix.len() <= WINDOWS_NATIVE_WRITE_LIMIT);
    let native = tokio::task::coop::unconstrained(std::future::poll_fn(|cx| {
        Pin::new(&mut *inner).poll_write(cx, prefix)
    }));
    tokio::pin!(native);
    native.as_mut().poll(cx)
}

pub(super) fn poll_windows_vectored_native_once<T: Write + Unpin>(
    inner: &mut T,
    cx: &mut Context<'_>,
    buffers: &[std::io::IoSlice<'_>],
    body_transferred: bool,
) -> Poll<std::io::Result<usize>> {
    assert!(
        !body_transferred
            || (buffers
                .iter()
                .all(|buffer| buffer.len() <= WINDOWS_NATIVE_WRITE_LIMIT)
                && (buffers.len() <= 1 || buffers.iter().all(|buffer| buffer.is_empty()))),
        "post-frame Windows vectored native write must be a single bounded prefix"
    );
    let native = tokio::task::coop::unconstrained(std::future::poll_fn(|cx| {
        Pin::new(&mut *inner).poll_write_vectored(cx, buffers)
    }));
    tokio::pin!(native);
    native.as_mut().poll(cx)
}

pub(super) fn poll_windows_vectored_once<T: Write + Unpin>(
    inner: &mut T,
    cx: &mut Context<'_>,
    buffers: &[std::io::IoSlice<'_>],
    body_transferred: bool,
) -> Poll<std::io::Result<usize>> {
    if !body_transferred {
        return poll_windows_vectored_native_once(inner, cx, buffers, false);
    }
    let Some(first) = buffers.iter().find(|buffer| !buffer.is_empty()) else {
        // Empty input delegates zero bytes and preserves the native empty-write result.
        return poll_windows_vectored_native_once(inner, cx, buffers, true);
    };
    let prefix = &first[..first.len().min(WINDOWS_NATIVE_WRITE_LIMIT)];
    let bounded = [std::io::IoSlice::new(prefix)];
    poll_windows_vectored_native_once(inner, cx, &bounded, true)
}

// Configure the live accepted socket without taking or rebuilding ownership.
// Listener inheritance alone is not a measured accepted-socket premise.
pub(super) fn accepted_send_buffer(peer: &TcpStream) -> std::io::Result<(i32, i32)> {
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
