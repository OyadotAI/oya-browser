/**
 * Unit tests for the browser commands (src/commands/browsers.ts): what they
 * send and how they print.
 */
import { FLAGS, captured, fakeFetch } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { cmdAsk, cmdGoto, cmdLs, cmdRm, cmdStart, cmdStatus } from '../../../src/commands/browsers.ts';

const BROWSER = {
  id: 'b1',
  name: 'shop',
  provider: null,
  persona: 'p1',
  personaName: null,
  health: 'stale',
  currentUrl: 'https://example.com/a',
  commands: 4,
  errors: 1,
  pending: 0,
  lastError: 'timeout',
};

describe('browser commands', () => {
  afterEach(() => mock.restoreAll());

  it('start maps flags to start options', async () => {
    const calls = fakeFetch({
      'POST /api/browsers/start': { id: 'b9', provider: 'steel', persona: 'auto', status: 'ready' },
    });
    const flags = {
      ...FLAGS,
      persona: 'auto',
      'queue-ms': '500',
      governed: true,
      policy: '{"region":"eu"}',
      captcha: false,
    };
    const { out } = await captured(() => cmdStart(flags));
    assert.deepEqual(calls[0].body, { profile: 'auto', queueMs: 500, governed: true, policy: { region: 'eu' } });
    assert.match(out, /✅ b9\n {3}provider: steel {3}persona: auto/);
  });

  it('ls prints one padded row per browser and a count', async () => {
    fakeFetch({ 'GET /api/browsers': [BROWSER] });
    const { out } = await captured(() => cmdLs(FLAGS));
    assert.equal(
      out,
      `◐ b1  ${'oya'.padEnd(14)} ${'p1'.padEnd(14)} ${'shop'.padEnd(18)} 4·1  example.com/a\n\n1 running.`,
    );
  });

  it('ls says when nothing is running', async () => {
    fakeFetch({ 'GET /api/browsers': [] });
    assert.equal((await captured(() => cmdLs(FLAGS))).out, 'No browsers running.');
  });

  it('status prints health, counters, the last error and recent actions', async () => {
    const activity = [{ action: 'click', summary: 'Buy', ok: false, ms: 12, error: 'gone' }];
    fakeFetch({ 'GET /api/browsers/b1': { ...BROWSER, activity } });
    const { out } = await captured(() => cmdStatus({ ...FLAGS, id: 'b1' }));
    assert.match(out, /stale · oya · persona p1/);
    assert.match(out, /last error: timeout/);
    assert.match(out, /✗ click {14}Buy, gone {2}12ms/);
  });

  it('rm reports each browser, including a sandbox left behind', async () => {
    const results = [
      { id: 'b1', ok: true, sandboxRemoved: true },
      { id: 'b2', ok: false, sandboxRemoved: false, error: 'x' },
    ];
    fakeFetch({ 'POST /api/browsers/stop': { stopped: 1, results } });
    const { out } = await captured(() => cmdRm(['b1', 'b2'], FLAGS));
    assert.equal(out, '✅ b1 (sandbox destroyed)\n✗ b2, sandbox NOT removed, check Oya Cloud x\nstopped 1');
  });

  it('refuse to run without their arguments', async () => {
    await assert.rejects(cmdRm([], FLAGS), /Usage: oya rm/);
    await assert.rejects(cmdGoto([], FLAGS), /Usage: oya goto <url>/);
    await assert.rejects(cmdAsk([], FLAGS), /Usage: oya ask/);
  });

  it('ask joins its words into one prompt', async () => {
    const calls = fakeFetch({ 'GET /api/browsers/b1': BROWSER, 'POST /api/browsers/b1/chat': { text: 'found it' } });
    const { out } = await captured(() => cmdAsk(['find', 'pricing'], { ...FLAGS, id: 'b1' }));
    assert.equal(out, 'found it');
    assert.deepEqual((calls[1].body as { messages: unknown }).messages, [{ role: 'user', content: 'find pricing' }]);
  });
});
