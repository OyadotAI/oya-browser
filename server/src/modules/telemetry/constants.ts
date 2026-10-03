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
/** Cents in a dollar, for amounts said in Slack. */
export const CENTS = 100;
/** Digits after the point in an amount of money. */
export const CENT_DIGITS = 2;

/** The longest download source or client name kept, matching the site's limit (ui/src/lib/constants.ts). */
export const SOURCE_MAX_LENGTH = 80;
