/**
 * Every number the platform layer runs on, by name: time units, batch sizes,
 * buffer caps, crypto sizes and the defaults behind operator-tunable limits.
 * The environment is read by the file that owns each setting, so it is read
 * at the same moment it always was.
 */

// ── Units ──

/** Milliseconds in a second. */
export const MS_PER_SECOND = 1000;
/** Milliseconds in a minute. */
export const MS_PER_MINUTE = 60_000;
/** Milliseconds in an hour. */
export const MS_PER_HOUR = 3_600_000;
/** Seconds in a minute. */
export const SECONDS_PER_MINUTE = 60;
/** Seconds in an hour. */
export const SECONDS_PER_HOUR = 3600;
/** Nanoseconds in a millisecond; the event loop monitor reports nanoseconds. */
export const NS_PER_MS = 1e6;
/** Bytes in a mebibyte. */
export const BYTES_PER_MIB = 1_048_576;
/** Indentation of the JSON files the platform writes, so they stay readable. */
export const JSON_INDENT = 2;
/** Characters of `Bearer ` before the token in an Authorization header. */
export const BEARER_PREFIX_LENGTH = 'Bearer '.length;

// ── Storage ──

/** Rows per database insert or upsert, so one flush never sends an unbounded request. */
export const DB_BATCH_ROWS = 500;

// ── Audit ──

/** Hex characters of a key's SHA-256 kept as its fingerprint. */
export const FINGERPRINT_HEX_CHARS = 16;
/** Longest target id or user agent an audit event keeps. */
export const AUDIT_FIELD_MAX_CHARS = 200;

/** Digest that links each audit row to the one before it. */
export const AUDIT_HASH_ALGORITHM = 'sha256';

/** Length of a chain id: enough that two instances never pick the same one. */
export const AUDIT_CHAIN_ID_BYTES = 8;

/** Characters in a hex sha256 digest. */
export const SHA256_HEX_CHARS = 64;

/** What the first row in a chain links to. */
export const AUDIT_GENESIS_HASH = '0'.repeat(SHA256_HEX_CHARS);
/** Most rows one durable audit history query returns. */
export const AUDIT_HISTORY_MAX_ROWS = 1000;
/** Keyed digest used on chains written since the chain became an HMAC. */
export const AUDIT_HMAC_ALGORITHM = 'sha256';
/**
 * Chain ids that start with this are HMAC chains. Older chains are plain
 * SHA-256, which anyone with write access can recompute, so the verifier counts
 * them as unkeyed rather than trusting them.
 */
export const AUDIT_HMAC_CHAIN_PREFIX = 'h-';
/** Purpose label for the chain key derived from the server secret when OYA_AUDIT_HMAC_KEY is unset. */
export const AUDIT_HMAC_KEY_PURPOSE = 'oya-audit-chain-v1';
/** Operator-supplied chain key; read here and nowhere else, at use, so setting it later is seen. Unset derives one from the server secret. */
export const auditHmacKey = () => process.env.OYA_AUDIT_HMAC_KEY || '';
/**
 * Most pending audit events kept in memory. Past it, events are appended to
 * the overflow file on disk rather than dropped.
 */
export const AUDIT_PENDING_MAX = 5000;
/** Milliseconds audit events wait to be written together. */
export const AUDIT_FLUSH_DELAY_MS = 2000;
/** Audit events kept in memory for recent(). */
export const AUDIT_RING_MAX = 500;
/** Newest stored audit rows the startup check verifies. */
export const AUDIT_VERIFY_MAX_ROWS = 50_000;
/** File mode of the audit overflow file: owner read and write only. */
export const AUDIT_OVERFLOW_FILE_MODE = 0o600;
/** How often a chain head is anchored to external storage unless OYA_AUDIT_ANCHOR_INTERVAL_MS says otherwise: 15 minutes. */
export const DEFAULT_AUDIT_ANCHOR_INTERVAL_MS = 900_000;
/** Milliseconds between anchors; read here and nowhere else. */
export const auditAnchorIntervalMs = () =>
  Number(process.env.OYA_AUDIT_ANCHOR_INTERVAL_MS) || DEFAULT_AUDIT_ANCHOR_INTERVAL_MS;
