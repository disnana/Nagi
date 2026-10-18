//! One native header map and its prior response claim; Request views share it.
//! Inbound/application retention is the existing input boundary, not a new
//! process-memory guarantee. No response factory or authentication proof lives here.
use crate::security_sf07_private::quota::Reservation;
use axum::http::{header::GetAll, Extensions, HeaderMap, HeaderValue};
use std::sync::Arc;

pub(super) struct ReceivedHeaders {
    // Dispose actual map names/values/container before its original claim.
    map: HeaderMap,
    // Native metadata shares the same actual Request/header owner. Never clone
    // Extensions: it must die before the request receipt can be recycled.
    _extensions: Extensions,
    _prior: Option<Arc<Reservation>>,
    context: ContextOwner,
    // Last: the actual map, prior response and Context die before this receipt.
    request_owner: Option<tokio::sync::OwnedSemaphorePermit>,
}
enum ContextOwner {
    Attached(super::ResponseContext),
    #[cfg(test)]
    Unattached,
}
impl ReceivedHeaders {
    #[cfg(test)]
    pub(super) fn configured(
        map: HeaderMap,
        prior: Option<Arc<Reservation>>,
        context: super::ResponseContext,
    ) -> Arc<Self> {
        Self::configured_native(map, Extensions::new(), prior, context)
    }
    pub(super) fn configured_native(
        map: HeaderMap,
        extensions: Extensions,
        prior: Option<Arc<Reservation>>,
        context: super::ResponseContext,
    ) -> Arc<Self> {
        Arc::new(Self {
            map,
            _extensions: extensions,
            _prior: prior,
            context: ContextOwner::Attached(context),
            request_owner: None,
        })
    }
    #[cfg(test)]
    pub(super) fn new(map: HeaderMap, prior: Option<Arc<Reservation>>) -> Arc<Self> {
        Self::new_native(map, Extensions::new(), prior)
    }
    #[cfg(test)]
    pub(super) fn new_native(
        map: HeaderMap,
        extensions: Extensions,
        prior: Option<Arc<Reservation>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            map,
            _extensions: extensions,
            _prior: prior,
            context: ContextOwner::Unattached,
            request_owner: None,
        })
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
    pub(super) fn profile(
        &mut self,
        profile: super::response_public::ResponseProfile,
    ) -> Result<(), crate::Error> {
        let owner = Arc::get_mut(&mut self.owner)
            .ok_or_else(|| crate::Error::internal("request owner already shared"))?;
        match &mut owner.context {
            ContextOwner::Attached(context) => context.profile = profile,
            #[cfg(test)]
            ContextOwner::Unattached => {
                return Err(crate::Error::internal("unattached request fixture"))
            }
        }
        Ok(())
    }
    pub(super) fn admit(
        &mut self,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<(), crate::Error> {
        let owner = Arc::get_mut(&mut self.owner)
            .ok_or_else(|| crate::Error::internal("request owner already shared"))?;
        if owner.request_owner.is_some() {
            return Err(crate::Error::internal("request owner already admitted"));
        }
        owner.request_owner = Some(permit);
        Ok(())
    }
    pub(super) fn response_context(&self) -> &super::ResponseContext {
        match &self.owner.context {
            ContextOwner::Attached(context) => context,
            #[cfg(test)]
            ContextOwner::Unattached => panic!("unattached header fixture has no response Context"),
        }
    }
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
#[cfg(test)]
impl From<HeaderMap> for RequestHeaders {
    fn from(map: HeaderMap) -> Self {
        Self::from_received(ReceivedHeaders::new(map, None))
    }
}

#[cfg(test)]
#[path = "request_headers_tests.rs"]
mod tests;

#[cfg(test)]
mod native_owner_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct NativeProbe {
        drops: Arc<AtomicUsize>,
        owners: Arc<tokio::sync::Semaphore>,
    }
    impl Drop for NativeProbe {
        fn drop(&mut self) {
            assert_eq!(self.owners.available_permits(), 0);
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn native_extensions_follow_last_header_view_and_drop_before_request_receipt() {
        let fixture = super::super::response_public::tests::headers_fixture(HeaderMap::new(), 256);
        let context = fixture.response_context().share();
        drop(fixture);
        let owners = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = Arc::clone(&owners).try_acquire_owned().unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let mut native = axum::http::Request::new(());
        native
            .headers_mut()
            .insert("x-native", HeaderValue::from_static("owned"));
        native.extensions_mut().insert(NativeProbe {
            drops: Arc::clone(&drops),
            owners: Arc::clone(&owners),
        });
        let mut incoming = super::super::IncomingRequest::new(native, None, context);
        incoming.parts.headers.admit(permit).unwrap();
        let view = incoming.parts.headers.share();
        assert_eq!(view.get_all("x-native").iter().next().unwrap(), "owned");
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(incoming);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(owners.available_permits(), 0);
        drop(view);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(owners.available_permits(), 1);
    }
}
