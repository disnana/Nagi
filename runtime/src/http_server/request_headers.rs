//! One native header map and its prior response claim; Request views share it.
//! Inbound/application retention is the existing input boundary, not a new
//! process-memory guarantee. No response factory or authentication proof lives here.
use crate::security_sf07_private::quota::Reservation;
use axum::http::{header::GetAll, HeaderMap, HeaderValue};
use std::sync::Arc;

pub(super) struct ReceivedHeaders {
    // Dispose actual map names/values/container before its original claim.
    map: HeaderMap,
    _prior: Option<Arc<Reservation>>,
}
impl ReceivedHeaders {
    pub(super) fn new(map: HeaderMap, prior: Option<Arc<Reservation>>) -> Arc<Self> {
        Arc::new(Self { map, _prior: prior })
    }
    #[cfg(test)]
    pub(super) fn value(&self, name: &str) -> Option<&[u8]> {
        self.map.get(name).map(HeaderValue::as_bytes)
    }
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.map.capacity()
    }
}

#[derive(Clone, Copy)]
enum Visibility {
    All,
    Sanitized,
}
impl Visibility {
    fn permits(self, name: &str) -> bool {
        matches!(self, Self::All)
            || !(name.eq_ignore_ascii_case("cookie") || name.eq_ignore_ascii_case("x-csrf-token"))
    }
}

// Per-Request visibility never edits or clones the shared native map. In
// particular, a trusted preparation head may be All while its handler is hidden.
pub(super) struct RequestHeaders {
    owner: Arc<ReceivedHeaders>,
    visibility: Visibility,
}
impl RequestHeaders {
    pub(super) fn from_received(owner: Arc<ReceivedHeaders>) -> Self {
        Self {
            owner,
            visibility: Visibility::All,
        }
    }
    pub(super) fn share(&self) -> Self {
        Self {
            owner: Arc::clone(&self.owner),
            visibility: self.visibility,
        }
    }
    pub(super) fn sanitize(&mut self) {
        self.visibility = Visibility::Sanitized;
    }
    pub(super) fn len(&self) -> usize {
        self.owner.map.len()
            - ["cookie", "x-csrf-token"]
                .into_iter()
                .filter(|name| !self.visibility.permits(name))
                .map(|name| self.owner.map.get_all(name).iter().count())
                .sum::<usize>()
    }
    pub(super) fn get_all(&self, name: impl AsRef<str>) -> GetAll<'_, HeaderValue> {
        let name = name.as_ref();
        self.owner.map.get_all(if self.visibility.permits(name) {
            name
        } else {
            ""
        })
    }
    pub(super) fn contains_key(&self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        self.visibility.permits(name) && self.owner.map.contains_key(name)
    }
    // Only the closed Session lookup uses this before callback sanitization.
    // No public Request accessor or Debug can return/borrow the raw container.
    pub(super) fn authentication_headers(&self) -> &HeaderMap {
        &self.owner.map
    }
    #[cfg(test)]
    fn fixture_map(&mut self) -> &mut HeaderMap {
        let owner = Arc::get_mut(&mut self.owner).expect("fixture header owner must be unique");
        assert!(
            owner._prior.is_none(),
            "fixture cannot mutate a charged native map"
        );
        &mut owner.map
    }
    #[cfg(test)]
    pub(super) fn insert<K: axum::http::header::IntoHeaderName>(
        &mut self,
        name: K,
        value: HeaderValue,
    ) -> Option<HeaderValue> {
        self.fixture_map().insert(name, value)
    }
    #[cfg(test)]
    pub(super) fn append<K: axum::http::header::IntoHeaderName>(
        &mut self,
        name: K,
        value: HeaderValue,
    ) -> bool {
        self.fixture_map().append(name, value)
    }
    #[cfg(test)]
    pub(super) fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner)
    }
}
impl From<HeaderMap> for RequestHeaders {
    fn from(map: HeaderMap) -> Self {
        Self::from_received(ReceivedHeaders::new(map, None))
    }
}

#[cfg(test)]
#[path = "request_headers_tests.rs"]
mod tests;
