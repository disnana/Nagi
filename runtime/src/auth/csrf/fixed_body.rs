//! The original sealed-token JSON owner has exactly 60 inline payload bytes.
//! No String/Vec capacity or general Bytes adoption crosses this boundary.
pub(in crate::auth) const BODY_BYTES: usize = 60;
const TOKEN_BYTES: usize = 43;
const PREFIX: &[u8; 15] = b"{\"csrf_token\":\"";
const SUFFIX: &[u8; 2] = b"\"}";
const _: [(); BODY_BYTES] = [(); PREFIX.len() + TOKEN_BYTES + SUFFIX.len()];

pub(in crate::auth) struct FixedTokenBody {
    bytes: [u8; BODY_BYTES],
}
impl FixedTokenBody {
    pub(in crate::auth) fn new(token: &str) -> Option<Self> {
        // Both issuers mint canonical base64url tokens. Refuse invalid internal
        // material before constructing/copying any representation bytes.
        if token.len() != TOKEN_BYTES
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return None;
        }
        let mut bytes = [0; BODY_BYTES];
        bytes[..PREFIX.len()].copy_from_slice(PREFIX);
        bytes[PREFIX.len()..PREFIX.len() + TOKEN_BYTES].copy_from_slice(token.as_bytes());
        bytes[PREFIX.len() + TOKEN_BYTES..].copy_from_slice(SUFFIX);
        Some(Self { bytes })
    }
    pub(in crate::auth) fn into_bytes(self) -> hyper::body::Bytes {
        hyper::body::Bytes::from_owner(self)
    }
}
impl AsRef<[u8]> for FixedTokenBody {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
impl Drop for FixedTokenBody {
    fn drop(&mut self) {
        #[cfg(test)]
        observation::dropped(self.bytes.as_ptr() as usize);
    }
}

// Closed test instrumentation, with the identical production owner layout.
// Registration is explicit, scoped and identified independently of pointers.
// Drop only increments a weak atomic witness; no secret bytes or callback.
#[cfg(test)]
pub(in crate::auth) mod observation {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    };
    struct Entry {
        pointer: usize,
        id: usize,
        drops: Weak<AtomicUsize>,
    }
    static ENTRIES: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    pub(in crate::auth) struct Probe {
        pointer: usize,
        id: usize,
        drops: Arc<AtomicUsize>,
    }
    pub(in crate::auth) fn register(body: &hyper::body::Bytes) -> Probe {
        // Test instrumentation keeps the MSRV 1.88 atomic API. Remove this
        // statement allowance when the supported baseline has its replacement.
        #[allow(deprecated)]
        let id = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("fixed-body probe identity exhausted");
        let pointer = body.as_ptr() as usize;
        let drops = Arc::new(AtomicUsize::new(0));
        let mut entries = ENTRIES.lock().expect("fixed-body probe registry poisoned");
        entries.retain(|entry| entry.drops.strong_count() != 0);
        entries.push(Entry {
            pointer,
            id,
            drops: Arc::downgrade(&drops),
        });
        Probe { pointer, id, drops }
    }
    impl Probe {
        pub(in crate::auth) fn drops(&self) -> usize {
            // A poisoned observer can never masquerade as a successful Drop.
            let _guard = ENTRIES.lock().expect("fixed-body probe registry poisoned");
            self.drops.load(Ordering::SeqCst)
        }
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            if let Ok(mut entries) = ENTRIES.lock() {
                entries.retain(|entry| !(entry.pointer == self.pointer && entry.id == self.id));
            }
        }
    }
    pub(super) fn dropped(pointer: usize) {
        // Avoid a second panic during unwinding. Probe::drops detects poison.
        let Ok(mut entries) = ENTRIES.lock() else {
            return;
        };
        for entry in entries.iter().filter(|entry| entry.pointer == pointer) {
            if let Some(drops) = entry.drops.upgrade() {
                drops.fetch_add(1, Ordering::SeqCst);
            }
        }
        // Remove before allocator reuse; a later owner cannot hit old entries.
        entries.retain(|entry| entry.pointer != pointer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_protocol_owner_validates_before_copy_without_hidden_capacity() {
        assert_eq!(std::mem::size_of::<FixedTokenBody>(), BODY_BYTES);
        let token = "A".repeat(43);
        let (body, meter) = crate::metrics::measure(|| FixedTokenBody::new(&token).unwrap());
        assert_eq!(meter.allocations, 0);
        assert!(body.as_ref().len() == BODY_BYTES);
        assert!(body.as_ref().starts_with(PREFIX) && body.as_ref().ends_with(SUFFIX));
        assert!(body.as_ref().get(15..58) == Some(token.as_bytes()));
        for bad in [
            "A".repeat(42),
            "A".repeat(44),
            format!("{}=", "A".repeat(42)),
            format!("{}\"", "A".repeat(42)),
        ] {
            let (result, meter) = crate::metrics::measure(|| FixedTokenBody::new(&bad));
            assert!(result.is_none());
            assert_eq!(meter.allocations, 0);
        }
    }
    #[test]
    fn scoped_parallel_probes_do_not_confuse_abandoned_registration_or_other_owner() {
        let token = "A".repeat(43);
        let one = FixedTokenBody::new(&token).unwrap().into_bytes();
        let two = FixedTokenBody::new(&token).unwrap().into_bytes();
        let abandoned = observation::register(&one);
        drop(abandoned);
        let first = observation::register(&one);
        let second = observation::register(&two);
        let last = one.clone();
        drop(one);
        assert_eq!(first.drops(), 0);
        std::thread::spawn(move || drop(last)).join().unwrap();
        assert_eq!(first.drops(), 1);
        assert_eq!(second.drops(), 0);
        drop(two);
        assert_eq!(second.drops(), 1);
        // New registration on a different live owner cannot inherit either hit.
        let three = FixedTokenBody::new(&token).unwrap().into_bytes();
        let third = observation::register(&three);
        assert_eq!(third.drops(), 0);
        drop(first);
        drop(second);
        drop(three);
        assert_eq!(third.drops(), 1);
    }
}
