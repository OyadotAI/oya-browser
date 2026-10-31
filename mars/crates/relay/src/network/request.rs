/** Literal HTTP requests carry no policy overrides; only script requests carry page-set headers,
and only ones the CORS rules allow. */
use super::constants::SCRIPT_REQUEST_HEADERS;
use crate::constants::FORM_BYTES;
use crate::protocol::{EngineError, EngineResult};
pub use mars_core::http::Method;
use serde::{Deserialize, Serialize};
/// These fields are internal service data, never an agent header API.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebRequest {
    /// Host-owned preferences shared with navigator.languages; never supplied by a page header API.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// Method is selected by the owned form algorithm, or by a page script for fetch and XHR.
    pub method: Method,
    /// The coordinator retains the actual URL separately from its public projection.
    pub url: String,
    /// Only the owned cookie jar assembles this field.
    pub cookie: String,
    /// Already URL-encoded successful form controls.
    pub body: String,
    /// Selects the smaller binary resource budget for explicitly requested images.
    #[serde(default)]
    pub image: bool,
    /// Fetch body readers need exact decoded bytes, independently of declared text charset.
    #[serde(default)]
    pub raw_response: bool,
    /// URL of the top-level document this fetch serves (the navigation's own URL for a
    /// navigation). Its origin partitions the HTTP cache; without it the cache is bypassed,
    /// so no response is ever shared across sites.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_level: Option<String>,
    /// URL of the document that caused this request: the page a link, form or subresource
    /// came from. None means the person asked (typed URL, bookmark, reload). It sets
    /// Sec-Fetch-Site, Origin and Referer the way Chrome would; without it they say "none".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initiator: Option<String>,
    /// What the response is for. It shapes Accept, Sec-Fetch-Dest and the HTTP/2 priority, which
    /// servers and bot detectors compare against a real browser's.
    #[serde(default)]
    pub destination: Destination,
    /// Headers a page's script set on a fetch or XHR (lowercase names), already stripped of
    /// forbidden names by the CORS rules; validation refuses any that slip through. Script
    /// requests only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, String)>,
    /// The Origin a script request sends, serialized the Fetch way (`null` once a redirect
    /// tainted it). None sends none. Script requests only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// A script request's mode, for Sec-Fetch-Mode. Script requests only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<super::cors::Mode>,
    /// Set for a CORS preflight: the request goes out as OPTIONS asking whether the real
    /// method and headers may follow, with no cookies, headers or body of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preflight: Option<Preflight>,
    /// For media (`Video`/`Audio`) only: the inclusive byte range to ask for, end open when
    /// None, sent as `Range: bytes=start-end` so a player fetches only what it is about to play.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<(u64, Option<u64>)>,
}
/// What a CORS preflight asks the server about the request waiting behind it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Preflight {
    /// Access-Control-Request-Method: the real request's method.
    pub method: String,
    /// Access-Control-Request-Headers: its CORS-unsafe header names, sorted, comma-joined.
    pub headers: String,
}
/// Chrome's request destinations that Mars issues. `image: true` always means `Image`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Destination {
    /// A top-level navigation or form submission.
    #[default]
    Document,
    /// A stylesheet.
    Style,
    /// A classic script.
    Script,
    /// A classic dedicated worker's initial same-origin script.
    Worker,
    /// A synchronous classic worker import, decoded as UTF-8 independently of its MIME charset.
    WorkerImport,
    /// An image.
    Image,
    /// A web font file for an `@font-face` rule: binary bytes of any declared type, under the
    /// image byte bound.
    Font,
    /// Script-issued fetch or XHR.
    Empty,
    /// A `<video>` element's media resource: raw bytes, fetched in byte ranges.
    Video,
    /// An `<audio>` element's media resource: raw bytes, fetched in byte ranges.
    Audio,
}
impl Destination {
    /** Media element fetches, which carry raw bytes and may ask for a byte range. */
    pub fn media(self) -> bool {
        matches!(self, Self::Video | Self::Audio)
    }
}
impl WebRequest {
    /// Navigation cannot accidentally inherit a previous submission body.
    pub fn get(url: &str) -> Self {
        Self {
            url: url.into(),
            ..Self::default()
        }
    }
    /// Internal validation also bounds misuse by a faulty coordinator.
    pub fn validate(&self) -> EngineResult<()> {
        mars_core::languages::validate(&self.languages)?;
        if self.body.len() > FORM_BYTES
            || self.raw_response
                && (self.destination != Destination::Empty
                    || self.image
                    || self.preflight.is_some())
            || self.range.is_some()
                && (!self.destination.media() || self.method != Method::Get || self.image)
            || self
                .range
                .is_some_and(|(start, end)| end.is_some_and(|end| end < start))
            || self.cookie.len() > crate::constants::HEADER_BYTES
            || self.cookie.chars().any(char::is_control)
            || (self.method.is_safe() && !self.body.is_empty())
            || self
                .top_level
                .as_deref()
                .is_some_and(|top| super::policy::web_url(top).is_err())
            || self
                .initiator
                .as_deref()
                .is_some_and(|initiator| super::policy::web_url(initiator).is_err())
            || !self.script_fields_valid()
        {
            return Err(EngineError::invalid("Invalid bounded web request."));
        }
        Ok(())
    }
    /** Script-only fields appear only on script requests, and every page-set byte is bounded:
    no forbidden or malformed header, no line break anywhere, and a preflight carries nothing
    of the page's own. */
    fn script_fields_valid(&self) -> bool {
        let script = self.destination == Destination::Empty;
        let plain = |text: &str| {
            !text
                .bytes()
                .any(|byte| byte.is_ascii_control() && byte != b'\t')
        };
        let token = |text: &str| {
            !text.is_empty()
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        };
        let bytes: usize = self
            .headers
            .iter()
            .map(|(name, value)| name.len() + value.len())
            .sum();
        let headers_ok = self.headers.len() <= SCRIPT_REQUEST_HEADERS
            && bytes <= crate::constants::HEADER_BYTES
            && self.headers.iter().all(|(name, value)| {
                token(name)
                    && *name == name.to_ascii_lowercase()
                    && !super::cors::forbidden_request_header(name)
                    && plain(value)
            });
        let origin_ok = self.origin.as_deref().is_none_or(|origin| {
            origin == "null"
                || super::policy::web_url(origin)
                    .is_ok_and(|url| url.origin().ascii_serialization() == origin)
        });
        let preflight_ok = self.preflight.as_ref().is_none_or(|preflight| {
            token(&preflight.method)
                && preflight
                    .headers
                    .split(',')
                    .all(|name| name.is_empty() || token(name))
                && self.method == Method::Get
                && self.cookie.is_empty()
                && self.headers.is_empty()
        });
        let used = !self.headers.is_empty()
            || self.origin.is_some()
            || self.mode.is_some()
            || self.preflight.is_some();
        // Images may carry only host-selected CORS fields, never page headers or preflights.
        let image = self.destination == Destination::Image
            && self.method == Method::Get
            && self.headers.is_empty()
            && self.preflight.is_none()
            && match self.mode {
                Some(super::cors::Mode::Cors) => self.origin.is_some(),
                None | Some(super::cors::Mode::NoCors) => self.origin.is_none(),
                _ => false,
            };
        // Module workers and their imports are CORS requests by spec: the host sets the mode
        // (and, cross-origin, the Origin); page headers and preflights never ride along.
        let worker = matches!(
            self.destination,
            Destination::Worker | Destination::WorkerImport
        ) && self.method == Method::Get
            && self.headers.is_empty()
            && self.preflight.is_none()
            && matches!(self.mode, None | Some(super::cors::Mode::Cors));
        (script || image || worker || !used) && headers_ok && origin_ok && preflight_ok
    }
}
/// Raw response metadata is private to the coordinator; never export cookies to agents.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebResponse {
    /// Final one-hop destination, already checked by networking policy.
    pub url: String,
    /// Server status is preserved for validation/error documents.
    pub status: u16,
    /// UTF-8 source, or base64 for explicitly requested image/raw fetch bytes and downloads.
    pub source: String,
    /// Tells the coordinator to decode source as bounded base64 image bytes.
    #[serde(default)]
    pub image_data: bool,
    /// An admitted fetch requested exact bytes; source is bounded base64, never decoded text.
    #[serde(default)]
    pub raw_data: bool,
    /// Canonical media type without parameters.
    pub mime: String,
    /// A redirect is processed by the coordinator with fresh authority.
    pub location: Option<String>,
    /// Repeated fields remain separate, never comma-joined.
    pub set_cookies: Vec<String>,
    /// Canonical WHATWG encoding the Content-Type header declared, so Eye can check a meta charset
    /// against it. None when undeclared (decoded as UTF-8), for image data or raw fetch bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// A top-level GET the server marked as an attachment, or whose type no page can display:
    /// `source` is then base64 of the raw body (still bounded by HTML_BYTES) and the coordinator
    /// saves it instead of publishing a page. It is never parsed or run.
    #[serde(default)]
    pub download: bool,
    /// The Content-Disposition value a download arrived with, for the suggested file name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    /// A script request's response headers (lowercase, repeats joined, at most
    /// SCRIPT_RESPONSE_HEADERS), for the coordinator's CORS check and filtering. Set-Cookie and
    /// Set-Cookie2 are never among them. A document carries only X-Frame-Options and
    /// Content-Security-Policy, for the framing check. Images carry only the two CORS admission
    /// fields; other requests carry none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, String)>,
}
