/**
 * Unit tests for ops Slack messages: a channel without a webhook sends nothing,
 * a configured one gets a single fire-and-forget POST with the text, a network
 * failure is silent, and a webhook Slack refuses is logged once.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { post, enabled } from '../../../src/platform/ops-slack.ts';
import { MESSAGE_MAX_CHARS } from '../../../src/platform/constants.ts';
import { stubFetch, json } from '../support/http.ts';

describe('ops slack', () => {
  beforeEach(() => {
    delete process.env.SLACK_OPS_WEBHOOK_SIGNUPS;
    delete process.env.SLACK_OPS_WEBHOOK_EVENTS;
  });
  afterEach(() => {
    mock.restoreAll();
    delete process.env.SLACK_OPS_WEBHOOK_SIGNUPS;
    delete process.env.SLACK_OPS_WEBHOOK_EVENTS;
  });

  it('sends nothing to a channel with no webhook', async () => {
    const calls = stubFetch(() => json({}));
    post('signups', 'x');
    post('events', 'y');
    await new Promise((r) => setImmediate(r));
    assert.equal(calls.length, 0);
    assert.equal(enabled('signups'), false);
  });

  it('posts the text to the channel’s own webhook, and only that channel’s', async () => {
    process.env.SLACK_OPS_WEBHOOK_SIGNUPS = 'https://hooks.example.test/signups';
    const calls = stubFetch(() => json({}));
    post('signups', '🎉 New signup: ana@example.com');
    post('events', 'should not go anywhere');
    await new Promise((r) => setImmediate(r));
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, 'https://hooks.example.test/signups');
    assert.deepEqual(JSON.parse(calls[0].init.body), { text: '🎉 New signup: ana@example.com' });
  });

  it('escapes Slack markup and caps the length, so a name someone chose cannot ping the channel or plant a link', async () => {
    process.env.SLACK_OPS_WEBHOOK_EVENTS = 'https://hooks.example.test/events';
    const calls = stubFetch(() => json({}));
    post('events', '💾 Playbook saved: <!channel> <https://evil.example|Reset your password> & more');
    post('events', 'x'.repeat(MESSAGE_MAX_CHARS + 50));
    await new Promise((r) => setImmediate(r));
    const [markup, long] = calls.map((c) => JSON.parse(c.init.body).text);
    assert.equal(
      markup,
      '💾 Playbook saved: &lt;!channel&gt; &lt;https://evil.example|Reset your password&gt; &amp; more',
    );
    assert.equal(long.length, MESSAGE_MAX_CHARS);
  });

  it('stays silent when the webhook fails or never answers', async () => {
    process.env.SLACK_OPS_WEBHOOK_EVENTS = 'https://hooks.example.test/events';
    const logged = mock.method(console, 'error', () => {});
    stubFetch(() => Promise.reject(new Error('network down')));
    post('events', 'x');
    await new Promise((r) => setImmediate(r));
    assert.equal(logged.mock.callCount(), 0);
  });

  it('logs once when Slack refuses the webhook, so a revoked URL does not drop messages unnoticed', async () => {
    process.env.SLACK_OPS_WEBHOOK_SIGNUPS = 'https://hooks.example.test/signups';
    const logged = mock.method(console, 'error', () => {});
    stubFetch(() => new Response('no_service', { status: 404 }));
    post('signups', 'x');
    post('signups', 'y');
    await new Promise((r) => setTimeout(r, 0));
    assert.equal(logged.mock.callCount(), 1);
    assert.match(String(logged.mock.calls[0].arguments[0]), /SLACK_OPS_WEBHOOK_SIGNUPS refused \(404 no_service\)/);
  });
});