/** Storage bucket chain heads are anchored to; unset turns anchoring off. Read here and nowhere else. */
export const auditAnchorBucket = () => process.env.OYA_AUDIT_ANCHOR_BUCKET || '';
/** Days of anchors the startup check reads: chains whose last anchor is older are not compared. */
export const AUDIT_ANCHOR_VERIFY_DAYS = 30;
/** Most anchor objects listed from one day's folder. */
export const AUDIT_ANCHOR_LIST_MAX = 10_000;
/** Digits a sequence number is padded to in an anchor's object name, so names sort by position. */
export const AUDIT_ANCHOR_SEQ_DIGITS = 12;
/** Characters of an ISO timestamp that make its date: the anchor's day folder. */
export const ISO_DATE_CHARS = 10;
/** Milliseconds in a day, for the anchor lookback. */
export const AUDIT_DAY_MS = 86_400_000;

// ── Usage ──

/** Characters of a key fingerprint shown in the usage snapshot. */
export const USAGE_ACTOR_CHARS = 12;
/** How often usage counters are flushed, unless OYA_USAGE_FLUSH_MS says otherwise. */
export const DEFAULT_USAGE_FLUSH_MS = 60_000;

// ── Rate limits and quotas (0 disables one) ──

/** Commands a key may send per minute. */
export const DEFAULT_COMMANDS_PER_MIN = 600;
/** Commands a key may send in one burst. */
export const DEFAULT_COMMANDS_BURST = 120;
/** Chat requests a key may send per minute. */
export const DEFAULT_CHAT_PER_MIN = 60;
/** Chat requests a key may send in one burst. */
export const DEFAULT_CHAT_BURST = 10;
/** Browsers a key may provision per minute. */
export const DEFAULT_PROVISION_PER_MIN = 20;
/** Browsers a key may provision in one burst. */
export const DEFAULT_PROVISION_BURST = 20;
/** Browser connections a key may open per minute. */
export const DEFAULT_CONNECT_PER_MIN = 120;
/** Browser connections a key may open in one burst. */
export const DEFAULT_CONNECT_BURST = 60;
/** New agent keys one caller address may sign up for in a day. */
export const DEFAULT_AGENT_SIGNUPS_PER_DAY = 3;
/** Install pings one address may send in an hour: a server sends one a day. */
export const DEFAULT_INSTALL_PINGS_PER_HOUR = 4;
/** Password sign-in attempts one email may make in an hour, so a password cannot be guessed online. */
export const DEFAULT_LOGINS_PER_HOUR = 10;
/** Minutes in an hour. */
export const MINUTES_PER_HOUR = 60;
/** Minutes in a day, for limits counted per day on a per-minute bucket. */
export const MINUTES_PER_DAY = 1440;
/** Browsers one key may hold at once. */
export const DEFAULT_MAX_BROWSERS = 5000;
/** Chat tokens one key may spend in an hour. */
export const DEFAULT_CHAT_TOKENS_PER_HOUR = 2_000_000;
/** Sandboxes one key may create in an hour. */
export const DEFAULT_SANDBOXES_PER_HOUR = 500;
/** How often idle, full rate-limit buckets are reclaimed. */
export const LIMIT_SWEEP_MS = 600_000;

// ── Metrics ──

/** Series one metric may hold; a runaway label set past this is dropped. */
export const MAX_SERIES_PER_METRIC = 2000;
/**
 * Histogram bucket upper bounds in milliseconds. Wide, because a navigate
 * legitimately takes tens of seconds while a frame capture takes single-digit ms.
 */
const HISTOGRAM_BOUNDS_MS = {
  ms5: 5,
  ms10: 10,
  ms25: 25,
  ms50: 50,
  ms100: 100,
  ms250: 250,
  ms500: 500,
  s1: 1000,
  s2_5: 2500,
  s5: 5000,
  s10: 10000,
  s30: 30000,
  s60: 60000,
};
/** The histogram buckets, in ascending order. */
export const HISTOGRAM_BUCKETS = Object.values(HISTOGRAM_BOUNDS_MS);
/** Median, for the dashboard snapshot. */
export const P50 = 0.5;
/** 95th percentile, for the dashboard snapshot. */
export const P95 = 0.95;
/** 99th percentile, for the dashboard snapshot. */
export const P99 = 0.99;
/** The event-loop lag percentile exported as a gauge. */
export const LOOP_LAG_PERCENTILE = 99;

