/**
 * The numbers the telemetry module runs on.
 */
import { MS_PER_HOUR } from '../../platform/constants.ts';

/** How long a key's owner and email are remembered, so a lookup never sits on a request path twice an hour. */
export const WHO_TTL_MS = MS_PER_HOUR;
/** Owners remembered at once; past this the oldest go, so a key-churning tenant cannot grow memory. */
export const WHO_CACHE_MAX = 10_000;
/** Hex characters of a key fingerprint shown when there is no owner: enough to tell keys apart, never to use one. */
export const WHO_FINGERPRINT_CHARS = 8;
/** Characters of a project id shown in an ops line. */
export const PROJECT_ID_CHARS = 8;
/** Width of the rule above and below a product card, matching A2ABase's cards. */
export const CARD_DIVIDER_CHARS = 22;
/** Sessions shorter than this get no card: a desktop reconnecting after sleep is not a session anyone ran. */
export const CARD_MIN_SESSION_SECONDS = 60;
/** The clients a start may say it came from; anything else is `rest`. */
export const CLIENTS = new Set(['mcp', 'console']);
/** The header a client names itself in. */
export const CLIENT_HEADER = 'x-oya-client';
/** Past this many cloud browsers at once, an unlicensed self-hosted install is worth a line in Slack. */
export { SELF_HOST_FREE_CAP } from '../../platform/license/index.ts';
/** The largest browser count a ping may report: anything above is a malformed ping. */
export const PING_MAX_COUNT = 1_000_000;
/** Keel's step meter is an unsigned 32-bit count. */
export const KEEL_MAX_STEPS = 4_294_967_295;
/** The version field stays bounded even when it includes a prerelease suffix. */
export const KEEL_VERSION_MAX_LENGTH = 32;
/** Characters in the UTC usage day. */
export const KEEL_DAY_CHARS = 10;
/** Versioned reports carry completed days in small batches through the existing admission limit. */
export const KEEL_SCHEMA_VERSION = 2;
/** Days in one delivery; the client retains a bounded backlog and drains it incrementally. */
export const KEEL_BATCH_DAYS = 7;
/** Distinct project identities an installation can accurately report for one day. */
export const KEEL_PROJECTS_PER_DAY = 128;
/** Runtime supports concurrent agent lanes while staying safely below JavaScript's integer limit. */
const KEEL_HOURS_PER_DAY = 24;
/** Concurrent lanes supported by the runtime allowance. */
const KEEL_MAX_LANES = 4096;
/** Largest accepted daily runtime across concurrent lanes. */
export const KEEL_MAX_RUNTIME_MS = MS_PER_HOUR * KEEL_HOURS_PER_DAY * KEEL_MAX_LANES;
/** Maximum serialized telemetry body, including opaque identifiers and all batch days. */
export const KEEL_REPORT_MAX_BYTES = 65_536;
/** Cents in a dollar, for amounts said in Slack. */
export const CENTS = 100;
/** Digits after the point in an amount of money. */
export const CENT_DIGITS = 2;

/** The longest download source or client name kept, matching the site's limit (ui/src/lib/constants.ts). */
export const SOURCE_MAX_LENGTH = 80;
