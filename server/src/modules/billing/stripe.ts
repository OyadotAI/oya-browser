/**
 * The few Stripe calls billing makes, by plain fetch: the SDK would be a
 * dependency for four endpoints. Also checks a webhook's signature, which is
 * what proves an event came from Stripe.
 */
import { createHmac, timingSafeEqual } from 'node:crypto';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MS_PER_SECOND, OUTBOUND_TIMEOUT_MS } from '../../platform/constants.ts';
import { STRIPE_API, STRIPE_VERSION, WEBHOOK_TOLERANCE_S } from './constants.ts';

/** Posts to Stripe; the one thing the services need from it. */
export type Stripe = {
  /** Posts `params` to `path` and answers the parsed body. */
  post(path: string, params?: Record<string, unknown>): Promise<any>;
};

/** Stripe's form encoding: nested objects and arrays as `a[b][0]=v`. */
export function formEncode(params: Record<string, unknown>, prefix = '', out = new URLSearchParams()) {
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    const name = prefix ? `${prefix}[${k}]` : k;
    if (typeof v === 'object') formEncode(v as Record<string, unknown>, name, out);
    else out.append(name, String(v));
  }
  return out;
}

/** A client for the key `key()` names, calling through `fetchFn` (a fake in tests). */
export function stripeClient(key: () => string, fetchFn: typeof fetch = fetch): Stripe {
  return { post: (path, params = {}) => call(fetchFn, key(), path, params) };
}

/** The request for one call: authorized with the key and pinned to the API version. */
const request = (key: string, params: Record<string, unknown>) => ({
  method: 'POST',
  headers: { authorization: `Bearer ${key}`, 'stripe-version': STRIPE_VERSION },
  body: formEncode(params),
  signal: AbortSignal.timeout(OUTBOUND_TIMEOUT_MS),
});

/** One POST; Stripe's own error message comes back as a 502. */
async function call(fetchFn: typeof fetch, key: string, path: string, params: Record<string, unknown>) {
  const res = await fetchFn(`${STRIPE_API}${path}`, request(key, params));
  const body: any = await res.json().catch(() => ({}));
  if (!res.ok) throw new HttpError(Status.BAD_GATEWAY, `Stripe: ${body?.error?.message || res.status}`);
  return body;
}

/** The timestamp and v1 signatures of a `Stripe-Signature` header. */
function parseHeader(header: string) {
  const t = Number(/(?:^|,)t=(\d+)/.exec(header)?.[1]);
  const signatures = [...header.matchAll(/(?:^|,)v1=([0-9a-f]+)/g)].map((m) => Buffer.from(m[1], 'hex'));
  return { t, signatures };
}

/** Whether `raw` was signed by Stripe with `secret`, recently enough not to be a replay. */
export function verifySignature(raw: Buffer | string, header: string, secret: string, nowMs = Date.now()) {
  const { t, signatures } = parseHeader(String(header || ''));
  if (!secret || !t || Math.abs(nowMs / MS_PER_SECOND - t) > WEBHOOK_TOLERANCE_S) return false;
  const expected = createHmac('sha256', secret).update(`${t}.`).update(raw).digest();
  return signatures.some((s) => s.length === expected.length && timingSafeEqual(s, expected));
}
