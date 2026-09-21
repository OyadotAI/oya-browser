/**
 * Unit tests for the Slack Resume button: only a signed request is trusted, and
 * a press resumes the run it names and tells the channel who did.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { slackActionsRouter } = await import('../../../../src/modules/slack/actions.ts');
const runs = await import('../../../../src/modules/playbooks/runs.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');
const { callRoute } = await import('../../support/agent.ts');

const SECRET = 'signing-secret';
const KEY = 'slack-actions-key';

/** A signed POST /slack/actions with a form-encoded payload. */
function press(payload: any, { sign = true, raw }: { sign?: boolean; raw?: string } = {}) {
  const text = raw ?? JSON.stringify(payload);
  const rawBody = `payload=${encodeURIComponent(text)}`;
  const ts = String(Math.floor(Date.now() / 1000));
  const signature = `v0=${createHmac('sha256', sign ? SECRET : 'wrong')
    .update(`v0:${ts}:${rawBody}`)
    .digest('hex')}`;
  return callRoute(slackActionsRouter, {
    method: 'POST',
    url: '/',
    body: { payload: text },
    headers: { 'x-slack-request-timestamp': ts, 'x-slack-signature': signature },
    extra: { rawBody: Buffer.from(rawBody) },
  });
}

/** A run parked on a person; resolves with the person's answer. */
function parkedRun() {
  mock.method(control(), 'emit', async () => {});
  let answered;
  const run = runs.start(KEY, 'b-actions', ({ requestHuman }) => (answered = requestHuman({ reason: 'captcha' })));
  return { run, answer: () => answered };
}

/** A Resume press for a run. */
const resumePress = (runId, owner, extra = {}) => ({
  actions: [{ action_id: 'resume_run', value: JSON.stringify({ runId, owner }) }],
  ...extra,
});

describe('POST /slack/actions', () => {
  beforeEach(() => (process.env.SLACK_SIGNING_SECRET = SECRET));
  afterEach(() => {
    delete process.env.SLACK_SIGNING_SECRET;
    mock.restoreAll();
  });

  it('refuses a request without a valid signature', async () => {
    const res = await press({ actions: [] }, { sign: false });
    assert.equal(res.status, 401);
  });

  it('refuses a signed payload that is not JSON', async () => {
    const res = await press(null, { raw: '{not json' });
    assert.equal(res.status, 400);
  });

  it('acknowledges any other action without doing anything', async () => {
    const res = await press({ actions: [{ action_id: 'open_live' }] });
    assert.equal(res.status, 200);
  });

  it('resumes the waiting run and tells the channel who resumed it', async () => {
    const { run, answer } = parkedRun();
    const posts = [];
    mock.method(
      globalThis,
      'fetch',
      async (url, init) => (posts.push([url, JSON.parse(init.body)]), new Response('ok')),
    );
    const res = await press(
      resumePress(run.id, fingerprint(KEY), { user: { id: 'U1' }, response_url: 'https://hooks.slack.test/r' }),
    );
    assert.equal(res.status, 200);
    assert.equal(await answer(), 'done');
    assert.deepEqual(posts[0], [
      'https://hooks.slack.test/r',
      { replace_original: false, text: '✅ Resumed by <@U1>.' },
    ]);
  });

  it('says so when the run is no longer waiting', async () => {
    const posts = [];
    mock.method(globalThis, 'fetch', async (url, init) => (posts.push(JSON.parse(init.body)), new Response('ok')));
    await press(resumePress('run_gone', 'owner', { response_url: 'https://hooks.slack.test/r' }));
    assert.equal(posts[0].text, '⚠️ That run is no longer waiting for anyone.');
  });

  it('will not resume another owner’s run with a forged owner', async () => {
    const { run } = parkedRun();
    await press(resumePress(run.id, 'someone-else'));
    assert.equal(runs.get(fingerprint(KEY), run.id).status, 'needs_attention');
    runs.respond(fingerprint(KEY), run.id);
  });

  it('ignores a button whose value is not JSON, and posts nowhere without a response URL', async () => {
    const fetch = mock.method(globalThis, 'fetch', async () => new Response('ok'));
    const res = await press({ actions: [{ action_id: 'resume_run', value: '{bad' }], user: { id: 'U1' } });
    assert.equal(res.status, 200);
    assert.equal(fetch.mock.callCount(), 0);
  });
});
