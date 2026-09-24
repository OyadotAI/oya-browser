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
