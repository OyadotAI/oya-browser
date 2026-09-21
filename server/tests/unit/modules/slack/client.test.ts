/**
 * Unit tests for the Slack client: where the console is reachable from, the
 * OAuth redirect, Web API calls and the code exchange, all against a stubbed fetch.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  consoleUrl,
  redirectUri,
  oauthConfigured,
  call,
  isDeadInstall,
  exchangeCode,
} from '../../../../src/modules/slack/client.ts';
import { DEFAULT_PORT } from '../../../../src/modules/slack/constants.ts';

const ENV = ['OYA_CONSOLE_URL', 'OYA_PUBLIC_WS_URL', 'PORT', 'SLACK_CLIENT_ID', 'SLACK_CLIENT_SECRET'];
let saved: Record<string, string | undefined>;

/** Stubs fetch with one JSON answer; returns the calls it received. */
function stubFetch(answer: object) {
  return mock.method(globalThis, 'fetch', async () => new Response(JSON.stringify(answer)));
}

describe('Slack client', () => {
  beforeEach(() => {
    saved = Object.fromEntries(ENV.map((k) => [k, process.env[k]]));
    for (const k of ENV) delete process.env[k];
  });
  afterEach(() => {
    for (const [k, v] of Object.entries(saved)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
    mock.restoreAll();
  });

  describe('consoleUrl', () => {
    it('prefers OYA_CONSOLE_URL, without trailing slashes', () => {
      process.env.OYA_CONSOLE_URL = 'https://console.example.com//';
      assert.equal(consoleUrl(), 'https://console.example.com');
    });

    it('derives https from a wss public WebSocket URL, dropping path and query', () => {
      process.env.OYA_PUBLIC_WS_URL = 'wss://oya.example.com/ws?token=x';
      assert.equal(consoleUrl(), 'https://oya.example.com');
    });

    it('derives http from a plain ws URL', () => {
      process.env.OYA_PUBLIC_WS_URL = 'ws://oya.internal:8080/ws';
      assert.equal(consoleUrl(), 'http://oya.internal:8080');
    });

    it('falls back to localhost on PORT, or the default port', () => {
      assert.equal(consoleUrl(), `http://localhost:${DEFAULT_PORT}`);
      process.env.PORT = '4000';
      assert.equal(consoleUrl(), 'http://localhost:4000');
    });
  });

  it('points the OAuth redirect at the console’s callback route', () => {
    process.env.OYA_CONSOLE_URL = 'https://console.example.com';
    assert.equal(redirectUri(), 'https://console.example.com/api/slack/callback');
  });

  it('counts OAuth as configured only with both the client id and secret', () => {
    process.env.SLACK_CLIENT_ID = 'id';
    assert.equal(oauthConfigured(), false);
    process.env.SLACK_CLIENT_SECRET = 'secret';
    assert.equal(oauthConfigured(), true);
  });

  it('calls a Web API method with the bot token as a bearer and returns the parsed body', async () => {
    const fetch = stubFetch({ ok: true, channel: { id: 'C1' } });
    const result = await call('xoxb-1', 'conversations.info', { channel: 'C1' });
    assert.deepEqual(result, { ok: true, channel: { id: 'C1' } });
    const [url, init] = fetch.mock.calls[0].arguments;
    assert.equal(url, 'https://slack.com/api/conversations.info');
    assert.equal(init.headers.Authorization, 'Bearer xoxb-1');
    assert.deepEqual(JSON.parse(init.body), { channel: 'C1' });
  });

  it('sends an empty object when a call has no body', async () => {
    const fetch = stubFetch({ ok: true });
    await call('xoxb-1', 'auth.test');
    assert.equal(fetch.mock.calls[0].arguments[1].body, '{}');
  });

  it('treats a revoked token or a lost channel as a dead install, and other errors as retryable', () => {
    for (const error of ['invalid_auth', 'token_revoked', 'channel_not_found', 'not_in_channel', 'is_archived'])
      assert.equal(isDeadInstall(error), true, error);
    assert.equal(isDeadInstall('ratelimited'), false);
  });

  it('exchanges an OAuth code with the app’s credentials and the redirect URI', async () => {
    process.env.SLACK_CLIENT_ID = 'cid';
    process.env.SLACK_CLIENT_SECRET = 'csecret';
    process.env.OYA_CONSOLE_URL = 'https://console.example.com';
    const fetch = stubFetch({ ok: true, access_token: 'xoxb-new' });
    assert.deepEqual(await exchangeCode('code-1'), { ok: true, access_token: 'xoxb-new' });
    const [url, init] = fetch.mock.calls[0].arguments;
    assert.equal(url, 'https://slack.com/api/oauth.v2.access');
    const form = Object.fromEntries(init.body);
    assert.deepEqual(form, {
      code: 'code-1',
      client_id: 'cid',
      client_secret: 'csecret',
      redirect_uri: 'https://console.example.com/api/slack/callback',
    });
  });
});
