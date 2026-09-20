/**
 * Every number and fixed address the Slack integration runs on, by name.
 */

/** Slack's Web API. */
export const API = 'https://slack.com/api';
/** Where the OAuth install starts. */
export const AUTHORIZE_URL = 'https://slack.com/oauth/v2/authorize';
// Deliberately a subset of what the shared getoya.ai Slack app already declares, so
// that one app serves this install too. `channels:join` rather than `chat:write.public`:
// the bot joins the chosen public channel outright instead of posting from outside it.
/** The bot scopes the install asks for. */
export const SCOPES = 'chat:write,channels:join,channels:read,groups:read';

/** Milliseconds per second, for Slack's second-resolution timestamps. */
export const MS_PER_SECOND = 1000;
/** Milliseconds per minute, for readable lifetimes. */
const MS_PER_MINUTE = 60_000;
/** Minutes an OAuth state stays valid. */
const STATE_TTL_MINUTES = 5;
/** How long an OAuth state (and its cookie) stays valid. */
export const STATE_TTL_MS = STATE_TTL_MINUTES * MS_PER_MINUTE;
/** Random bytes in an OAuth state. */
export const STATE_BYTES = 24;
/** Slack's replay window for signed requests: five minutes. */
export const SIGNATURE_WINDOW_SECONDS = 300;

/** Every call to Slack gives up after this long. */
export const REQUEST_TIMEOUT_MS = 10000;
/** The console's port when nothing says where it is. */
export const DEFAULT_PORT = 3100;
/** Longest message body put in an alert. */
export const MAX_BODY_CHARS = 2500;
/** Channels asked for in one listing. */
export const CHANNEL_PAGE = 200;
/** Trailing characters of a bot token shown in its masked form. */
export const MASK_TAIL = 4;
