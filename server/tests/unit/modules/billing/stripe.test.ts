/**
 * Unit tests for billing's Stripe client: the form encoding Stripe reads, the
 * call pinned to one API version, and the webhook signature check.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { formEncode, stripeClient, verifySignature } from '../../../../src/modules/billing/stripe.ts';
import { STRIPE_VERSION } from '../../../../src/modules/billing/constants.ts';

/** The webhook secret the tests sign with. */
const SECRET = 'whsec_test';
/** A fixed time, in ms. */
const NOW = 1_800_000_000_000;

/** A Stripe-Signature header for `body` signed at `t` (seconds) with `secret`. */
const header = (body: string, t = NOW / 1000, secret = SECRET) =>
  `t=${t},v1=${createHmac('sha256', secret).update(`${t}.${body}`).digest('hex')}`;

describe('formEncode', () => {
  it('writes nested objects and arrays the way Stripe reads them, leaving out empty values', () => {
    const form = formEncode({ mode: 'subscription', line_items: [{ price: 'p1', quantity: 1 }], customer: undefined });
    assert.equal(
      form.toString(),
      'mode=subscription&line_items%5B0%5D%5Bprice%5D=p1&line_items%5B0%5D%5Bquantity%5D=1',
    );
  });
});

describe('stripeClient', () => {
  it('posts form-encoded with the key and the pinned API version, and answers the body', async () => {
    const calls: any[] = [];
    const fetchFn = async (url, init) => (calls.push({ url, init }), new Response(JSON.stringify({ url: 'u' })));
    const body = await stripeClient(() => 'sk_test', fetchFn as any).post('/checkout/sessions', { a: 1 });
    assert.deepEqual(body, { url: 'u' });
    assert.equal(calls[0].url, 'https://api.stripe.com/v1/checkout/sessions');
    assert.equal(calls[0].init.headers.authorization, 'Bearer sk_test');
    assert.equal(calls[0].init.headers['stripe-version'], STRIPE_VERSION);
  });

  it('turns a Stripe error into a 502 carrying Stripe’s message', async () => {
    const fetchFn = async () => new Response(JSON.stringify({ error: { message: 'No such price' } }), { status: 400 });
    await assert.rejects(stripeClient(() => 'k', fetchFn as any).post('/x'), {
      status: 502,
      message: 'Stripe: No such price',
    });
  });

  it('names the status when Stripe’s answer is not JSON', async () => {
    const fetchFn = async () => new Response('oops', { status: 500 });
    await assert.rejects(stripeClient(() => 'k', fetchFn as any).post('/x'), { message: 'Stripe: 500' });
  });
});

describe('verifySignature', () => {
  it('accepts a body Stripe signed', () => {
    assert.equal(verifySignature('{"a":1}', header('{"a":1}'), SECRET, NOW), true);
  });

  it('accepts the raw bytes as a Buffer', () => {
    assert.equal(verifySignature(Buffer.from('{"a":1}'), header('{"a":1}'), SECRET, NOW), true);
  });

  it('refuses a body changed after signing', () => {
    assert.equal(verifySignature('{"a":2}', header('{"a":1}'), SECRET, NOW), false);
  });

  it('refuses a signature made with another secret', () => {
    assert.equal(verifySignature('{}', header('{}', NOW / 1000, 'whsec_other'), SECRET, NOW), false);
  });

  it('refuses an old signature, which could be a replay', () => {
    assert.equal(verifySignature('{}', header('{}', NOW / 1000 - 301), SECRET, NOW), false);
  });

  it('refuses a missing header or secret', () => {
    assert.equal(verifySignature('{}', '', SECRET, NOW), false);
    assert.equal(verifySignature('{}', header('{}'), '', NOW), false);
  });
});
