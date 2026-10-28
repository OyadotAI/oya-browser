/** Owned desktop HTTP framing over validated OS sockets and verified Rustls TLS. */
mod batch;
mod body;
mod cache;
mod encoding;
mod eyeballs;
mod fields;
mod hints;
mod h2;
mod hello;
mod lease;
mod legacy;
mod pool;
#[cfg(test)]
mod testing;
mod transport;
use super::mime;
use super::policy::{fixture_granted, web_url};
use super::{Destination, Method, WebRequest, WebResponse};
use crate::constants::{HTML_BYTES, HTTP_OK, IMAGE_BYTES, REDIRECTS, WEB_TIMEOUT};
use crate::protocol::{EngineError, EngineResult};
use ::url::Url;
pub use batch::fetch_batch;
pub use cache::{clear_cache, set_cache_directory};
pub use fields::CHROME_ENGINE;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{Instant, SystemTime};

/// Fetch results retain the final validated URL after a bounded redirect chain.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebPage {
    /// Fully received UTF-8 supported-markup candidate.
    pub source: String,
    /// Final destination for relative resolution and committed address state.
    pub url: String,
    /// Canonical encoding the source was decoded with, so the parser checks the page's meta against it.
    pub charset: Option<String>,
}

/// A one-use prepared exchange that expires with the navigation's remaining deadline. It is
/// either a fresh cache entry (no socket at all) or an exact connection, new or reused.
pub struct Prepared {
    request: WebRequest,
    url: Url,
    deadline: Instant,
    /** Whether the fixture grant admitted this destination; pool and cache keys include it. */
    fixture: bool,
    /** Cache identity, when the request carries a top-level partition. */
    key: Option<cache::Key>,
    source: Source,
}

/** Where a prepared response will come from. */
enum Source {
    /** Fresh in the cache; transmit sends nothing. */
    Cached(cache::Stored),
    /** A connection that waits for transmit, plus a stale entry to revalidate, if any. */
    Live {
        channel: Channel,
        reused: bool,
        stale: Option<cache::Stored>,
        /** Closes an HTTP/1.1 socket at the deadline; a shared HTTP/2 connection has none. */
        lease: Option<lease::Lease>,
    },
}

/** The protocol a connection speaks. */
enum Channel {
    /** One exchange at a time, reusable through the keep-alive pool. */
    Http1(transport::Connection),
    /** A multiplexed connection shared through the HTTP/2 registry. */
    Http2(std::sync::Arc<h2::Session>),
}

impl Channel {
    /** The exact admitted address of the connection. */
    fn peer(&self) -> Option<std::net::SocketAddr> {
        match self {
            Self::Http1(connection) => connection.peer(),
            Self::Http2(session) => session.peer(),
        }
    }

    /** HTTP/1.1 sockets get a lease that closes them at the deadline. */
    fn lease(&self, deadline: Instant) -> EngineResult<Option<lease::Lease>> {
        match self {
            Self::Http1(connection) => connection.lease(deadline).map(Some),
            Self::Http2(_) => Ok(None),
        }
    }
}

/** Why a send failed, and whether a reused socket may be replaced and the GET resent. */
enum Failure {
    /** The server did not process the request; it most likely closed an idle connection. */
    Retryable(EngineError),
    /** Response bytes arrived; resending could repeat what the server already answered. */
    Final(EngineError),
}
impl Failure {
    /** The user-facing error either way. */
    fn error(self) -> EngineError {
        match self {
            Self::Retryable(error) | Self::Final(error) => error,
        }
    }
}

impl Prepared {
    /// Validate, consult the cache, then reuse or open a connection and negotiate TLS (ALPN
    /// h2 or http/1.1) without sending HTTP application bytes. Only GETs reuse pooled or
    /// shared connections or cached answers.
    pub fn connect(
        request: WebRequest,
        grants: &[String],
        remaining_ms: u64,
    ) -> EngineResult<Self> {
        request.validate()?;
        let url = web_url(&request.url)?;
        let budget = std::time::Duration::from_millis(remaining_ms)
            .saturating_sub(super::constants::REPLY_MARGIN)
            .min(WEB_TIMEOUT);
        let deadline = Instant::now() + budget;
        let fixture = fixture_granted(&url, grants);
        let key = cache::Key::of(&request, &url, fixture);
        let get = request.method == Method::Get;
        let lookup = match key.as_ref().filter(|_| get) {
            Some(key) => cache::lookup(key, &request, SystemTime::now()),
            None => cache::Lookup::Miss,
        };
        let stale = match lookup {
            cache::Lookup::Fresh(stored) => {
                let source = Source::Cached(stored);
                return Ok(Self {
                    request,
                    url,
                    deadline,
                    fixture,
                    key,
                    source,
                });
            }
            cache::Lookup::Stale(stored) => Some(stored),
            cache::Lookup::Miss => None,
        };
        let (channel, reused) = acquire(&url, fixture, get, true, deadline)?;
        let lease = channel.lease(deadline)?;
        let source = Source::Live {
            channel,
            reused,
            stale,
            lease,
        };
        Ok(Self {
            request,
            url,
            deadline,
            fixture,
            key,
            source,
        })
    }

