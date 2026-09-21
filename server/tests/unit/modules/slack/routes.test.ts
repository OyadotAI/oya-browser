/**
 * Unit tests for /api/slack: the install's status, bring-your-own-bot, the
 * channel list, disconnecting, and the OAuth install and callback.
 */
import { describe, it, before, after, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { slackRouter } = await import('../../../../src/modules/slack/routes.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');

const KEY = 'slack-routes-key';
let forget: () => void;

/** Answers Slack API calls by method name. */
function stubSlack(answers: Record<string, any>) {
  mock.method(globalThis, 'fetch', async (url) => {
    const method = String(url).split('/').pop();
    return new Response(JSON.stringify(answers[method] ?? { ok: false, error: 'unexpected' }));
  });
}

/** Calls the router as the test key. */
const as = (method: string, url: string, body?: any) => callRoute(slackRouter, { method, url, key: KEY, body });

describe('/api/slack', () => {
  before(() => (forget = allowKey(KEY)));
  after(() => forget());
  beforeEach(() => keyConfig.clearSlack(KEY));
  afterEach(() => {
    delete process.env.SLACK_CLIENT_ID;
    delete process.env.SLACK_CLIENT_SECRET;
    delete process.env.OYA_CONSOLE_URL;
    mock.restoreAll();
  });

  it('refuses a caller without an API key', async () => {
    const res = await callRoute(slackRouter, { url: '/' });
    assert.equal(res.status, 401);
  });

  it('reports a key with no install as disconnected, with OAuth unavailable', async () => {
    const res = await as('GET', '/');
    assert.deepEqual(res.body, {
      connected: false,
      oauthAvailable: false,
      redirectUri: null,
      teamName: null,
      channelId: null,
      channelName: null,
      byo: false,
      botToken: '',
    });
  });

  it('reports a connected install with its token masked and the redirect URI to register', async () => {
    process.env.SLACK_CLIENT_ID = 'cid';
    process.env.SLACK_CLIENT_SECRET = 'cs';
    process.env.OYA_CONSOLE_URL = 'https://console.example.com';
    await keyConfig.saveSlack(KEY, {
      botToken: 'xoxb-9876',
      teamName: 'Team',
      channelId: 'C1',
      channelName: 'a',
      byo: true,
    });
    const { body } = await as('GET', '/');
    assert.equal(body.connected, true);
    assert.equal(body.botToken, '••••9876');
    assert.equal(body.redirectUri, 'https://console.example.com/api/slack/callback');
  });

  it('saves a pasted bot token through PUT', async () => {
    stubSlack({ 'auth.test': { ok: true, team_id: 'T1', team: 'Team' } });
    const res = await as('PUT', '/', { botToken: 'xoxb-1111' });
    assert.equal(res.body.connected, true);
    assert.equal(keyConfig.getSlack(KEY).botToken, 'xoxb-1111');
  });

  it('answers a refused token with its 400', async () => {
    const res = await as('PUT', '/', {});
    assert.equal(res.status, 400);
  });

  it('forgets the install and turns the sink off on DELETE', async () => {
    await keyConfig.saveSlack(KEY, { botToken: 'xoxb-1' });
    const res = await as('DELETE', '/');
    assert.deepEqual(res.body, { connected: false });
    assert.equal(keyConfig.getSlack(KEY), null);
    const hook = await control().store.get('webhook', `slack:${projectId(KEY)}`);
    assert.equal(hook.enabled, false);
  });

  it('refuses to list channels before Slack is connected', async () => {
    const res = await as('GET', '/channels');
    assert.equal(res.status, 409);
  });

  it('lists the channels the bot can see', async () => {
    await keyConfig.saveSlack(KEY, { botToken: 'xoxb-1' });
    stubSlack({
      'conversations.list': {
        ok: true,
        channels: [
          { id: 'C1', name: 'a', is_private: true },
          { id: 'C2', name: 'b' },
        ],
      },
    });
    const res = await as('GET', '/channels');
    assert.deepEqual(res.body.channels, [
      { id: 'C1', name: 'a', private: true },
      { id: 'C2', name: 'b', private: false },
    ]);
  });

  it('answers a refused channel list with Slack’s error', async () => {
    await keyConfig.saveSlack(KEY, { botToken: 'xoxb-1' });
    stubSlack({ 'conversations.list': { ok: false, error: 'missing_scope' } });
    const res = await as('GET', '/channels');
    assert.equal(res.status, 400);
    assert.match(res.body.error, /missing_scope/);
  });

  it('refuses the OAuth install on a deployment without its own Slack app', async () => {
    const res = await as('GET', '/install');
    assert.equal(res.status, 501);
  });

  it('starts the OAuth install with a state bound to a cookie', async () => {
    process.env.SLACK_CLIENT_ID = 'cid';
    process.env.SLACK_CLIENT_SECRET = 'cs';
    const res = await as('GET', '/install?json=1');
    const url = new URL(res.body.url);
    assert.equal(url.origin + url.pathname, 'https://slack.com/oauth/v2/authorize');
    assert.equal(url.searchParams.get('client_id'), 'cid');
    assert.equal(res.cookies.oya_slack_state.value, url.searchParams.get('state'));
  });

  it('redirects to Slack when the install is not asked for as JSON', async () => {
    process.env.SLACK_CLIENT_ID = 'cid';
    process.env.SLACK_CLIENT_SECRET = 'cs';
    const res = await as('GET', '/install');
    assert.equal(res.status, 302);
    assert.match(res.redirect, /^https:\/\/slack\.com\/oauth\/v2\/authorize\?/);
  });

  it('takes the callback without an API key, clears the state cookie and redirects', async () => {
    const res = await callRoute(slackRouter, { url: '/callback?error=access_denied' });
    assert.equal(res.redirect, '/dashboard?slack=access_denied');
    assert.deepEqual(res.cleared, ['oya_slack_state']);
  });
});
