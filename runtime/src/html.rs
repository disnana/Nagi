//! Closed, buffered HTML values. All output is context-encoded and budgeted.
//! Navigation values authorize a browser href only, never a server fetch.
use crate::Error;
use std::{cell::Cell, sync::Arc};
use url::{Host, Position, SyntaxViolation, Url};
#[path = "html/encoding.rs"]
mod encoding;

const PHRASING: u8 = 1;
const FLOW: u8 = 2;
const LIST_ITEM: u8 = 4;
const DOCUMENT_PREFIX: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>";
const DOCUMENT_MIDDLE: &str = "</title></head><body>";
const DOCUMENT_SUFFIX: &str = "</body></html>";

#[derive(Clone, Copy, PartialEq, Eq)]
struct Limits {
    output: usize,
    input: usize,
    nodes: usize,
    depth: usize,
    urls: usize,
    url_bytes: usize,
}
impl Limits {
    const HARD: Self = Self {
        output: 65_536,
        input: 16_384,
        nodes: 1_024,
        depth: 16,
        urls: 16,
        url_bytes: 2_048,
    };
}
#[derive(PartialEq, Eq)]
struct Profile {
    base: Url,
    limits: Limits,
}

pub struct HtmlPolicy {
    profile: Arc<Profile>,
}
pub struct HtmlAttributes {
    profile: Arc<Profile>,
    title: Option<String>,
    href: Option<String>,
    summary: Summary,
}
pub struct NavigationUrl {
    profile: Arc<Profile>,
    value: String,
    input: usize,
}
pub struct HtmlFragment {
    profile: Arc<Profile>,
    encoded: String,
    summary: Summary,
}
pub struct HtmlDocument {
    _profile: Arc<Profile>,
    _encoded: String,
    _summary: Summary,
}
impl HtmlDocument {
    // Only the checked standard response factory may consume the owned bytes.
    // This bridge does not expose a public raw-HTML reconstruction path.
    pub(crate) fn into_encoded(self) -> String {
        self._encoded
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Div,
    Span,
    P,
    H1,
    H2,
    Strong,
    Em,
    Ul,
    Ol,
    Li,
    A,
    Br,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlTag(Tag);
impl HtmlTag {
    pub const DIV: Self = Self(Tag::Div);
    pub const SPAN: Self = Self(Tag::Span);
    pub const P: Self = Self(Tag::P);
    pub const H1: Self = Self(Tag::H1);
    pub const H2: Self = Self(Tag::H2);
    pub const STRONG: Self = Self(Tag::Strong);
    pub const EM: Self = Self(Tag::Em);
    pub const UL: Self = Self(Tag::Ul);
    pub const OL: Self = Self(Tag::Ol);
    pub const LI: Self = Self(Tag::Li);
    pub const A: Self = Self(Tag::A);
    pub const BR: Self = Self(Tag::Br);
}
impl Tag {
    fn name(self) -> &'static str {
        match self {
            Self::Div => "div",
            Self::Span => "span",
            Self::P => "p",
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::Strong => "strong",
            Self::Em => "em",
            Self::Ul => "ul",
            Self::Ol => "ol",
            Self::Li => "li",
            Self::A => "a",
            Self::Br => "br",
        }
    }
    fn class(self) -> u8 {
        match self {
            Self::Div | Self::P | Self::H1 | Self::H2 | Self::Ul | Self::Ol => FLOW,
            Self::Li => LIST_ITEM,
            _ => PHRASING,
        }
    }
    fn children(self) -> u8 {
        match self {
            Self::Div | Self::Li => FLOW | PHRASING,
            Self::Ul | Self::Ol => LIST_ITEM,
            Self::Br => 0,
            _ => PHRASING,
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Summary {
    input: usize,
    nodes: usize,
    depth: usize,
    urls: usize,
    kinds: u8,
    anchor: bool,
}
fn invalid() -> Error {
    Error::invalid("invalid HTML value or budget")
}
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or_else(invalid)
}
fn total(parts: &[usize]) -> Result<usize, Error> {
    parts.iter().try_fold(0, |n, part| add(n, *part))
}
fn check(profile: &Profile, summary: Summary, output: usize) -> Result<(), Error> {
    let l = profile.limits;
    if output > l.output
        || summary.input > l.input
        || summary.nodes > l.nodes
        || summary.depth > l.depth
        || summary.urls > l.urls
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn combine(a: Summary, b: Summary) -> Result<Summary, Error> {
    Ok(Summary {
        input: add(a.input, b.input)?,
        nodes: add(a.nodes, b.nodes)?,
        depth: a.depth.max(b.depth),
        urls: add(a.urls, b.urls)?,
        kinds: a.kinds | b.kinds,
        anchor: a.anchor || b.anchor,
    })
}
fn matching(a: &Profile, b: &Profile) -> Result<(), Error> {
    if a == b {
        Ok(())
    } else {
        Err(invalid())
    }
}
fn buffer(length: usize) -> Result<String, Error> {
    let mut result = String::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| Error::internal("HTML allocation failed"))?;
    Ok(result)
}
fn owned(value: &str) -> Result<String, Error> {
    let mut result = buffer(value.len())?;
    result.push_str(value);
    Ok(result)
}
fn strict_url_input(value: &str, max: usize) -> Result<(), Error> {
    if value.len() > max
        || value
            .chars()
            .any(|c| c <= '\u{20}' || c == '\u{7f}' || c == '\\')
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn exactly_one_slash(value: &str) -> bool {
    value.starts_with('/') && !value.starts_with("//")
}
fn same_destination(a: &Url, b: &Url) -> bool {
    a.origin() == b.origin()
        && a.path() == b.path()
        && a.query() == b.query()
        && a.fragment() == b.fragment()
}

/// Canonical HTTPS origin; no userinfo, query, fragment, root dot or nonroot path.
pub fn policy(base_origin: &str) -> Result<HtmlPolicy, Error> {
    strict_url_input(base_origin, Limits::HARD.url_bytes)?;
    let credentials = Cell::new(false);
    let callback = |violation| {
        if violation == SyntaxViolation::EmbeddedCredentials {
            credentials.set(true);
        }
    };
    let base = Url::options()
        .syntax_violation_callback(Some(&callback))
        .parse(base_origin)
        .map_err(|_| invalid())?;
    if credentials.get()
        || base.scheme() != "https"
        || base.path() != "/"
        || base.query().is_some()
        || base.fragment().is_some()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.as_str().len() > Limits::HARD.url_bytes
    {
        return Err(invalid());
    }
    match base.host() {
        Some(Host::Domain(domain)) => {
            if domain.len() > 253
                || domain.ends_with('.')
                || domain
                    .split('.')
                    .any(|part| part.is_empty() || part.len() > 63)
            {
                return Err(invalid());
            }
        }
        Some(Host::Ipv4(_)) | Some(Host::Ipv6(_)) => {}
        None => return Err(invalid()),
    }
    Ok(HtmlPolicy {
        profile: Arc::new(Profile {
            base,
            limits: Limits::HARD,
        }),
    })
}
/// Lower hard ceilings; only URLs may be zero. Existing limits cannot be raised.
pub fn limits(
    policy: HtmlPolicy,
    output_bytes: i64,
    input_bytes: i64,
    nodes: i64,
    depth: i64,
    urls: i64,
    url_bytes: i64,
) -> Result<HtmlPolicy, Error> {
    fn lower(value: i64, current: usize, zero: bool) -> Result<usize, Error> {
        let n = usize::try_from(value).map_err(|_| invalid())?;
        if n > current || (!zero && n == 0) {
            Err(invalid())
        } else {
            Ok(n)
        }
    }
    let old = policy.profile.limits;
    let new = Limits {
        output: lower(output_bytes, old.output, false)?,
        input: lower(input_bytes, old.input, false)?,
        nodes: lower(nodes, old.nodes, false)?,
        depth: lower(depth, old.depth, false)?,
        urls: lower(urls, old.urls, true)?,
        url_bytes: lower(url_bytes, old.url_bytes, false)?,
    };
    if policy.profile.base.as_str().len() > new.url_bytes {
        return Err(invalid());
    }
    Ok(HtmlPolicy {
        profile: Arc::new(Profile {
            base: policy.profile.base.clone(),
            limits: new,
        }),
    })
}
pub fn empty(policy: &HtmlPolicy) -> HtmlFragment {
    HtmlFragment {
        profile: Arc::clone(&policy.profile),
        encoded: String::new(),
        summary: Summary::default(),
    }
}
pub fn text(policy: &HtmlPolicy, value: &str) -> Result<HtmlFragment, Error> {
    let l = policy.profile.limits;
    let length = encoding::encoded_len(value, l.input, l.output)?;
    let summary = Summary {
        input: value.len(),
        nodes: 1,
        kinds: PHRASING,
        ..Summary::default()
    };
    check(&policy.profile, summary, length)?;
    Ok(HtmlFragment {
        profile: Arc::clone(&policy.profile),
        encoded: encoding::encode(value, l.input, l.output)?,
        summary,
    })
}
pub fn attributes(policy: &HtmlPolicy) -> HtmlAttributes {
    HtmlAttributes {
        profile: Arc::clone(&policy.profile),
        title: None,
        href: None,
        summary: Summary::default(),
    }
}
impl HtmlAttributes {
    fn output_length(&self) -> Result<usize, Error> {
        total(&[
            self.title.as_ref().map_or(0, |s| s.len() + 9),
            self.href.as_ref().map_or(0, |s| s.len() + 8),
        ])
    }
    fn append_to(&self, output: &mut String) {
        if let Some(value) = &self.title {
            output.push_str(" title=\"");
            output.push_str(value);
            output.push('"');
        }
        if let Some(value) = &self.href {
            output.push_str(" href=\"");
            output.push_str(value);
            output.push('"');
        }
    }
}
pub fn title(mut attributes: HtmlAttributes, value: &str) -> Result<HtmlAttributes, Error> {
    if attributes.title.is_some() {
        return Err(invalid());
    }
    let l = attributes.profile.limits;
    let length = encoding::encoded_len(value, l.input, l.output)?;
    let summary = combine(
        attributes.summary,
        Summary {
            input: value.len(),
            ..Summary::default()
        },
    )?;
    check(
        &attributes.profile,
        summary,
        total(&[attributes.output_length()?, length, 9])?,
    )?;
    attributes.title = Some(encoding::encode(value, l.input, l.output)?);
    attributes.summary = summary;
    Ok(attributes)
}
pub fn navigation(policy: &HtmlPolicy, value: &str) -> Result<NavigationUrl, Error> {
    let l = policy.profile.limits;
    strict_url_input(value, l.url_bytes.min(l.input))?;
    if !exactly_one_slash(value) || l.urls == 0 {
        return Err(invalid());
    }
    let expected = policy.profile.base.join(value).map_err(|_| invalid())?;
    if expected.origin() != policy.profile.base.origin() {
        return Err(invalid());
    }
    let final_href = &expected[Position::BeforePath..];
    if !exactly_one_slash(final_href) || final_href.len() > l.url_bytes {
        return Err(invalid());
    }
    let resolved = policy
        .profile
        .base
        .join(final_href)
        .map_err(|_| invalid())?;
    if !same_destination(&expected, &resolved) || resolved.origin() != policy.profile.base.origin()
    {
        return Err(invalid());
    }
    // Check the eventual quoted-attribute encoding before storing the URL.
    encoding::encoded_len(final_href, l.url_bytes, l.output)?;
    Ok(NavigationUrl {
        profile: Arc::clone(&policy.profile),
        value: owned(final_href)?,
        input: value.len(),
    })
}
pub fn href(mut attributes: HtmlAttributes, value: NavigationUrl) -> Result<HtmlAttributes, Error> {
    if attributes.href.is_some() {
        return Err(invalid());
    }
    matching(&attributes.profile, &value.profile)?;
    let l = attributes.profile.limits;
    let length = encoding::encoded_len(&value.value, l.url_bytes, l.output)?;
    let summary = combine(
        attributes.summary,
        Summary {
            input: value.input,
            urls: 1,
            ..Summary::default()
        },
    )?;
    check(
        &attributes.profile,
        summary,
        total(&[attributes.output_length()?, length, 8])?,
    )?;
    attributes.href = Some(encoding::encode(&value.value, l.url_bytes, l.output)?);
    attributes.summary = summary;
    Ok(attributes)
}
pub fn element(
    policy: &HtmlPolicy,
    tag: HtmlTag,
    attributes: HtmlAttributes,
    children: HtmlFragment,
) -> Result<HtmlFragment, Error> {
    matching(&policy.profile, &attributes.profile)?;
    matching(&policy.profile, &children.profile)?;
    let kind = tag.0;
    if (attributes.href.is_some() && kind != Tag::A)
        || children.summary.kinds & !kind.children() != 0
        || (kind == Tag::Br && children.summary.nodes != 0)
        || (kind == Tag::A && children.summary.anchor)
    {
        return Err(invalid());
    }
    let summary = combine(attributes.summary, children.summary)?;
    let summary = Summary {
        nodes: add(summary.nodes, 1)?,
        depth: add(children.summary.depth, 1)?,
        kinds: kind.class(),
        anchor: summary.anchor || kind == Tag::A,
        ..summary
    };
    let name = kind.name();
    let end_length = if kind == Tag::Br { 0 } else { name.len() + 3 };
    let length = total(&[
        name.len(),
        2,
        attributes.output_length()?,
        children.encoded.len(),
        end_length,
    ])?;
    check(&policy.profile, summary, length)?;
    let mut encoded = buffer(length)?;
    encoded.push('<');
    encoded.push_str(name);
    attributes.append_to(&mut encoded);
    encoded.push('>');
    encoded.push_str(&children.encoded);
    if kind != Tag::Br {
        encoded.push_str("</");
        encoded.push_str(name);
        encoded.push('>');
    }
    Ok(HtmlFragment {
        profile: Arc::clone(&policy.profile),
        encoded,
        summary,
    })
}
pub fn join(
    policy: &HtmlPolicy,
    left: HtmlFragment,
    right: HtmlFragment,
) -> Result<HtmlFragment, Error> {
    matching(&policy.profile, &left.profile)?;
    matching(&policy.profile, &right.profile)?;
    let summary = combine(left.summary, right.summary)?;
    let length = add(left.encoded.len(), right.encoded.len())?;
    check(&policy.profile, summary, length)?;
    let mut encoded = buffer(length)?;
    encoded.push_str(&left.encoded);
    encoded.push_str(&right.encoded);
    Ok(HtmlFragment {
        profile: Arc::clone(&policy.profile),
        encoded,
        summary,
    })
}
pub fn copy_fragment(policy: &HtmlPolicy, fragment: &HtmlFragment) -> Result<HtmlFragment, Error> {
    matching(&policy.profile, &fragment.profile)?;
    check(&policy.profile, fragment.summary, fragment.encoded.len())?;
    Ok(HtmlFragment {
        profile: Arc::clone(&policy.profile),
        encoded: owned(&fragment.encoded)?,
        summary: fragment.summary,
    })
}
pub fn document(
    policy: &HtmlPolicy,
    title: &str,
    body: HtmlFragment,
) -> Result<HtmlDocument, Error> {
    matching(&policy.profile, &body.profile)?;
    if body.summary.kinds & !(FLOW | PHRASING) != 0 {
        return Err(invalid());
    }
    let l = policy.profile.limits;
    let title_length = encoding::encoded_len(title, l.input, l.output)?;
    let summary = Summary {
        input: add(body.summary.input, title.len())?,
        nodes: add(body.summary.nodes, 7)?,
        depth: add(body.summary.depth, 2)?.max(3),
        ..body.summary
    };
    let length = total(&[
        DOCUMENT_PREFIX.len(),
        title_length,
        DOCUMENT_MIDDLE.len(),
        body.encoded.len(),
        DOCUMENT_SUFFIX.len(),
    ])?;
    check(&policy.profile, summary, length)?;
    let encoded_title = encoding::encode(title, l.input, l.output)?;
    let mut encoded = buffer(length)?;
    encoded.push_str(DOCUMENT_PREFIX);
    encoded.push_str(&encoded_title);
    encoded.push_str(DOCUMENT_MIDDLE);
    encoded.push_str(&body.encoded);
    encoded.push_str(DOCUMENT_SUFFIX);
    Ok(HtmlDocument {
        _profile: Arc::clone(&policy.profile),
        _encoded: encoded,
        _summary: summary,
    })
}

#[cfg(test)]
#[path = "html/tests.rs"]
mod tests;