    /// Which protocol the response will arrive over: `cache` for a fresh cache hit, otherwise
    /// `h2` or `http/1.1`. Diagnostics and measurement only.
    pub fn protocol(&self) -> &'static str {
        match &self.source {
            Source::Cached(_) => "cache",
            Source::Live {
                channel: Channel::Http2(_),
                ..
            } => "h2",
            Source::Live { .. } => "http/1.1",
        }
    }

    /// Consuming the token prevents replay or hidden automatic redirects. A reused connection
    /// the server closed while idle, or one that refused the stream, is replaced once, only
    /// when the server provably did not process the request.
    pub fn transmit(self) -> EngineResult<WebResponse> {
        let Self {
            request,
            url,
            deadline,
            fixture,
            key,
            source,
        } = self;
        let (channel, reused, stale, lease) = match source {
            Source::Cached(stored) => {
                return finish(
                    url,
                    HTTP_OK,
                    stored.headers,
                    stored.body,
                    Vec::new(),
                    &request,
                );
            }
            Source::Live {
                channel,
                reused,
                stale,
                lease,
            } => (channel, reused, stale, lease),
        };
        let conditional = stale
            .as_ref()
            .map(cache::Stored::validators)
            .unwrap_or_default();
        let get = request.method == Method::Get;
        let (response, address) =
            match send(channel, &url, fixture, &request, &conditional, deadline) {
                Ok(sent) => sent,
                Err(Failure::Retryable(_)) if reused => {
                    drop(lease);
                    let (fresh, _) = acquire(&url, fixture, get, false, deadline)?;
                    let _lease = fresh.lease(deadline)?;
                    send(fresh, &url, fixture, &request, &conditional, deadline)
                        .map_err(Failure::error)?
                }
                Err(failure) => return Err(failure.error()),
            };
        let now = SystemTime::now();
        let body::Response {
            status,
            headers,
            set_cookies,
            body,
            ..
        } = response;
        // Only a GET is stored. An unsafe method may have changed the resource, so it also drops
        // the GET cached for its URL; a HEAD's empty answer must never stand in for the GET's.
        if request.method != Method::Get {
            if !request.method.is_safe() {
                key.iter().for_each(cache::invalidate);
            }
            return finish(url, status, headers, body, set_cookies, &request);
        }
        match (status, stale, key, address) {
            (304, Some(stale), Some(_), _) => {
                let fresh = cache::freshen(stale, &request, &headers, now);
                finish(
                    url,
                    HTTP_OK,
                    fresh.headers,
                    fresh.body,
                    set_cookies,
                    &request,
                )
            }
            (HTTP_OK, _, Some(key), Some(address)) => {
                cache::store(key, &request, &headers, &body, address, now);
                finish(url, status, headers, body, set_cookies, &request)
            }
            _ => finish(url, status, headers, body, set_cookies, &request),
        }
    }
}

/**
 * A connection for one exchange and whether it was reused. HTTPS GETs first look for the
 * origin's HTTP/2 connection (waiting briefly if another request is opening it), then an idle
 * HTTP/1.1 socket when `pooled`, then connect. Every other method (POST, HEAD, PUT, PATCH,
 * DELETE, OPTIONS) always connects afresh and never shares, so it is never retried as reused.
 */
/** Raster formats the native painter decodes; CDNs negotiate WebP from our Accept header. */
const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];

fn acquire(
    url: &Url,
    fixture: bool,
    get: bool,
    pooled: bool,
    deadline: Instant,
) -> EngineResult<(Channel, bool)> {
    let claim = match (get && url.scheme() == "https")
        .then(|| h2::REGISTRY.acquire(url, fixture, deadline))
    {
        Some(h2::Acquired::Session(session)) => return Ok((Channel::Http2(session), true)),
        Some(h2::Acquired::Claim(claim)) => Some(claim),
        Some(h2::Acquired::Http1) | None => None,
    };
    if let Some(connection) = (get && pooled)
        .then(|| pool::POOL.checkout(url, fixture))
        .flatten()
    {
        claim.into_iter().for_each(|claim| claim.finish(None));
        return Ok((Channel::Http1(connection), true));
    }
    let channel = open(url, fixture, deadline)?;
    if let Some(claim) = claim {
        claim.finish(match &channel {
            Channel::Http2(session) => Some(session.clone()),
            Channel::Http1(_) => None,
        });
    }
    Ok((channel, false))
}