// ── LLM ──

/** Characters of a failed LLM response logged; the body is never returned to the caller. */
export const LLM_ERROR_LOG_CHARS = 500;
/** How long one model request may take by default: thinking models can spend minutes on a hard step. */
export const DEFAULT_LLM_TIMEOUT_MS = 180_000;
/** How long one model request may take before it is abandoned and, if attempts remain, retried. */
export const LLM_TIMEOUT_MS = Number(process.env.OYA_LLM_TIMEOUT_MS) || DEFAULT_LLM_TIMEOUT_MS;
/** Attempts per model request: a rate limit, an overload or a dropped connection is tried again. */
export const LLM_ATTEMPTS = 4;
/** The first retry's wait; each later one doubles, with jitter, up to LLM_RETRY_MAX_MS. */
export const LLM_RETRY_BASE_MS = 1_000;
/** The longest wait between two attempts, whatever the provider's retry-after says. */
export const LLM_RETRY_MAX_MS = 30_000;
/** Each retry waits this many times longer than the one before. */
export const LLM_BACKOFF_FACTOR = 2;
/** The longest reply Claude may write in one step (thinking included): a step is short, a final report is not long. */
export const CLAUDE_MAX_TOKENS = 16_000;
/** How hard Claude thinks per step; `high` is the API default and the sweet spot for agentic browsing. */
export const CLAUDE_EFFORT = process.env.OYA_CLAUDE_EFFORT || 'high';

// ── Network guard ──

/** isIP()'s answer for an IPv6 address. */
export const IPV6 = 6;
/** First octet from which IPv4 is multicast or reserved. */
export const IPV4_MULTICAST_FIRST_OCTET = 224;

/** An IPv4 range by its first octet and, when given, an inclusive span of second octets. */
export type Ipv4Range = {
  /** First octet. */
  first: number;
  /** Lowest second octet in the range; any when absent. */
  from?: number;
  /** Highest second octet in the range. */
  to?: number;
};

/** Link-local (the cloud metadata service lives there) and "this network": never dialled. */
export const NEVER_ALLOWED_V4: Ipv4Range[] = [{ first: 169, from: 254, to: 254 }, { first: 0 }];

/** Sixteen-bit groups in a full IPv6 address. */
export const IPV6_GROUPS = 8;
/** Radix of an IPv6 group. */
export const HEX_RADIX = 16;
/** Bits in a byte: an IPv6 group holds two IPv4 octets. */
export const BYTE_BITS = 8;
/** Mask for the low byte of a group. */
export const BYTE_MASK = 0xff;

/** The group that marks an IPv4-mapped or SIIT-translated IPv6 address. */
export const V4_MAPPED_GROUP = 0xffff;
/** The two leading groups of the NAT64 well-known prefix 64:ff9b::/96. */
export const NAT64_GROUPS = { first: 0x64, second: 0xff9b };
/** The leading group of 6to4 (2002::/16), followed by the IPv4 address. */
export const SIX_TO_FOUR_GROUP = 0x2002;

/** An IPv6 prefix, as its leading groups, that carries an IPv4 address in the two groups from `at`. */
export type V4Embedding = {
  /** The leading groups, each matched exactly. */
  prefix: number[];
  /** Index of the group holding the first two IPv4 octets. */
  at: number;
};

/**
 * IPv6 forms that route to, or name, an IPv4 address: judged as that address.
 * Mapped ::ffff:0:0/96, SIIT ::ffff:0:0:0/96, NAT64 64:ff9b::/96, compatible
 * ::/96 and 6to4 2002::/16. WHATWG URL rewrites [::ffff:169.254.169.254] to
 * [::ffff:a9fe:a9fe], so the hex spelling is the one that arrives here.
 */
export const V4_EMBEDDINGS: V4Embedding[] = [
  { prefix: [0, 0, 0, 0, 0, V4_MAPPED_GROUP], at: 6 },
  { prefix: [0, 0, 0, 0, V4_MAPPED_GROUP, 0], at: 6 },
  { prefix: [NAT64_GROUPS.first, NAT64_GROUPS.second, 0, 0, 0, 0], at: 6 },
  { prefix: [0, 0, 0, 0, 0, 0], at: 6 },
  { prefix: [SIX_TO_FOUR_GROUP], at: 1 },
];

