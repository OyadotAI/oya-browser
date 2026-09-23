/**
 * Unit tests for the Turnstile check: off without a secret, closed without a
 * token, Cloudflare's verdict otherwise, and open when Cloudflare is down.
 */
import { afterEach, beforeEach, describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { verifyCaptcha } from '../../../../src/modules/auth/captcha.ts';

/** Makes fetch answer Cloudflare's JSON `verdict`, recording the form it was sent. */
function cloudflareSays(verdict: any) {
  const sent: URLSearchParams[] = [];
  mock.method(globalThis, 'fetch', async (_url: string, init: any) => {
    sent.push(init.body);
    return new Response(JSON.stringify(verdict));
  });
  return sent;
}

describe('verifyCaptcha', () => {
  beforeEach(() => (process.env.TURNSTILE_SECRET_KEY = 'test-secret'));
  afterEach(() => {
    delete process.env.TURNSTILE_SECRET_KEY;
    mock.restoreAll();
  });

  it('lets everything through when no secret is configured', async () => {
    delete process.env.TURNSTILE_SECRET_KEY;
    assert.equal(await verifyCaptcha(undefined), true);
  });

  it('refuses a request without a token', async () => {
    assert.equal(await verifyCaptcha(''), false);
  });

  it('passes a token Cloudflare accepts, sending the secret with it', async () => {
    const sent = cloudflareSays({ success: true });
    assert.equal(await verifyCaptcha('tok'), true);
    assert.deepEqual([sent[0].get('secret'), sent[0].get('response')], ['test-secret', 'tok']);
  });

  it('refuses a token Cloudflare rejects', async () => {
    cloudflareSays({ success: false });
    assert.equal(await verifyCaptcha('tok'), false);
  });

  it('lets the request through when Cloudflare cannot be reached', async () => {
    mock.method(globalThis, 'fetch', async () => {
      throw new Error('offline');
    });
    assert.equal(await verifyCaptcha('tok'), true);
  });
});