/** A fresh connection; HTTP/2 when ALPN chose it, which then sends its preface. */
fn open(url: &Url, fixture: bool, deadline: Instant) -> EngineResult<Channel> {
    let connection = transport::connect(url, fixture, deadline)?;
    if connection.protocol() == "h2" {
        return h2::Session::start(connection, deadline).map(Channel::Http2);
    }
    Ok(Channel::Http1(connection))
}

/**
 * One exchange on either protocol, returning the response and the peer address. Reusable
 * HTTP/1.1 GET sockets go back to the pool; HTTP/2 connections stay in the registry.
 */
fn send(
    channel: Channel,
    url: &Url,
    fixture: bool,
    request: &WebRequest,
    conditional: &[(&'static str, String)],
    deadline: Instant,
) -> Result<(body::Response, Option<std::net::IpAddr>), Failure> {
    let address = channel.peer().map(|peer| peer.ip());
    let limit = if request.image {
        IMAGE_BYTES
    } else {
        HTML_BYTES
    };
    let response = match channel {
        Channel::Http2(session) => session.exchange(url, request, conditional, limit, deadline)?,
        Channel::Http1(connection) => {
            let (response, connection) =
                send_http1(connection, url, request, conditional, deadline)?;
            if response.reusable && request.method == Method::Get && request.preflight.is_none() {
                pool::POOL.checkin(url, fixture, connection);
            }
            response
        }
    };
    Ok((response, address))
}

/**
 * Writes the request and reads the response. A write failure, a read error or EOF before the
 * first response byte is retryable; anything after the first byte is final.
 */
fn send_http1(
    mut connection: transport::Connection,
    url: &Url,
    request: &WebRequest,
    conditional: &[(&'static str, String)],
    deadline: Instant,
) -> Result<(body::Response, transport::Connection), Failure> {
    connection
        .write_web_request(url, request, conditional, deadline)
        .map_err(Failure::Retryable)?;
    let mut first = vec![0; transport::READ_CHUNK];
    let count = connection
        .read(&mut first, deadline)
        .map_err(Failure::Retryable)?;
    if count == 0 {
        return Err(Failure::Retryable(body::truncated()));
    }
    first.truncate(count);
    // Chromium's PreflightController downloads a preflight's headers only, so a preflight
    // body that never ends cannot stall the request behind it (the connection is not reused).
    let headers_only = request.method == Method::Head || request.preflight.is_some();
    let response = body::response(
        &mut connection,
        deadline,
        request.image,
        headers_only,
        first,
    )
    .map_err(Failure::Final)?;
    Ok((response, connection))
}

/**
 * Fresh and cached responses alike pass the same media type, image and charset checks, and a
 * cached body is held to the current body budget. A top-level download keeps its raw bytes.
 */
fn finish(
    url: Url,
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
    set_cookies: Vec<String>,
    request: &WebRequest,
) -> EngineResult<WebResponse> {
    // Fonts arrive as bytes like images, under the same bound, but are not typed as images.
    let image_data = request.image || request.destination == Destination::Font;
    let limit = if image_data { IMAGE_BYTES } else { HTML_BYTES };
    if body.len() > limit {
        return Err(EngineError::limit("HTTP body bytes", limit));
    }
    let (mime, charset) = headers
        .get("content-type")
        .and_then(|value| mime::parse(value))
        .map(|parsed| (parsed.essence, parsed.charset))
        .unwrap_or_default();
    if request.image && !IMAGE_TYPES.contains(&mime.as_str()) {
        return Err(EngineError::unsupported(
            "Only PNG, JPEG, WebP and GIF images are supported.",
        ));
    }
    if request.destination == Destination::Document
        && let Some(accept) = headers.get("accept-ch")
    {
        hints::remember(&url, accept);
    }
    let download = status == HTTP_OK && downloads(request, &headers, &mime);
    let charset = mime::html_charset(&mime, charset, &body);
    let declared = charset
        .as_deref()
        .and_then(mars_core::http::encoding_for_label)
        .filter(|_| !image_data && !download)
        .map(str::to_owned);
    Ok(WebResponse {
        url: url.into(),
        status,
        source: if image_data || download {
            base64(&body)
        } else {
            mime::decode(body, charset.as_deref())?
        },
        image_data,
        mime,
        location: headers.get("location").cloned(),
        set_cookies,
        charset: declared,
        download,
        disposition: headers
            .get("content-disposition")
            .filter(|_| download)
            .cloned(),
        headers: if request.destination == Destination::Empty {
            super::cors::response_headers(&headers)
        } else {
            Vec::new()
        },
    })
}

/**
 * Whether a response is a file to save rather than a page: only a top-level GET, and only when the
 * server says `attachment` or sends a type a browser tab cannot show. Text, images, JSON, XML and
 * PDF stay navigations (Chrome renders them inline), so an ordinary link never turns into a file.
 */
fn downloads(request: &WebRequest, headers: &BTreeMap<String, String>, mime: &str) -> bool {
    if request.image
        || request.method != Method::Get
        || request.destination != Destination::Document
    {
        return false;
    }
    let attachment = headers.get("content-disposition").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("attachment"))
    });
    attachment || !(mime.is_empty() || displayable(mime))
}

/** Types a tab renders inline instead of downloading. */
fn displayable(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime.starts_with("image/")
        || mime.ends_with("+xml")
        || mime.ends_with("+json")
        || ["application/json", "application/xml", "application/pdf"].contains(&mime)
}

/// Every redirect and DNS answer passes policy before a request is sent.
pub fn fetch(value: &str, fixture_origins: &[String]) -> EngineResult<WebPage> {
    let mut url = web_url(value)?;
    let deadline = Instant::now() + WEB_TIMEOUT;
    for redirects in 0..=REDIRECTS {
        let response = exchange(&url, fixture_origins, deadline)?;
        if response.status == HTTP_OK {
            return complete(response, url);
        }
        if redirects == REDIRECTS || ![301, 302, 303, 307, 308].contains(&response.status) {
            return Err(EngineError::unsupported(
                "Unsupported HTTP status or redirect limit exceeded.",
            ));
        }
        url = redirect(&url, &response.headers, fixture_origins)?;
    }
    Err(EngineError::unsupported("Redirect limit exceeded."))
}

/** Header parsing and body decoding stay owned by Oya rather than an HTTP client crate. */
fn exchange(url: &Url, grants: &[String], deadline: Instant) -> EngineResult<body::Response> {
    let fixture = fixture_granted(url, grants);
    let channel = open(url, fixture, deadline)?;
    let request = WebRequest::get(url.as_str());
    send(channel, url, fixture, &request, &[], deadline)
        .map(|(response, _)| response)
        .map_err(Failure::error)
}

/** Bounded binary bodies cross the private JSON pipe without UTF-8 loss. */
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

/** Final bodies must have the documented media type and encoding. */
fn complete(response: body::Response, url: Url) -> EngineResult<WebPage> {
    let value = response
        .headers
        .get("content-type")
        .ok_or_else(|| EngineError::unsupported("The response needs Content-Type: text/html."))?;
    let content_type = mime::parse(value)
        .filter(|parsed| parsed.essence == "text/html")
        .ok_or_else(|| EngineError::unsupported("Only text/html documents are supported."))?;
    let charset = mime::html_charset(&content_type.essence, content_type.charset, &response.body);
    let source = mime::decode(response.body, charset.as_deref())?;
    let canonical = charset
        .as_deref()
        .and_then(mars_core::http::encoding_for_label)
        .map(str::to_owned);
    Ok(WebPage {
        source,
        url: url.to_string(),
        charset: canonical,
    })
}

/** Redirects cannot downgrade HTTPS or gain access to another private fixture origin. */
fn redirect(url: &Url, headers: &BTreeMap<String, String>, grants: &[String]) -> EngineResult<Url> {
    let location = headers
        .get("location")
        .ok_or_else(|| EngineError::unsupported("Redirect has no Location."))?;
    redirect_url(url, location, grants)
}
/// Every coordinator-managed redirect repeats the same downgrade and fixture-origin policy: a
/// redirect out of a granted fixture may reach a private destination only when that exact
/// origin is granted too (WPT's cross-origin redirects between its own test hosts).
pub fn redirect_url(url: &Url, location: &str, grants: &[String]) -> EngineResult<Url> {
    if location.chars().any(char::is_control) || location.contains('\\') {
        return Err(super::policy::denied());
    }
    let next = web_url(
        url.join(location)
            .map_err(|_| super::policy::denied())?
            .as_str(),
    )?;
    if url.scheme() == "https" && next.scheme() != "https" {
        return Err(super::policy::denied());
    }
    if fixture_granted(url, grants)
        && next.origin() != url.origin()
        && !fixture_granted(&next, grants)
        && next.host_str().is_some_and(|host| {
            let host = host.trim_matches(['[', ']']);
            super::policy::local_hostname(host)
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| !super::policy::public_address(ip))
        })
    {
        return Err(super::policy::denied());
    }
    Ok(next)
}
