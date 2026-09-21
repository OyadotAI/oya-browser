/**
 * Unit tests for storing a key's Slack install, both from the OAuth callback and
 * from a pasted bot token, against a stubbed Slack API.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { completeOAuth, saveByo, mask } = await import('../../../../src/modules/slack/install.ts');
const { issueState } = await import('../../../../src/modules/slack/oauth-state.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');

const KEY = 'slack-install-key';

/** Answers Slack API calls by method name; records every call. */
function stubSlack(answers: Record<string, any>) {
  const calls: { method: string; body: any }[] = [];
  mock.method(globalThis, 'fetch', async (url, init) => {
    const method = String(url).split('/').pop();
    calls.push({ method, body: typeof init.body === 'string' ? JSON.parse(init.body) : init.body });
    return new Response(JSON.stringify(answers[method] ?? { ok: false, error: 'unexpected' }));
  });
  return calls;
}

/** The project's Slack sink row. */
const sink = () => control().store.get('webhook', `slack:${projectId(KEY)}`);

/** An OAuth callback request whose cookie matches its state. */
const callback = (query, cookieState = query.state) => ({
  query,
  headers: { cookie: `oya_slack_state=${cookieState}` },
});

describe('Slack install', () => {
  beforeEach(async () => {
    await keyConfig.clearSlack(KEY);
    process.env.SLACK_CLIENT_ID = 'cid';
    process.env.SLACK_CLIENT_SECRET = 'csecret';
  });
  afterEach(() => {
    delete process.env.SLACK_CLIENT_ID;
    delete process.env.SLACK_CLIENT_SECRET;
    mock.restoreAll();
  });

  it('masks a bot token to its last four characters, and nothing to nothing', () => {
    assert.equal(mask('xoxb-123456'), '••••3456');
    assert.equal(mask(''), '');
    assert.equal(mask(undefined), '');
  });

  describe('completeOAuth', () => {
    it('sends Slack’s own error back to the dashboard', async () => {
      assert.equal(
        await completeOAuth({ query: { error: 'access denied' }, headers: {} }),
        '/dashboard?slack=access%20denied',
      );
    });

    it('refuses a callback whose state does not match the browser’s cookie', async () => {
      const state = await issueState(KEY);
      assert.equal(await completeOAuth(callback({ code: 'c', state }, 'forged')), '/dashboard?slack=state_mismatch');
    });

    it('refuses a state that was already used', async () => {
      await control().project(KEY);
      const state = await issueState(KEY);
      stubSlack({ 'oauth.v2.access': { ok: true, access_token: 'xoxb-1', team: { id: 'T1', name: 'Team' } } });
      await completeOAuth(callback({ code: 'c', state }));
      assert.equal(await completeOAuth(callback({ code: 'c', state })), '/dashboard?slack=expired');
    });

    it('refuses a callback with no code', async () => {
      await control().project(KEY);
      const state = await issueState(KEY);
      assert.equal(await completeOAuth(callback({ state })), '/dashboard?slack=expired');
    });

    it('reports a failed code exchange by Slack’s error', async () => {
      await control().project(KEY);
      const state = await issueState(KEY);
      stubSlack({ 'oauth.v2.access': { ok: false, error: 'invalid_code' } });
      assert.equal(await completeOAuth(callback({ code: 'c', state })), '/dashboard?slack=invalid_code');
    });

    it('stores a fresh install and sends the dashboard to pick a channel', async () => {
      await control().project(KEY);
      const state = await issueState(KEY);
      stubSlack({ 'oauth.v2.access': { ok: true, access_token: 'xoxb-1', team: { id: 'T1', name: 'Team' } } });
      assert.equal(await completeOAuth(callback({ code: 'c', state })), '/dashboard?slack=pick-channel');
      const install = keyConfig.getSlack(KEY);
      assert.equal(install.botToken, 'xoxb-1');
      assert.equal(install.teamName, 'Team');
      assert.equal(install.byo, false);
      assert.equal((await sink()).enabled, false, 'no channel yet, so alerts stay off');
    });

    it('keeps the channel already picked when the app is reinstalled', async () => {
      await control().project(KEY);
      await keyConfig.saveSlack(KEY, { botToken: 'old', channelId: 'C1', channelName: 'alerts' });
      const state = await issueState(KEY);
      stubSlack({ 'oauth.v2.access': { ok: true, access_token: 'xoxb-2', team: { id: 'T1' } } });
      assert.equal(await completeOAuth(callback({ code: 'c', state })), '/dashboard?slack=connected');
      assert.equal(keyConfig.getSlack(KEY).channelName, 'alerts');
      assert.equal((await sink()).enabled, true);
    });
  });

  describe('saveByo', () => {
    it('refuses a missing bot token', async () => {
      await assert.rejects(saveByo({ body: {} }, KEY), { status: 400, message: 'A bot token is required' });
    });

    it('refuses a token Slack rejects, naming Slack’s error', async () => {
      stubSlack({ 'auth.test': { ok: false, error: 'invalid_auth' } });
      await assert.rejects(saveByo({ body: { botToken: 'xoxb-bad' } }, KEY), {
        status: 400,
        message: 'Slack rejected that token: invalid_auth',
      });
    });

    it('stores a verified token with its channel, joins the channel, and answers masked', async () => {
      const calls = stubSlack({
        'auth.test': { ok: true, team_id: 'T1', team: 'Team' },
        'conversations.info': { ok: true, channel: { name: 'alerts' } },
        'conversations.join': { ok: true },
      });
      const view = await saveByo({ body: { botToken: ' xoxb-12345 ', channelId: 'C1' } }, KEY);
      assert.deepEqual(view, {
        connected: true,
        teamName: 'Team',
        channelId: 'C1',
        channelName: 'alerts',
        byo: true,
        botToken: '••••2345',
      });
      assert.equal(keyConfig.getSlack(KEY).botToken, 'xoxb-12345');
      assert.deepEqual(calls.at(-1), { method: 'conversations.join', body: { channel: 'C1' } });
      assert.equal((await sink()).channel, 'C1');
    });

    it('keeps the stored token when the form sends the masked placeholder back', async () => {
      await keyConfig.saveSlack(KEY, { botToken: 'xoxb-kept', byo: false, channelId: 'C1', channelName: 'alerts' });
      const calls = stubSlack({ 'auth.test': { ok: true, team_id: 'T1' }, 'conversations.join': { ok: true } });
      const view = await saveByo({ body: { botToken: '••••kept' } }, KEY);
      assert.equal(keyConfig.getSlack(KEY).botToken, 'xoxb-kept');
      assert.equal(view.byo, false, 'an OAuth install stays one');
      assert.equal(view.channelName, 'alerts', 'an unchanged channel keeps its name without asking Slack');
      assert.ok(!calls.some((c) => c.method === 'conversations.info'));
    });

    it('clears the channel when the form sends an empty one, turning alerts off', async () => {
      await keyConfig.saveSlack(KEY, { botToken: 'xoxb-kept', channelId: 'C1' });
      const calls = stubSlack({ 'auth.test': { ok: true, team_id: 'T1' } });
      const view = await saveByo({ body: { channelId: '' } }, KEY);
      assert.equal(view.channelId, null);
      assert.equal((await sink()).enabled, false);
      assert.ok(!calls.some((c) => c.method === 'conversations.join'));
    });

    it('stores no channel name when Slack will not say it', async () => {
      stubSlack({
        'auth.test': { ok: true, team_id: 'T1' },
        'conversations.info': { ok: false },
        'conversations.join': { ok: true },
      });
      const view = await saveByo({ body: { botToken: 'xoxb-1', channelId: 'C9' } }, KEY);
      assert.equal(view.channelName, null);
    });
  });
});
