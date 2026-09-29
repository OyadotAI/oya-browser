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

/** The event with secret headers, cookies and key-shaped strings removed. */
export function scrub<T extends Sentry.ErrorEvent>(event: T): T {
  const headers = event.request?.headers ?? {};
  for (const name of Object.keys(headers)) if (SECRET_HEADER.test(name)) headers[name] = REDACTED;
  if (event.request) delete event.request.cookies;
  return JSON.parse(JSON.stringify(event).replace(KEYISH, REDACTED));
}

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
