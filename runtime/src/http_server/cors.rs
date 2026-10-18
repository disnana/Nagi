//! Exact route metadata. CORS grants browser reads; it creates no authority or CSRF permission.
use super::origin::Origin;
use super::*;
use hyper::body::Body as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorsCredentials {
    Omit,
    Allow,
}
impl CorsCredentials {
    pub const OMIT: Self = Self::Omit;
    pub const ALLOW: Self = Self::Allow;
}
pub struct CorsPolicy {
    origins: Vec<(Origin, HeaderValue)>,
    headers: Vec<HeaderName>,
    credentials: CorsCredentials,
    max_age: HeaderValue,
    origin_bytes: usize,
    header_count: usize,
    header_bytes: usize,
}
impl CorsPolicy {
    pub(super) fn retained_bytes(&self) -> Result<usize, Error> {
        let invalid = || Error::invalid("CORS metadata overflow");
        let mut bytes = self
            .origins
            .capacity()
            .checked_mul(std::mem::size_of::<(Origin, HeaderValue)>())
            .ok_or_else(invalid)?;
        bytes = bytes
            .checked_add(
                self.headers
                    .capacity()
                    .checked_mul(std::mem::size_of::<HeaderName>())
                    .ok_or_else(invalid)?,
            )
            .ok_or_else(invalid)?;
        bytes = bytes
            .checked_add(self.max_age.as_bytes().len())
            .ok_or_else(invalid)?;
        for (origin, wire) in &self.origins {
            bytes = bytes
                .checked_add(origin.retained_bytes())
                .and_then(|n| n.checked_add(wire.as_bytes().len()))
                .ok_or_else(invalid)?;
        }
        for name in &self.headers {
            bytes = bytes.checked_add(name.as_str().len()).ok_or_else(invalid)?;
        }
        Ok(bytes)
    }
}
impl fmt::Debug for CorsPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CorsPolicy")
            .field("origins", &self.origins.len())
            .field("headers", &self.headers.len())
            .field("credentials", &self.credentials)
            .finish_non_exhaustive()
    }
}
fn bounded(values: &[String], count: usize, bytes: usize, separators: bool) -> Result<(), Error> {
    if values.len() > count {
        return Err(Error::invalid("CORS count limit exceeded"));
    }
    let mut total = if separators {
        values
            .len()
            .saturating_sub(1)
            .checked_mul(2)
            .ok_or_else(|| Error::invalid("CORS byte overflow"))?
    } else {
        0
    };
    for value in values {
        total = total
            .checked_add(value.len())
            .filter(|n| *n <= bytes)
            .ok_or_else(|| Error::invalid("CORS byte limit exceeded"))?;
    }
    if total > bytes {
        return Err(Error::invalid("CORS byte limit exceeded"));
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub fn cors_policy(
    origins: Vec<String>,
    request_headers: Vec<String>,
    credentials: CorsCredentials,
    max_age_seconds: i64,
    max_origins: i64,
    max_origin_bytes: i64,
    max_headers: i64,
    max_header_bytes: i64,
) -> Result<CorsPolicy, Error> {
    let origin_count = positive_size(max_origins, isize::MAX as usize, "CORS origins")?;
    let origin_bytes = positive_size(max_origin_bytes, isize::MAX as usize, "CORS origin bytes")?;
    let header_count = positive_size(max_headers, isize::MAX as usize, "CORS headers")?;
    let header_bytes = positive_size(max_header_bytes, isize::MAX as usize, "CORS header bytes")?;
    let age = u32::try_from(max_age_seconds).map_err(|_| Error::invalid("invalid CORS max age"))?;
    bounded(&origins, origin_count, origin_bytes, false)?;
    bounded(&request_headers, header_count, header_bytes, true)?;
    let mut allowed = Vec::new();
    let mut canonical_bytes = 0usize;
    for raw in origins {
        let origin = Origin::checked(&raw)?;
        if allowed.iter().any(|(prior, _)| *prior == origin) {
            return Err(Error::invalid("duplicate canonical CORS origin"));
        }
        let serialized = origin.serialized();
        canonical_bytes = canonical_bytes
            .checked_add(serialized.len())
            .filter(|n| *n <= origin_bytes)
            .ok_or_else(|| Error::invalid("canonical CORS origins exceed byte limit"))?;
        let header = HeaderValue::from_str(&serialized)
            .map_err(|_| Error::invalid("invalid CORS origin serialization"))?;
        allowed.push((origin, header));
    }
    let mut headers = Vec::new();
    for name in request_headers {
        if name == "*" {
            return Err(Error::invalid("wildcard CORS headers are unsupported"));
        }
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| Error::invalid("invalid CORS header name"))?;
        if headers.contains(&name) {
            return Err(Error::invalid("duplicate canonical CORS header"));
        }
        headers.push(name);
    }
    Ok(CorsPolicy {
        origins: allowed,
        headers,
        credentials,
        max_age: HeaderValue::from(age),
        origin_bytes,
        header_count,
        header_bytes,
    })
}
fn supported(method: &Method) -> bool {
    !matches!(method.as_str(), "*" | "CONNECT" | "TRACE" | "TRACK")
}
pub fn cors_route<S, E>(
    mut app: App<S, E>,
    method: Method,
    registered_path: &str,
    cors: CorsPolicy,
) -> Result<App<S, E>, Error> {
    if !supported(&method)
        || method.as_str().len() > cors.header_bytes
        || HeaderValue::from_str(method.as_str()).is_err()
    {
        return Err(Error::invalid("unsupported CORS method"));
    }
    let route = app
        .routes
        .get_mut(registered_path)
        .and_then(|routes| routes.iter_mut().find(|route| route.method == method))
        .ok_or_else(|| {
            Error::invalid("CORS attachment requires an exact registered method/path record")
        })?;
    if route.cors.is_some() {
        return Err(Error::invalid("CORS route metadata is already sealed"));
    }
    route.cors = Some(cors);
    Ok(app)
}
pub(super) fn candidate(method: &Method, headers: &HeaderMap) -> bool {
    *method == Method::OPTIONS
        && (headers.contains_key(names::ACCESS_CONTROL_REQUEST_METHOD)
            || headers.contains_key(names::ACCESS_CONTROL_REQUEST_HEADERS))
}
fn single(headers: &HeaderMap, name: HeaderName) -> Result<Option<&HeaderValue>, Status> {
    let mut values = headers.get_all(name).iter();
    let value = values.next();
    if values.next().is_some() {
        return Err(Status::BAD_REQUEST);
    }
    Ok(value)
}
pub(super) fn requested_method(headers: &HeaderMap) -> Result<Method, Status> {
    let value =
        single(headers, names::ACCESS_CONTROL_REQUEST_METHOD)?.ok_or(Status::BAD_REQUEST)?;
    let method = Method::from_bytes(value.as_bytes()).map_err(|_| Status::BAD_REQUEST)?;
    if !supported(&method) {
        return Err(Status::BAD_REQUEST);
    }
    Ok(method)
}
fn checked_origin(headers: &HeaderMap, limit: usize) -> Result<Origin, Status> {
    let value = single(headers, names::ORIGIN)?.ok_or(Status::BAD_REQUEST)?;
    if value.as_bytes().len() > limit {
        return Err(Status::BAD_REQUEST);
    }
    let raw = value.to_str().map_err(|_| Status::BAD_REQUEST)?;
    if raw == "null" {
        return Err(Status::FORBIDDEN);
    }
    Origin::checked(raw).map_err(|_| Status::BAD_REQUEST)
}
fn allowed(policy: &CorsPolicy, headers: &HeaderMap) -> Option<HeaderValue> {
    let origin = checked_origin(headers, policy.origin_bytes).ok()?;
    policy
        .origins
        .iter()
        .find(|(prior, _)| *prior == origin)
        .map(|(_, value)| value.clone())
}
pub(super) struct Context<'a> {
    route: Option<&'a CorsPolicy>,
    grant: Option<HeaderValue>,
    dependencies: vary::Dependencies,
}
impl<'a> Context<'a> {
    pub(super) fn new<S>(route: &'a Route<S>, headers: &HeaderMap, preflight: bool) -> Self {
        let dependencies = if preflight {
            vary::Dependencies::PREFLIGHT
        } else {
            vary::Dependencies::route(route).union(if route.cors.is_some() {
                vary::Dependencies::ORIGIN
            } else {
                vary::Dependencies::default()
            })
        };
        Self {
            route: route.cors.as_ref(),
            grant: route
                .cors
                .as_ref()
                .and_then(|policy| allowed(policy, headers)),
            dependencies,
        }
    }
    pub(super) fn profile(&self) -> response_public::ResponseProfile {
        response_public::ResponseProfile {
            origin: self.grant.clone(),
            credentials: self
                .route
                .is_some_and(|p| p.credentials == CorsCredentials::Allow),
            dependencies: self.dependencies,
        }
    }
    pub(super) fn finish(self, mut response: HttpResponse) -> HttpResponse {
        // Current request is authoritative, including absence and credentials
        // denial. A retained same-producer Response carries no CORS grant.
        response
            .headers_mut()
            .remove(names::ACCESS_CONTROL_ALLOW_ORIGIN);
        response
            .headers_mut()
            .remove(names::ACCESS_CONTROL_ALLOW_CREDENTIALS);
        if let Some(origin) = self.grant {
            response
                .headers_mut()
                .insert(names::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            if self
                .route
                .is_some_and(|policy| policy.credentials == CorsCredentials::Allow)
            {
                response.headers_mut().insert(
                    names::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_static("true"),
                );
            }
        }
        vary::add(&mut response, self.dependencies);
        vary::finish(response)
    }
}
pub(super) fn route_less(mut response: HttpResponse, preflight: bool) -> HttpResponse {
    if preflight {
        vary::add(&mut response, vary::Dependencies::PREFLIGHT);
    }
    vary::finish(response)
}
fn requested_headers(
    headers: &HeaderMap,
    count: usize,
    bytes: usize,
) -> Result<Vec<HeaderName>, Status> {
    let Some(value) = single(headers, names::ACCESS_CONTROL_REQUEST_HEADERS)? else {
        return Ok(Vec::new());
    };
    if value.as_bytes().len() > bytes {
        return Err(Status::BAD_REQUEST);
    }
    let raw = value.to_str().map_err(|_| Status::BAD_REQUEST)?;
    let mut names = Vec::new();
    for segment in raw.split(',') {
        let token = segment.trim_matches([' ', '\t']);
        if token.is_empty() || token == "*" || names.len() >= count {
            return Err(Status::BAD_REQUEST);
        }
        let name = HeaderName::from_bytes(token.as_bytes()).map_err(|_| Status::BAD_REQUEST)?;
        if names.contains(&name) {
            return Err(Status::BAD_REQUEST);
        }
        names.push(name);
    }
    let serialized_bytes = names.iter().try_fold(
        names
            .len()
            .saturating_sub(1)
            .checked_mul(2)
            .ok_or(Status::BAD_REQUEST)?,
        |n, name| {
            n.checked_add(name.as_str().len())
                .filter(|n| *n <= bytes)
                .ok_or(Status::BAD_REQUEST)
        },
    )?;
    if serialized_bytes > bytes {
        return Err(Status::BAD_REQUEST);
    }
    names.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
    Ok(names)
}
pub(super) fn preflight<S>(
    route: &Route<S>,
    requested: &Method,
    headers: &HeaderMap,
    body: &hyper::body::Incoming,
    options: &Options,
    context: &ResponseContext,
) -> HttpResponse {
    let run = || -> Result<HttpResponse, Status> {
        if !body.is_end_stream()
            || headers.contains_key(names::TRANSFER_ENCODING)
            || headers
                .get_all(names::CONTENT_LENGTH)
                .iter()
                .any(|v| v.as_bytes() != b"0")
        {
            return Err(Status::BAD_REQUEST);
        }
        let policy = route.cors.as_ref();
        checked_origin(
            headers,
            policy.map_or(options.header_bytes, |p| p.origin_bytes),
        )?;
        let requested_headers = requested_headers(
            headers,
            policy.map_or(options.header_count, |p| p.header_count),
            policy.map_or(options.header_bytes, |p| p.header_bytes),
        )?;
        let policy = policy.ok_or(Status::FORBIDDEN)?;
        if allowed(policy, headers).is_none()
            || requested_headers
                .iter()
                .any(|name| !policy.headers.contains(name))
        {
            return Err(Status::FORBIDDEN);
        }
        let mut response = empty(context, Status::NO_CONTENT)
            .map_err(|_| Status::SERVICE_UNAVAILABLE)?
            .into_http(false);
        response
            .headers_mut()
            .insert(names::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response.headers_mut().insert(
            names::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_str(requested.as_str()).map_err(|_| Status::BAD_REQUEST)?,
        );
        if !requested_headers.is_empty() {
            let value = requested_headers
                .iter()
                .map(HeaderName::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            response.headers_mut().insert(
                names::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_str(&value).map_err(|_| Status::BAD_REQUEST)?,
            );
        }
        response
            .headers_mut()
            .insert(names::ACCESS_CONTROL_MAX_AGE, policy.max_age.clone());
        Ok(response)
    };
    run().unwrap_or_else(|status| transport(context, status, false, true))
}

#[cfg(test)]
mod tests;
