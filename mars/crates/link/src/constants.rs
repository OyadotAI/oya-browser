/**
 * Every number the link runs on, by name. The persona constants must equal Oya's
 * (server/src/modules/personas/constants.ts); the socket ones match the Electron browser's
 * (browser/src/main/connection/constants.ts) so the server sees the same client behaviour.
 */
use std::time::Duration;

/// Longest host the egress matcher accepts, in UTF-16 units as JavaScript counts them.
pub const MAX_HOST_UTF16: usize = 253;

/// LCG multiplier (Numerical Recipes); must match Oya's.
pub const LCG_MULTIPLIER: u32 = 1_664_525;
/// LCG increment (Numerical Recipes); must match Oya's.
pub const LCG_INCREMENT: u32 = 1_013_904_223;
/// Largest unsigned 32-bit value: the LCG's divisor.
pub const UINT32_MAX: f64 = 4_294_967_295.0;
/// The string hash is `hash * 31 + unit`, done as a shift by 5 minus hash.
pub const HASH_SHIFT: u32 = 5;
/// Upper bound of the canvas noise seed.
pub const CANVAS_NOISE_MAX: f64 = 0.01;
/// Upper bound of the audio noise seed.
pub const AUDIO_NOISE_MAX: f64 = 0.01;
/// Upper bound of the client-rects noise seed.
pub const RECTS_NOISE_MAX: f64 = 0.001;
/// Screen height lost to the taskbar or dock.
pub const TASKBAR_PX: u32 = 40;
/// Colour and pixel depth every profile reports.
pub const COLOR_DEPTH: u32 = 24;

/// Smallest web-exposed logical processor count.
pub const MIN_PROCESSORS: u32 = 1;
/// Largest processor count the native identity accepts.
pub const MAX_PROCESSORS: u32 = 256;
/// Longest locale or timezone accepted.
pub const MAX_POLICY_TEXT: usize = 128;

/// Same browser reconnected; this socket is the old one.
pub const CLOSE_REPLACED: u16 = 4000;
/// No auth message in time.
pub const CLOSE_AUTH_TIMEOUT: u16 = 4001;
/// Bad credential, someone else's browser id, or a session that could not be set up.
pub const CLOSE_REJECTED: u16 = 4003;

/// How often the link pings the server.
pub const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Pings without an answer tolerated before the socket is closed.
pub const MAX_MISSED_PONGS: u32 = 4;
/// First reconnect delay, before backoff, in milliseconds.
pub const RECONNECT_BASE_MS: f64 = 500.0;
/// Each failed reconnect waits this much longer than the last.
pub const RECONNECT_GROWTH: f64 = 1.5;
/// The longest reconnect delay, before jitter, in milliseconds.
pub const RECONNECT_MAX_MS: f64 = 10_000.0;
/// Random extra delay, so a fleet does not reconnect in lockstep, in milliseconds.
pub const RECONNECT_JITTER_MS: u64 = 500;
/// Random bytes in a browser id.
pub const BROWSER_ID_BYTES: usize = 8;
/// How long a pairing claim or socket handshake may take.
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(15);
/// How often a blocked socket read wakes to check the heartbeat and stop flag.
pub const READ_TICK: Duration = Duration::from_millis(250);
/// Largest server frame accepted (16 MiB): a cookie jar or draft is big, a frame past this is not.
pub const MAX_FRAME_BYTES: u64 = 16 * 1024 * 1024;
/// Largest HTTP response head or pairing answer accepted.
pub const MAX_HTTP_BYTES: usize = 64 * 1024;
/// Default HTTPS port.
pub const HTTPS_PORT: u16 = 443;
/// Default HTTP port.
pub const HTTP_PORT: u16 = 80;
/// The HTTP status of a WebSocket upgrade.
pub const SWITCHING_PROTOCOLS: u16 = 101;
/// The lowest HTTP success status.
pub const HTTP_OK_MIN: u16 = 200;
/// One past the highest HTTP success status.
pub const HTTP_OK_END: u16 = 300;
/// Bytes in a WebSocket masking key.
pub const MASK_BYTES: usize = 4;
/// Random bytes in a WebSocket handshake key.
pub const WS_KEY_BYTES: usize = 16;
