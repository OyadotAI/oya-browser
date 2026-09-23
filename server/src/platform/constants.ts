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

/** Version byte at the start of every sealed buffer. */
export const SEALED_VERSION = 1;
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
