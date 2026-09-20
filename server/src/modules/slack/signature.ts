/**
 * Checking that a request came from Slack, and comparing secrets without
 * leaking their contents through timing.
 */
import { createHmac, timingSafeEqual } from 'node:crypto';
import { MS_PER_SECOND, SIGNATURE_WINDOW_SECONDS } from './constants.ts';

/** Constant-time string comparison; unequal lengths are simply unequal. */
export function sameText(x, y) {
  const a = Buffer.from(x),
    b = Buffer.from(y);
  return a.length === b.length && timingSafeEqual(a, b);
}

/** Whether a Slack timestamp falls outside the replay window. */
const stale = (timestamp) => Math.abs(Date.now() / MS_PER_SECOND - Number(timestamp)) > SIGNATURE_WINDOW_SECONDS;

/** Slack's v0 signature over the raw request body, with the five-minute replay window. */
export function verifySignature(rawBody, headers, secret = process.env.SLACK_SIGNING_SECRET) {
  if (!secret) return false;
  const timestamp = headers['x-slack-request-timestamp'],
    signature = headers['x-slack-signature'];
  if (!timestamp || !signature) return false;
  if (stale(timestamp)) return false;
  const expected = `v0=${createHmac('sha256', secret).update(`v0:${timestamp}:${rawBody}`).digest('hex')}`;
  return sameText(expected, String(signature));
}
