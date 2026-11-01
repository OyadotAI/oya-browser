/** Relay-local budgets for connection reuse, the HTTP cache and parallel fetches. */
use std::time::Duration;

/// Relay answers this much before the caller's deadline. `remaining_ms` is measured against
/// the same instant the coordinator stops waiting for the reply, so without a margin Relay's own
/// timeout always lands after it: the coordinator then retires the whole network service as
/// hung (and restarts the engine) instead of receiving an ordinary, degradable timeout.
pub const REPLY_MARGIN: Duration = Duration::from_millis(300);

/// The error code of an image a server sent as AVIF, which the painter does not decode yet.
/// Chrome's image Accept offers AVIF first, so CDNs that negotiate (Airbnb's) send it; the
/// caller asks once more with `ACCEPT_IMAGE_WITHOUT_AVIF` instead of showing nothing.
pub const IMAGE_AVIF_REFUSED: &str = "image_avif_unsupported";

/// Chrome's image Accept without AVIF, for the one retry after `IMAGE_AVIF_REFUSED`; the
/// first request keeps Chrome's exact header.
pub const ACCEPT_IMAGE_WITHOUT_AVIF: &str = "image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";

/// One socket read. Large enough that a 2 MiB body is not thousands of system calls, small
/// enough that the retained cursor buffer stays a fixed, modest allocation.
pub const READ_BYTES: usize = 16 * 1024;

/// Stacked Content-Encoding codings decoded at most. Real servers send one; each extra layer
/// costs a full bounded decode, so a longer list is refused rather than worked through.
pub const CONTENT_CODINGS: usize = 2;

/// An idle pooled connection older than this is closed instead of reused. Servers commonly
/// drop idle keep-alive sockets after 5 to 60 seconds; the liveness probe catches earlier closes.
pub const POOL_IDLE: Duration = Duration::from_secs(30);

/// Idle connections kept per (scheme, host, port, grant). Matches the common browser limit of
/// six, which is also the batch fetch width.
pub const POOL_PER_ORIGIN: usize = 6;

/// Idle connections kept in total, so a page with many origins cannot hold unbounded sockets.
pub const POOL_TOTAL: usize = 24;

/// TLS records drained by the liveness probe before a connection is judged too chatty to reuse.
pub const POOL_PROBE_RECORDS: usize = 8;

/// Memory cache budget, in stored body and header bytes, before least recently used eviction.
pub const CACHE_MEMORY_BYTES: usize = 32 * 1024 * 1024;

/// Disk cache budget, in file bytes, before the oldest files are removed.
pub const CACHE_DISK_BYTES: u64 = 128 * 1024 * 1024;

/// Disk entries kept at most, so the eviction scan of the directory stays bounded.
pub const CACHE_DISK_ENTRIES: usize = 1024;

/// Vary variants kept per URL; older ones are dropped first.
pub const CACHE_VARIANTS: usize = 4;

/// RFC 9111 section 4.2.2: heuristic freshness is this fraction (1/10) of the time since
/// Last-Modified.
pub const HEURISTIC_DIVISOR: u32 = 10;

/// Concurrent workers for a batch of subresource fetches.
pub const BATCH_WORKERS: usize = 6;

/// Cookies kept in total. Chromium's store holds at most 3000 before global eviction.
pub const COOKIE_COUNT: usize = 3000;

/// Cookies kept per registrable domain (eTLD+1). Chromium evicts beyond 180 per domain.
pub const DOMAIN_COOKIES: usize = 180;

/// Name plus value bytes of one cookie. RFC 6265bis section 5.6 and Chromium both cap at 4096.
pub const COOKIE_PAIR_BYTES: usize = 4096;

/// One attribute value; RFC 6265bis section 5.6 ignores longer ones (Chromium: 1024).
pub const COOKIE_ATTRIBUTE_BYTES: usize = 1024;

/// One whole Set-Cookie field: the pair plus a generous allowance for attributes.
pub const COOKIE_FIELD_BYTES: usize = COOKIE_PAIR_BYTES + 4 * COOKIE_ATTRIBUTE_BYTES;

