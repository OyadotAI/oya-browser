/**
 * Unit tests for the routes that drive a browser: one command (booked in
 * usage, failures answered with their status and code) and the chat route's
 * input checks.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { chat, runCommand } from '../../../../../src/modules/browsers/http/command.ts';
import * as usage from '../../../../../src/platform/usage.ts';
import { HttpError } from '../../../../../src/platform/errors.ts';
import { disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, driveBrowser, fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-command';

/** Runs a command on B and returns the response. */
async function run(body: object) {
  const res = new FakeResponse();
  await runCommand(fakeRequest({ key: 'k-cmd', params: { browserId: B }, body }), res);
  return res;
}

describe('runCommand', () => {
  beforeEach(() => {
    stubControl();
    usage.reset();
  });
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('answers with the browser’s result and counts the command', async () => {
    const driver = driveBrowser(B, (action, params) => ({ ok: true, data: { action, params } }));
    const res = await run({ action: 'click', params: { selector: '#go' } });
    assert.deepEqual(res.body, { ok: true, data: { action: 'click', params: { selector: '#go' } } });
    assert.equal(driver.sent[0].action, 'click');
    assert.equal(usage.current('k-cmd').commands, 1);
  });

  it('sends empty params when none are given', async () => {
    const driver = driveBrowser(B, () => ({ ok: true }));
    await run({ action: 'screenshot' });
    assert.deepEqual(driver.sent[0].params, {});
  });

  it('counts a failed result as a command error', async () => {
    driveBrowser(B, () => ({ ok: false, error: 'no such element' }));
    const res = await run({ action: 'click' });
    assert.equal(res.body.ok, false);
    assert.equal(usage.current('k-cmd').command_errors, 1);
  });

  it('refuses a server-internal action before reaching the browser', async () => {
    const driver = driveBrowser(B, () => ({ ok: true }));
    const res = await run({ action: 'evaluate_raw', params: { expression: '1' } });
    assert.equal(res.statusCode, 403);
    assert.equal(driver.sent.length, 0);
  });

  it('answers a thrown error with its status and code', async () => {
    driveBrowser(B, () => {
      throw new HttpError(409, 'Human has control', { code: 'control_held' });
    });
    const res = await run({ action: 'click' });
    assert.deepEqual(
      [res.statusCode, res.body],
      [409, { ok: false, error: 'Human has control', code: 'control_held' }],
    );
    assert.equal(usage.current('k-cmd').command_errors, 1);
  });

  it('answers a lost answer as 504 command_outcome_unknown', async () => {
    driveBrowser(B, () => {
      throw new Error('Command click timed out after 30s');
    });
    const res = await run({ action: 'click' });
    assert.deepEqual([res.statusCode, res.body.code], [504, 'command_outcome_unknown']);
  });

  it('answers 500 for a browser that is not connected', async () => {
    const res = await run({ action: 'click' });
    assert.deepEqual([res.statusCode, res.body.error], [500, `Browser ${B} not connected`]);
  });
});

describe('chat', () => {
  afterEach(() => mock.restoreAll());

  /** Runs chat with `body` and returns the response. */
  async function talk(body: object) {
    const res = new FakeResponse();
    await chat(fakeRequest({ params: { browserId: B }, body }), res);
    return res;
  }

  it('requires a messages array', async () => {
    const res = await talk({ messages: 'hi' });
    assert.deepEqual([res.statusCode, res.body], [400, { error: 'messages array required' }]);
  });

  it('refuses data that is not names to strings, numbers or files', async () => {
    const res = await talk({ messages: [], data: { 'bad name': 'x' } });
    assert.equal(res.statusCode, 400);
    assert.match(res.body.error, /^data must map names/);
  });

  it('refuses a file passed as a secret', async () => {
    const file = { file: 'a.txt', type: 'text/plain', b64: 'YQ==' };
    const res = await talk({ messages: [], secrets: { doc: file } });
    assert.equal(res.statusCode, 400);
  });

  it('says a run that never acted on a page cannot be saved as a playbook', async () => {
    const answer = { choices: [{ message: { role: 'assistant', content: 'The title is Northwind.' } }] };
    mock.method(globalThis, 'fetch', async () => new Response(JSON.stringify(answer), { status: 200 }));
    const saved = process.env.OPENAI_API_KEY;
    process.env.OPENAI_API_KEY = 'sk-test';
    try {
      const body = JSON.parse((await talk({ messages: [{ role: 'user', content: 'what is the title?' }] })).ended);
      assert.equal(body.text, 'The title is Northwind.');
      assert.equal(body.replayable, false);
    } finally {
      if (saved === undefined) delete process.env.OPENAI_API_KEY;
      else process.env.OPENAI_API_KEY = saved;
    }
  });

  it('commits to 200 and reports a failed chat in the body', async () => {
    mock.method(globalThis, 'fetch', async () => {
      throw new Error('network is off in tests');
    });
    mock.method(console, 'error', () => {});
    const res = await talk({ messages: [{ role: 'user', content: 'go' }] });
    assert.equal(res.statusCode, 200);
    const body = JSON.parse(res.ended);
    assert.ok(body.error);
    assert.ok(body.status >= 400);
  });
});
