/**
 * Browser error reporting to Sentry. The DSN is read on the server at request
 * time, like PostHog's settings, because one image serves hosted and
 * self-hosted consoles and a DSN baked into the bundle would report for
 * everyone. Nothing personal is sent and key-shaped strings and emails are
 * scrubbed: this page holds administrator credentials. It also shows what
 * pages and agents said (live view, run transcripts), and nothing marks which
 * error text came from there, so free text is cut short and console output is
 * not sent at all.
 */
import type { ErrorEvent } from '@sentry/browser';

/** Shortest unbroken base64url run treated as a possible key: an API key is 32. */
export const SECRET_MIN_CHARS = 32;

/** Sentry 11 collects headers, cookies, bodies, AI prompts and local variables by default; none of it leaves. */
const NO_DATA_COLLECTION = {
  userInfo: false,
  cookies: false,
  httpHeaders: false,
  httpBodies: [],
  urlQueryParams: false,
  genAI: { inputs: false, outputs: false },
  databaseQueryData: false,
  queues: false,
  stackFrameVariables: false,
};

/** An API key, bearer token or similar inside free text. */
const KEYISH = new RegExp(`[A-Za-z0-9_-]{${SECRET_MIN_CHARS},}`, 'g');

/** The DSN the operator set, read at request time; null when unset. */
export const sentryDsn = () => process.env.SENTRY_DSN?.trim() || null;

/** The origin a DSN reports to, for the Content-Security-Policy; null when unset or malformed. */
export function sentryOrigin(dsn = sentryDsn()): string | null {
  try {
    return dsn ? new URL(dsn).origin : null;
  } catch {
    return null;
  }
}

/** An email address inside free text. */
const EMAIL = /[^\s@<>"'()[\]]+@[^\s@<>"'()[\]]+\.[A-Za-z]{2,}/g;

/** Characters of an error's text that are kept: enough to tell errors apart, too few to carry a page. */
const TEXT_MAX_CHARS = 200;

/** Free text with key-shaped runs and emails replaced, then cut short. */
const clean = (text?: string) =>
  text?.replace(KEYISH, '[redacted]').replace(EMAIL, '[redacted]').slice(0, TEXT_MAX_CHARS);

/** Breadcrumb categories never sent: console lines echo whatever was logged, page and agent text included. */
const DROPPED_CRUMBS = new Set(['console']);

/**
 * The event with keys, emails and long text removed from the fields free text
 * can reach: the page URL, the message, exception texts and breadcrumbs. Only
 * those: Sentry's own ids are 32 characters too and must survive, or the event
 * is rejected.
 */
export function scrub(event: ErrorEvent): ErrorEvent {
  if (event.request) event.request.url = clean(event.request.url);
  if (event.message) event.message = clean(event.message);
  for (const e of event.exception?.values ?? []) e.value = clean(e.value);
  if (event.breadcrumbs) event.breadcrumbs = event.breadcrumbs.filter((b) => !DROPPED_CRUMBS.has(b.category ?? ''));
  for (const b of event.breadcrumbs ?? []) Object.assign(b, { message: clean(b.message), data: cleanData(b.data) });
  return event;
}

/** A breadcrumb's data with its URL cleaned: fetch and navigation crumbs carry one. */
const cleanData = (data?: Record<string, unknown>) =>
  data && typeof data.url === 'string' ? { ...data, url: clean(data.url) } : data;

/** Starts reporting in the browser; loaded only when a DSN is set, so a console without one fetches nothing. */
export async function startSentry(dsn: string) {
  const Sentry = await import('@sentry/browser');
  Sentry.init({ dsn, dataCollection: NO_DATA_COLLECTION, beforeSend: scrub });
}
