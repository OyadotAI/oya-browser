/**
 * Unit tests for signing in with stored credentials through a scripted page:
 * the stops that must never retry (locked, rejected, no credentials), the
 * attempt budget per browser and site, and confirming the form went away.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { LOGIN_CONFIRM_MS, LOGIN_POLL_MS, MAX_LOGIN_ATTEMPTS } from '../../../../src/modules/challenges/constants.ts';
import { ownDataDir } from '../../support/data-dir.ts';
import { advance } from '../../support/http.ts';
import { scriptedPage } from '../../support/challenges.ts';

ownDataDir('oya-login-');
// The verdict loop sleeps through timers/promises, whose binding is fixed when
// first imported: importing with the clock mocked lets mock.timers drive it.
mock.timers.enable({ apis: ['setTimeout'] });
const login = await import('../../../../src/modules/challenges/login.ts');
const credentials = await import('../../../../src/modules/personas/credentials.ts');
mock.timers.reset();

const { DETECT_JS, fillCredentialsJS, requestCodeJS } = login;
/** Where a person would finish by hand. */
const LIVE = 'https://oya.example/live/b1';
/** A sign-in form, as DETECT_JS reports it. */
const FORM = {
  present: true,
  stage: 'credentials',
  hasUsername: true,
  hasSubmit: true,
  rejected: false,
  locked: false,
};
/** The page once the form is gone. */
const GONE = { present: false };
/** The fill script for the stored login. */
const FILL = fillCredentialsJS('ada', 's3cret');
/** What every sign-in here is called with. */
const OPTIONS = { domain: 'portal.example', browserId: 'b1', liveViewUrl: LIVE };

