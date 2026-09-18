/**
 * Unit tests for completing an MFA challenge through a scripted page: the
 * handoff when nothing can answer, typing a TOTP code, confirming the prompt
 * went away, and the failures reported with the live view.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { MFA_CONFIRM_MS, MFA_POLL_MS } from '../../../../src/modules/challenges/constants.ts';
import { ownDataDir } from '../../support/data-dir.ts';
import { advance, stubFetch, text } from '../../support/http.ts';
import { scriptedPage, scriptsRun } from '../../support/challenges.ts';

ownDataDir('oya-mfa-');
// The confirm loop sleeps through timers/promises, whose binding is fixed when
// first imported: importing with the clock mocked lets mock.timers drive it.
mock.timers.enable({ apis: ['setTimeout'] });
const mfa = await import('../../../../src/modules/challenges/mfa.ts');
const { SUBMIT_CODE_JS } = await import('../../../../src/modules/challenges/mfa-page.ts');
mock.timers.reset();

const { DETECT_JS, fillCodeJS } = mfa;
/** A valid TOTP secret. */
const SECRET = 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ';
/** Where a person would finish by hand. */
const LIVE = 'https://oya.example/live/b1';
/** The prompt, as DETECT_JS reports it. */
const PROMPT = { present: true, segmented: false, fieldCount: 1 };
/** The page once the prompt is gone. */
const GONE = { present: false };

describe('mfa.complete', () => {
  beforeEach(() => {
    mfa.reset();
    mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 59_000 });
  });
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('reports no challenge when the page is not asking for one', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [GONE]]]);
    assert.deepEqual(await mfa.complete(evaluate, 'p1'), { present: false, completed: false, method: 'none' });
  });

  it('hands the challenge to a person when no factor is configured', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [PROMPT]]]);
    const result = await mfa.complete(evaluate, 'p1', { domain: 'portal.example', liveViewUrl: LIVE });
    assert.equal(result.method, 'handoff');
    assert.equal(result.liveViewUrl, LIVE);
    assert.match(result.error, /configured for portal\.example or this persona/);
  });

  it('hands a push-approval prompt to a person even with a factor configured', async () => {
    await mfa.set('p1', { type: 'totp', secret: SECRET });
    const evaluate = scriptedPage([[DETECT_JS, [{ present: true, handoff: true }]]]);
    assert.equal((await mfa.complete(evaluate, 'p1')).method, 'handoff');
  });

  it('types the current TOTP code, submits it and confirms the prompt went away', async () => {
    await mfa.set('p1', { type: 'totp', secret: SECRET });
    const fill = fillCodeJS('287082', false);
    const evaluate = scriptedPage([
      [DETECT_JS, [PROMPT, GONE]],
      [fill, [{ filled: true }]],
      [SUBMIT_CODE_JS, [true]],
    ]);
    const result = await mfa.complete(evaluate, 'p1', { liveViewUrl: LIVE });
    assert.deepEqual(result, {
      present: true,
      completed: true,
      filled: true,
      submitted: true,
      method: 'totp',
      segmented: false,
    });
    assert.ok(scriptsRun(evaluate).includes(fill), 'the RFC code for this moment was typed');
  });

  it('types one digit per box on a segmented prompt', async () => {
    await mfa.set('p1', { type: 'totp', secret: SECRET });
    const fill = fillCodeJS('287082', true);
    const evaluate = scriptedPage([
      [DETECT_JS, [{ present: true, segmented: true }, GONE]],
      [fill, [{ filled: true }]],
      [SUBMIT_CODE_JS, [true]],
    ]);
    const result = await mfa.complete(evaluate, 'p1');
    assert.equal(result.segmented, true);
    assert.equal(result.completed, true);
  });

  it('keeps checking for the prompt to go until the confirm window closes', async () => {
    await mfa.set('p1', { type: 'totp', secret: SECRET });
    const evaluate = scriptedPage([
      [DETECT_JS, [PROMPT, PROMPT, new Error('navigating'), GONE]],
      [fillCodeJS('287082', false), [{ filled: true }]],
      [SUBMIT_CODE_JS, [true]],
    ]);
    const pending = mfa.complete(evaluate, 'p1');
    await advance(MFA_POLL_MS, 2);
    assert.equal((await pending).completed, true);
  });

  it('reports a code the site never confirmed, with the live view', async () => {
    await mfa.set('p1', { type: 'totp', secret: SECRET });
    const evaluate = scriptedPage([
      [DETECT_JS, [PROMPT]],
      [fillCodeJS('287082', false), [{ filled: true }]],
      [SUBMIT_CODE_JS, [true]],
    ]);
    const pending = mfa.complete(evaluate, 'p1', { liveViewUrl: LIVE });
    await advance(MFA_POLL_MS, MFA_CONFIRM_MS / MFA_POLL_MS);
    const result = await pending;
    assert.equal(result.completed, false);
    assert.equal(result.liveViewUrl, LIVE);
    assert.match(result.error, /has not confirmed it/);
  });

  it('reports a field it could not fill', async () => {
    await mfa.set('p1', { type: 'totp', secret: SECRET });
    const evaluate = scriptedPage([
      [DETECT_JS, [PROMPT]],
      [fillCodeJS('287082', false), [{ filled: false, reason: 'field not found' }]],
    ]);
    const result = await mfa.complete(evaluate, 'p1');
    assert.equal(result.filled, false);
    assert.equal(result.error, 'Could not fill the code field');
  });

  it('reports why the code could not be had', async () => {
    await mfa.set('p1', { type: 'gmail', refreshToken: 'rt-mfa', clientId: 'c' });
    stubFetch(() => text('invalid_grant', 400));
    const evaluate = scriptedPage([[DETECT_JS, [PROMPT]]]);
    const result = await mfa.complete(evaluate, 'p1', { liveViewUrl: LIVE });
    assert.equal(result.method, 'gmail');
    assert.equal(result.completed, false);
    assert.match(result.error, /Mailbox token refresh failed \(400\)/);
  });
});

describe('mfa.submitCode', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
  afterEach(() => mock.timers.reset());

  it('detects the prompt itself before typing a person’s code', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [PROMPT, GONE]],
      [fillCodeJS('K7R4QP', false), [{ filled: true }]],
      [SUBMIT_CODE_JS, [true]],
    ]);
    const result = await mfa.submitCode(evaluate, 'K7R4QP');
    assert.deepEqual(result, { present: true, filled: true, submitted: true, completed: true });
  });

  it('types nothing when there is no prompt to type into', async () => {
    const evaluate = scriptedPage([[DETECT_JS, [{ present: true, handoff: true }]]]);
    assert.deepEqual(await mfa.submitCode(evaluate, '445566'), {
      present: false,
      filled: false,
      submitted: false,
      completed: false,
    });
  });

  it('checks once, without waiting, when nothing could be pressed', async () => {
    const evaluate = scriptedPage([
      [DETECT_JS, [PROMPT]],
      [fillCodeJS('445566', false), [{ filled: true }]],
      [SUBMIT_CODE_JS, [false]],
    ]);
    const result = await mfa.submitCode(evaluate, '445566', false);
    assert.deepEqual(result, { present: true, filled: true, submitted: false, completed: false });
  });
});

describe('mfa page scripts', () => {
  it('embeds the code as a JSON string, so it cannot break out of the script', () => {
    const script = fillCodeJS('1"); alert(1); ("', true);
    assert.ok(script.includes(JSON.stringify('1"); alert(1); ("')));
    assert.ok(script.includes('if (true)'));
    assert.ok(fillCodeJS(445566, false).includes('"445566"'));
  });
});