/** Ranges that are not safely routable on behalf of a caller (multicast is checked separately). */
export const PRIVATE_V4: Ipv4Range[] = [
  { first: 0 },
  { first: 127 }, // this host, loopback
  { first: 10 }, // RFC1918
  { first: 172, from: 16, to: 31 },
  { first: 192, from: 168, to: 168 },
  { first: 169, from: 254, to: 254 }, // link-local, incl. cloud metadata
  { first: 100, from: 64, to: 127 }, // CGNAT
  { first: 192, from: 0, to: 0 }, // protocol assignments
  { first: 198, from: 18, to: 19 }, // benchmarking
];

// ── Secrets ──

/** Version byte of the first sealed format, which names no key: it opens under any key the server holds. */
export const SEALED_VERSION = 1;
/** Version byte of the keyed format: the version, then the id of the key that wrapped the data key. */
export const SEALED_KEYED_VERSION = 2;
/**
 * Whether new records are written in the keyed format. Off by default: a
 * release before it cannot open keyed records, so turning it on during a
 * rolling update or before a rollback would strand them. Turn it on with
 * OYA_SEAL_KEY_IDS=true once every replica reads it. Both formats always open.
 */
export const sealKeyIds = () => process.env.OYA_SEAL_KEY_IDS === 'true';
/** Bytes of a key id: a fingerprint of the key, enough to pick between current and previous. */
export const KEY_ID_BYTES = 4;
/** AES-256 key size: data keys, the KEK and a generated secret. */
export const KEY_BYTES = 32;
/** AES-GCM nonce size. */
export const NONCE_BYTES = 12;
/** AES-GCM auth tag size. */
export const TAG_BYTES = 16;
/** scrypt cost N = 2^15. */
export const SCRYPT_N = 32_768;
/** scrypt block size. */
export const SCRYPT_R = 8;
/** Memory scrypt may use, in MiB: N=2^15,r=8 needs 32MB, exactly Node's default ceiling. */
const SCRYPT_MAXMEM_MIB = 96;
/** scrypt's memory ceiling in bytes, raised explicitly rather than left to trip at runtime. */
export const SCRYPT_MAXMEM = SCRYPT_MAXMEM_MIB * BYTES_PER_MIB;

// ── Runtime config ──

/** Characters of a saved API key left visible when it is shown masked. */
export const MASKED_KEY_TAIL = 4;

// ── Error answers ──

/** Longest string quoted back in a "must be X, not Y" message; a longer one is called a string. */
export const GOT_MAX_CHARS = 40;
/** Hex characters of the reference a 500 carries, enough to find one log line among a day's. */
export const REF_CHARS = 8;

// ── Outbound analytics and ops messages ──

/** Events held before a flush; past this the newest are dropped, because analytics must never grow memory. */
export const ANALYTICS_PENDING_MAX = 1000;
/** Events per batch POST to PostHog. */
export const ANALYTICS_BATCH_MAX = 50;
/** How long a batch waits for company before it is sent. */
export const ANALYTICS_FLUSH_MS = 10_000;
/** How long one outbound analytics or Slack call may take before it is abandoned. */
export const OUTBOUND_TIMEOUT_MS = 5_000;
/** Longest ops message posted to Slack; a longer one was not written by us. */
export const MESSAGE_MAX_CHARS = 400;
/** Characters in a multi-line product card: a handful of short lines, still bounded. */
export const CARD_MAX_CHARS = 1500;
/** Shortest unbroken base64url run scrubbed from error reports as a possible key: an API key is 32. */
export const SECRET_MIN_CHARS = 32;

// ── Postgres ──

/** Postgres connections per process unless DATABASE_POOL_MAX says otherwise. */
export const DEFAULT_POOL_MAX = 10;
/** Storage is on the request path; a hung connect must not hang a request. */
export const PG_CONNECT_TIMEOUT_MS = 10_000;
/** Idle Postgres connections are closed after this. */
export const PG_IDLE_TIMEOUT_MS = 30_000;
/**
 * How DATABASE_URL's TLS is read when OYA_DB_TLS is unset: `libpq`, its own
 * sslmode as psql reads it (`require` encrypts without verifying). Set
 * OYA_DB_TLS=verify-full to require TLS with the certificate and host checked.
 */
export const DEFAULT_DB_TLS = 'libpq';
