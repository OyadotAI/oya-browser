/**
 * Error reporting to Sentry, set up the way a2abase does it: off unless
 * SENTRY_DSN is set, so a self-hosted server sends nothing; no personal data;
 * and anything shaped like a credential is scrubbed before an event leaves.
 * Sentry's own alert rules are what carry an error to Slack.
 */
import * as Sentry from '@sentry/node';
import { SECRET_MIN_CHARS } from './constants.ts';
import { RELEASE_VERSION } from './version.ts';

/** Header names whose values never leave the process. */
const SECRET_HEADER = /authorization|cookie|api-key|token|secret/i;

/** An API key, bearer token or similar inside free text: a long unbroken base64url run. */
const KEYISH = new RegExp(`[A-Za-z0-9_-]{${SECRET_MIN_CHARS},}`, 'g');

/** Placeholder for a value that was removed. */
const REDACTED = '[redacted]';

/** Free text with key-shaped runs replaced. */
const clean = (text?: string) => text?.replace(KEYISH, REDACTED);

/** The request with secret headers redacted, cookies dropped and its URL and query cleaned. */
function scrubRequest(request?: Sentry.ErrorEvent['request']) {
  if (!request) return;
  const headers = request.headers ?? {};
  for (const name of Object.keys(headers)) if (SECRET_HEADER.test(name)) headers[name] = REDACTED;
  delete request.cookies;
  request.url = clean(request.url);
  if (typeof request.query_string === 'string') request.query_string = clean(request.query_string);
}

/**
 * The event with secrets removed from the fields free text can reach: the
 * request, the message, exception texts and breadcrumbs. Only those: Sentry's
 * own ids (event, trace, span) are also 32 characters and must survive, or the
 * event is rejected.
 */
export function scrub<T extends Sentry.ErrorEvent>(event: T): T {
  scrubRequest(event.request);
  if (event.message) event.message = clean(event.message);
  for (const e of event.exception?.values ?? []) e.value = clean(e.value);
  for (const b of event.breadcrumbs ?? []) Object.assign(b, { message: clean(b.message), data: cleanData(b.data) });
  return event;
}

/** A breadcrumb's data with its URL cleaned, the one field there that carries request text. */
const cleanData = (data?: Record<string, unknown>) =>
  data && typeof data.url === 'string' ? { ...data, url: clean(data.url) } : data;

/** Starts reporting when a DSN is configured; returns whether it did. */
export function startSentry(env: NodeJS.ProcessEnv = process.env) {
  if (!env.SENTRY_DSN) return false;
  const environment = env.SENTRY_ENVIRONMENT || 'production';
  Sentry.init({ dsn: env.SENTRY_DSN, environment, release: RELEASE_VERSION, sendDefaultPii: false, beforeSend: scrub });
  return true;
}

/** Reports an unexpected failure under the reference the caller was given; a no-op when Sentry is off. */
export const captureUnexpected = (err: unknown, tags: Record<string, string>) => {
  Sentry.captureException(err, { tags });
};
