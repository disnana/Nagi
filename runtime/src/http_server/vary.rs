//! Finite framework-owned response dependencies. Only the outer boundary writes Vary.
use super::*;

#[derive(Clone, Copy, Default)]
pub(super) struct Dependencies(u8);
impl Dependencies {
    pub(super) const ORIGIN: Self = Self(1 << 5);
    pub(super) const COOKIE: Self = Self(1 << 4);
    pub(super) const AUTH: Self = Self((1 << 2) | (1 << 4));
    pub(super) const LOGIN: Self = Self(Self::AUTH.0 | Self::ORIGIN.0);
    pub(super) const GUARDED: Self = Self(Self::LOGIN.0 | (1 << 6));
    pub(super) const BROWSER: Self = Self(Self::GUARDED.0 | (1 << 3));
    pub(super) const PREFLIGHT: Self = Self((1 << 0) | (1 << 1) | Self::ORIGIN.0);
    pub(super) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub(super) fn source(source: PolicySource) -> Self {
        match source {
            PolicySource::Public => Self::default(),
            PolicySource::Bearer | PolicySource::Session => Self::AUTH,
            PolicySource::Prelogin => Self::LOGIN,
            PolicySource::BrowserLogin => Self::BROWSER,
        }
    }
    pub(super) fn route<S>(route: &Route<S>) -> Self {
        match &route.kind {
            RouteKind::Guarded { .. } => Self::GUARDED,
            RouteKind::CsrfTokens(_) | RouteKind::Bootstrap(_) => Self::LOGIN,
            RouteKind::Browser { .. } => Self::BROWSER,
            RouteKind::Ordinary(_) => Self::source(route.source),
        }
    }
}
pub(super) fn add(response: &mut axum::http::Response<BufferedBody>, dependencies: Dependencies) {
    let prior = response
        .extensions()
        .get::<Dependencies>()
        .copied()
        .unwrap_or_default();
    response.extensions_mut().insert(prior.union(dependencies));
}
pub(super) fn finish(
    mut response: axum::http::Response<BufferedBody>,
) -> axum::http::Response<BufferedBody> {
    let dependencies = response
        .extensions_mut()
        .remove::<Dependencies>()
        .unwrap_or_default();
    // Public header setters cannot supply Vary. No untyped header text is reparsed here.
    response.headers_mut().remove(names::VARY);
    let names = [
        "Access-Control-Request-Headers",
        "Access-Control-Request-Method",
        "Authorization",
        "Content-Type",
        "Cookie",
        "Origin",
        "X-CSRF-Token",
    ];
    let mut value = String::new();
    for (index, name) in names.iter().enumerate() {
        if dependencies.0 & (1 << index) != 0 {
            if !value.is_empty() {
                value.push_str(", ");
            }
            value.push_str(name);
        }
    }
    if !value.is_empty() {
        response.headers_mut().insert(
            names::VARY,
            HeaderValue::from_str(&value).expect("finite static dependency names"),
        );
    }
    response
}