describe('login.complete', () => {
  beforeEach(() => {
    credentials.reset();
    credentials.set('p1', 'portal.example', { username: 'ada', password: 's3cret' });
    login.forget('b1');
    mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 1000 });
  });
  afterEach(() => mock.timers.reset());

  it('reports no login page, passing on a lockout notice', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [{ present: false, locked: true }]]]);
    assert.deepEqual(await login.complete(evaluate, 'p1', OPTIONS), {
      present: false,
      completed: false,
      method: 'none',
      locked: true,
    });
  });

  it('stops at a locked account without typing anything', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [{ ...FORM, locked: true }]]]);
    const result = await login.complete(evaluate, 'p1', OPTIONS);
    assert.deepEqual([result.method, result.locked, result.liveViewUrl], ['handoff', true, LIVE]);
    assert.equal(evaluate.mock.callCount(), 1);
  });

  it('presses the request-a-code control on a code step', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [{ present: true, stage: 'request_code' }]],
      [requestCodeJS, [{ requested: true }]],
    ]);
    const result = await login.complete(evaluate, 'p1', OPTIONS);
    assert.deepEqual(result, {
      present: true,
      completed: true,
      method: 'request_code',
      requestedAt: 1000,
      liveViewUrl: LIVE,
    });
  });

  it('reports a code step it could not press', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [{ present: true, stage: 'request_code' }]],
      [requestCodeJS, [{ requested: false, reason: 'no request control' }]],
    ]);
    assert.equal((await login.complete(evaluate, 'p1', OPTIONS)).error, 'no request control');
  });

  it('hands the sign-in to a person when nothing is stored for the site', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [FORM]]]);
    const result = await login.complete(evaluate, 'p1', { ...OPTIONS, domain: 'elsewhere.example' });
    assert.equal(result.method, 'handoff');
    assert.match(result.error, /No credentials are stored for elsewhere\.example/);
    assert.match((await login.complete(evaluate, 'p1', { browserId: 'b1' })).error, /for this site/);
  });

  it('never retries a password the site has just rejected', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [{ ...FORM, rejected: true }]]]);
    const result = await login.complete(evaluate, 'p1', OPTIONS);
    assert.equal(result.rejected, true);
    assert.match(result.error, /rejected the stored credentials for ada at portal\.example/);
    assert.equal(login.attemptsFor('b1', 'portal.example'), 0);
  });

  it('signs in with the stored credentials, including a subdomain’s parent', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM, GONE]],
      [FILL, [{ filled: true, submitted: true }]],
    ]);
    const result = await login.complete(evaluate, 'p1', { ...OPTIONS, domain: 'app.portal.example' });
    assert.deepEqual(result, {
      present: true,
      completed: true,
      method: 'credentials',
      submitted: true,
      submittedAt: 1000,
      username: 'ada',
      domain: 'portal.example',
      rejected: false,
      locked: false,
    });
    assert.equal(login.attemptsFor('b1', 'portal.example'), 0, 'a success clears the counter');
  });

  it('dates a sign-in from before its form was sent, so a code emailed during the post counts', async () => {
    const evaluate = mock.fn(async (script: string) => {
      if (script === FILL) {
        mock.timers.tick(3000); // the site handles the post, and emails the code, meanwhile
        return { filled: true, submitted: true };
      }
      return evaluate.mock.callCount() > 1 ? GONE : FORM;
    });
    const result = await login.complete(evaluate, 'p1', OPTIONS);
    assert.equal(result.submittedAt, 1000);
  });

  it('dates a code request from before its control was pressed', async () => {
    const evaluate = mock.fn(async (script: string) => {
      if (script === requestCodeJS) {
        mock.timers.tick(3000);
        return { requested: true };
      }
      return { present: true, stage: 'request_code' };
    });
    const result = await login.complete(evaluate, 'p1', OPTIONS);
    assert.equal(result.requestedAt, 1000);
  });

  it('counts reaching the code step as signed in', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM, { present: true, stage: 'request_code' }]],
      [FILL, [{ filled: true, submitted: false }]],
    ]);
    assert.equal((await login.complete(evaluate, 'p1', OPTIONS)).completed, true);
  });

  it('waits for the site’s verdict and reports a rejection after submitting', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM, FORM, { ...FORM, rejected: true }]],
      [FILL, [{ filled: true, submitted: true }]],
    ]);
    const pending = login.complete(evaluate, 'p1', OPTIONS);
    await advance(LOGIN_POLL_MS);
    const result = await pending;
    assert.equal(result.completed, false);
    assert.match(result.error, /rejected the stored credentials/);
    assert.equal(login.attemptsFor('b1', 'portal.example'), 1);
  });

  it('reports a lockout that appears after submitting', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM, { ...FORM, locked: true }]],
      [FILL, [{ filled: true, submitted: true }]],
    ]);
    assert.equal((await login.complete(evaluate, 'p1', OPTIONS)).error, 'The site says this account is now locked.');
  });

  it('waits out a page it cannot read, then reports what it finds', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM, new Error('navigating'), GONE]],
      [FILL, [{ filled: true, submitted: true }]],
    ]);
    const pending = login.complete(evaluate, 'p1', OPTIONS);
    await advance(LOGIN_POLL_MS);
    assert.equal((await pending).completed, true);
  });

  it('never calls a sign-in it could not see signed in', async () => {
    // The page stays unreadable for the whole window. Reporting that as success
    // is what cleared the attempt counter and let the next run spend another of
    // the portal's tries on a password the site had already refused.
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM, new Error('navigating')]],
      [FILL, [{ filled: true, submitted: true }]],
    ]);
    const pending = login.complete(evaluate, 'p1', OPTIONS);
    await advance(LOGIN_POLL_MS, LOGIN_CONFIRM_MS / LOGIN_POLL_MS);
    const result = await pending;
    assert.equal(result.completed, false);
    assert.match(result.error, /never finished loading/);
    assert.equal(login.attemptsFor('b1', 'portal.example'), 1);
  });

  it('reports a form still showing once the confirm window closes', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM]],
      [FILL, [{ filled: true, submitted: true }]],
    ]);
    const pending = login.complete(evaluate, 'p1', OPTIONS);
    await advance(LOGIN_POLL_MS, LOGIN_CONFIRM_MS / LOGIN_POLL_MS);
    const result = await pending;
    assert.equal(result.completed, false);
    assert.match(result.error, /still showing/);
  });

  it('reports a form it could not fill', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM]],
      [FILL, [{ filled: false, reason: 'password field not found' }]],
    ]);
    assert.equal((await login.complete(evaluate, 'p1', OPTIONS)).error, 'password field not found');
  });

  it('stops before the account can lock once the attempts are spent', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM]],
      [FILL, [{ filled: true, submitted: false }]],
    ]);
    for (let i = 0; i < MAX_LOGIN_ATTEMPTS; i++) await login.complete(evaluate, 'p1', OPTIONS);
    const result = await login.complete(evaluate, 'p1', OPTIONS);
    assert.equal(result.exhausted, true);
    assert.match(result.error, new RegExp(`after ${MAX_LOGIN_ATTEMPTS} attempts`));
    assert.equal(login.attemptsFor('b2', 'portal.example'), 0, 'the budget is per browser');
  });

  it('forgets one site’s attempts, or all of a browser’s', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [FORM]],
      [FILL, [{ filled: true, submitted: false }]],
    ]);
    await login.complete(evaluate, 'p1', OPTIONS);
    login.forget('b1', 'other.example');
    assert.equal(login.attemptsFor('b1', 'portal.example'), 1);
    login.forget('b1', 'portal.example');
    assert.equal(login.attemptsFor('b1', 'portal.example'), 0);
    await login.complete(evaluate, 'p1', OPTIONS);
    login.forget('b1');
    assert.equal(login.attemptsFor('b1', 'portal.example'), 0);
  });
});

describe('fillCredentialsJS', () => {
  it('embeds the username and password as JSON, never raw', () => {
    const script = fillCredentialsJS('a"b', 'p\\w');
    assert.ok(script.includes(JSON.stringify({ username: 'a"b', password: 'p\\w' })));
  });
});
