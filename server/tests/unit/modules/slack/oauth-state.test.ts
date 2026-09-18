/**
 * Unit tests for the Slack OAuth state: single use, five minutes, kept in the
 * control store, and bound to the browser that started the install by a cookie.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { issueState, redeemState, matchesStateCookie, setStateCookie, clearStateCookie } =
  await import('../../../../src/modules/slack/oauth-state.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { STATE_TTL_MS } = await import('../../../../src/modules/slack/constants.ts');

const KEY = 'slack-state-key';

/** A request carrying cookies. */
const withCookie = (cookie?: string, extra = {}) => ({ headers: cookie ? { cookie } : {}, ...extra });

/** A response that records the cookie calls. */
function cookieJar() {
  const calls = { set: [] as any[], cleared: [] as any[] };
  const res = {
    cookie: (...args) => calls.set.push(args),
    clearCookie: (...args) => calls.cleared.push(args),
  };
  return { res, calls };
}

describe('Slack OAuth state', () => {
  afterEach(() => mock.timers.reset());

  it('redeems an issued state once for the key that asked for it', async () => {
    await control().project(KEY);
    const state = await issueState(KEY);
    assert.equal(await redeemState(state), KEY);
    assert.equal(await redeemState(state), null, 'a second redemption finds nothing');
  });

  it('stores the project id, never the key itself', async () => {
    const state = await issueState(KEY);
    const row = await control().store.get('slack_state', state);
    assert.equal(row.project, projectId(KEY));
    assert.ok(!JSON.stringify(row).includes(KEY));
    await redeemState(state);
  });

  it('refuses an expired state and consumes it anyway', async () => {
    await control().project(KEY);
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    const state = await issueState(KEY);
    mock.timers.tick(STATE_TTL_MS + 1);
    assert.equal(await redeemState(state), null);
    assert.equal(await control().store.get('slack_state', state), null);
  });

  it('refuses an empty or unknown state', async () => {
    assert.equal(await redeemState(''), null);
    assert.equal(await redeemState('never-issued'), null);
  });

  it('matches the state against the cookie of the browser that started the install', () => {
    assert.equal(matchesStateCookie(withCookie('a=1; oya_slack_state=abc; b=2'), 'abc'), true);
    assert.equal(matchesStateCookie(withCookie('oya_slack_state=abc'), 'abd'), false);
  });

  it('refuses when the cookie or the state is missing, rather than matching two absences', () => {
    assert.equal(matchesStateCookie(withCookie(), 'abc'), false);
    assert.equal(matchesStateCookie(withCookie('oya_slack_state='), ''), false);
  });

  it('sets an HTTP-only, lax, path-scoped cookie that lives as long as the state', () => {
    const { res, calls } = cookieJar();
    setStateCookie(withCookie(), res, 'abc');
    const [name, value, options] = calls.set[0];
    assert.equal(name, 'oya_slack_state');
    assert.equal(value, 'abc');
    assert.deepEqual(options, {
      httpOnly: true,
      secure: false,
      sameSite: 'lax',
      maxAge: STATE_TTL_MS,
      path: '/api/slack',
    });
  });

  it('marks the cookie secure behind an https proxy', () => {
    const { res, calls } = cookieJar();
    setStateCookie(withCookie(undefined, { headers: { 'x-forwarded-proto': 'https' } }), res, 'abc');
    assert.equal(calls.set[0][2].secure, true);
  });

  it('clears the cookie on its own path', () => {
    const { res, calls } = cookieJar();
    clearStateCookie(res);
    assert.deepEqual(calls.cleared[0], ['oya_slack_state', { path: '/api/slack' }]);
  });
});
