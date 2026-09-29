/**
 * Browser error reporting to Sentry. The DSN is read on the server at request
 * time, like PostHog's settings, because one image serves hosted and
 * self-hosted consoles and a DSN baked into the bundle would report for
 * everyone. Nothing personal is sent and key-shaped strings are scrubbed: this
 * page holds administrator credentials.
 */
import type { ErrorEvent } from '@sentry/browser';

/** Shortest unbroken base64url run treated as a possible key: an API key is 32. */
export const SECRET_MIN_CHARS = 32;

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

/** Free text with key-shaped runs replaced. */
const clean = (text?: string) => text?.replace(KEYISH, '[redacted]');

/**
 * The event with keys removed from the fields free text can reach: the page
 * URL, the message, exception texts and breadcrumbs. Only those: Sentry's own
 * ids are 32 characters too and must survive, or the event is rejected.
 */
export function scrub(event: ErrorEvent): ErrorEvent {
  if (event.request) event.request.url = clean(event.request.url);
  if (event.message) event.message = clean(event.message);
  for (const e of event.exception?.values ?? []) e.value = clean(e.value);
  for (const b of event.breadcrumbs ?? []) Object.assign(b, { message: clean(b.message), data: cleanData(b.data) });
  return event;
}

/** A breadcrumb's data with its URL cleaned: fetch and navigation crumbs carry one. */
const cleanData = (data?: Record<string, unknown>) =>
  data && typeof data.url === 'string' ? { ...data, url: clean(data.url) } : data;

/** Starts reporting in the browser; loaded only when a DSN is set, so a console without one fetches nothing. */
export async function startSentry(dsn: string) {
  const Sentry = await import('@sentry/browser');
  Sentry.init({ dsn, sendDefaultPii: false, beforeSend: scrub });
}