/// Stored name, value, domain and path bytes across the jar before least recently used eviction.
/// Real jars average about a hundred bytes a cookie, so 3000 of them fit with room to spare.
pub const COOKIE_JAR_BYTES: usize = 1024 * 1024;

/// One serialized jar record: the stored bytes plus JSON framing for every cookie.
pub const COOKIE_PROFILE_BYTES: usize = COOKIE_JAR_BYTES + COOKIE_COUNT * 192;

/// RFC 6265bis section 5.5: Expires and Max-Age are clamped to 400 days in the future.
pub const COOKIE_LIFETIME: Duration = Duration::from_secs(400 * 24 * 60 * 60);

/// HTTP/2 SETTINGS_HEADER_TABLE_SIZE Relay sends (Chrome's value). It bounds the HPACK
/// decoder's dynamic table, whatever size updates the server sends.
pub const H2_HEADER_TABLE_BYTES: usize = 65_536;

/// HTTP/2 SETTINGS_INITIAL_WINDOW_SIZE Relay sends (Chrome's value). Like Chrome, Relay sends a
/// stream WINDOW_UPDATE once more than half of it is consumed, so a body may outgrow it; the
/// body budget, not the window, is what cancels an oversized stream.
pub const H2_STREAM_WINDOW: u32 = 6_291_456;

/// HTTP/2 SETTINGS_MAX_HEADER_LIST_SIZE Relay sends (Chrome's value) and enforces, both on the
/// compressed block and on the decoded list.
pub const H2_HEADER_LIST_BYTES: usize = 262_144;

/// The connection WINDOW_UPDATE Chrome sends after SETTINGS: 15 MiB less the default 65535.
pub const H2_CONNECTION_INCREMENT: u32 = 15_663_105;

/// Largest HTTP/2 frame payload accepted or sent. Relay keeps the protocol default for
/// SETTINGS_MAX_FRAME_SIZE, as Chrome does.
pub const H2_FRAME_BYTES: usize = 16_384;

/// Streams Relay keeps open on one connection, whatever larger number the server allows.
pub const H2_STREAMS: usize = 100;

/// CONTINUATION frames accepted per header block, so empty frames cannot stall a connection.
pub const H2_CONTINUATIONS: usize = 64;

/// HTTP/2 connections kept, one per origin, before the least recently used is dropped.
pub const H2_SESSIONS: usize = POOL_TOTAL;

/// Origins remembered as HTTP/1.1-only, so their concurrent requests connect in parallel
/// instead of waiting to see whether the first connection negotiates HTTP/2.
pub const H2_HTTP1_ORIGINS: usize = 256;

// The connection window holds a whole stream window, so one stream's flow is never stalled by
// the connection before its own half-window WINDOW_UPDATE goes out.
const _: () = assert!(H2_CONNECTION_INCREMENT >= H2_STREAM_WINDOW);
// A stream's half-window replenish threshold is reached within its window, frame by frame.
const _: () = assert!(H2_FRAME_BYTES < H2_STREAM_WINDOW as usize / 2);

/// Headers one script request may set. Chromium has no such cap, but real pages set a handful;
/// the bound keeps a page from turning one fetch into an oversized header block.
pub const SCRIPT_REQUEST_HEADERS: usize = 64;

/// Response headers handed to a page's script for one response, after filtering.
pub const SCRIPT_RESPONSE_HEADERS: usize = 64;

/// Fetch standard: a CORS-safelisted request-header value is at most 128 bytes.
pub const SAFELISTED_VALUE_BYTES: usize = 128;

/// Fetch standard: past 1024 safelisted value bytes in total, safelisted headers need a preflight.
pub const SAFELISTED_TOTAL_BYTES: usize = 1024;

/// Fetch standard: a preflight without Access-Control-Max-Age is cached for five seconds.
pub const PREFLIGHT_DEFAULT_AGE: Duration = Duration::from_secs(5);

/// Chromium caps a cached preflight at two hours, whatever Access-Control-Max-Age says.
pub const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(2 * 60 * 60);

/// Preflight results one page keeps; the oldest is dropped first.
pub const PREFLIGHT_ENTRIES: usize = 64;
