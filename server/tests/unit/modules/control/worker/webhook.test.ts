/**
 * Unit tests for customer webhooks: the signed body and headers a receiver can
 * verify, and the SSRF check that runs before any request is made.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { sealText } from '../../../../../src/platform/secrets.ts';
import { sendWebhook, signedEvent } from '../../../../../src/modules/control/worker/webhook.ts';

afterEach(() => mock.timers.reset());

const hook = { id: 'hook:p', secret: sealText('webhook:hook:p', 'shh') };

describe('signedEvent', () => {
  it('sends the event as JSON with its id and a timestamped HMAC the receiver can verify', () => {
    mock.timers.enable({ apis: ['Date'], now: 1_700_000_000_500 });
    const event = { id: 42, type: 'session.ready' };
    const { body, headers } = signedEvent(hook, event);
    assert.deepEqual(JSON.parse(body), event);
    assert.equal(headers['Content-Type'], 'application/json');
    assert.equal(headers['Oya-Event-Id'], '42');
    const expected = createHmac('sha256', 'shh').update(`1700000000.${body}`).digest('hex');
    assert.equal(headers['Oya-Signature'], `t=1700000000,v1=${expected}`);
  });

  it('cannot sign with a secret sealed for another hook', () => {
    assert.throws(() => signedEvent({ ...hook, id: 'hook:other' }, { id: 1 }));
  });
});

describe('sendWebhook', () => {
  it('refuses anything but HTTPS before sending', async () => {
    await assert.rejects(sendWebhook('http://example.com/hook', '{}', {}), { status: 400 });
  });

  it('refuses a private address before sending', async () => {
    await assert.rejects(sendWebhook('https://127.0.0.1/hook', '{}', {}), { status: 400 });
    await assert.rejects(sendWebhook('https://169.254.169.254/latest', '{}', {}), { status: 400 });
  });
});
