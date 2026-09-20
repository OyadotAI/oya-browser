/**
 * Customer webhooks: the signed request body and headers, and the HTTPS POST
 * pinned to the address the SSRF check approved.
 */
import https from 'node:https';
import { isIP } from 'node:net';
import { createHmac } from 'node:crypto';
import { openText } from '../../../platform/secrets.ts';
import { assertSafeTarget } from '../../../platform/net-guard.ts';
import { Status } from '../../../platform/http-status.ts';
import { MS_PER_SECOND, REDIRECT_MIN, WEBHOOK_TIMEOUT_MS } from './constants.ts';

/** The event as a body plus headers carrying its id and a timestamped HMAC under the hook's secret. */
export function signedEvent(hook, event) {
  const body = JSON.stringify(event),
    timestamp = String(Math.floor(Date.now() / MS_PER_SECOND));
  const headers = {
    'Content-Type': 'application/json',
    'Oya-Event-Id': String(event.id),
    'Oya-Signature': `t=${timestamp},v1=${sign(hook, timestamp, body)}`,
  };
  return { body, headers };
}

/** Hex HMAC-SHA256 of `timestamp.body` under the hook's secret. */
const sign = (hook, timestamp, body) =>
  createHmac('sha256', openText(`webhook:${hook.id}`, hook.secret))
    .update(`${timestamp}.${body}`)
    .digest('hex');

/** POSTs to a customer webhook over HTTPS, pinned to the address the SSRF check approved; true on a 2xx. */
export async function sendWebhook(url, body, headers): Promise<boolean> {
  const target = await assertSafeTarget(url, { protocols: ['https:'], label: 'webhook URL' });
  return post(url, body, headers, target.addresses[0]);
}

/** One POST; resolves whether the answer was a 2xx. */
function post(url, body, headers, address): Promise<boolean> {
  return new Promise((resolve, reject) => {
    const request = https.request(url, pinned(headers, address), (response) => {
      response.resume();
      resolve(response.statusCode >= Status.OK && response.statusCode < REDIRECT_MIN);
    });
    request.once('error', reject);
    request.end(body);
  });
}

/** Request options that resolve the host to `address` only. */
function pinned(headers, address): https.RequestOptions {
  return {
    method: 'POST',
    headers,
    signal: AbortSignal.timeout(WEBHOOK_TIMEOUT_MS),
    // Pin the address checked above while retaining the original TLS hostname.
    lookup: (hostname, options, callback) =>
      options.all ? callback(null, [{ address, family: isIP(address) }]) : callback(null, address, isIP(address)),
  };
}
