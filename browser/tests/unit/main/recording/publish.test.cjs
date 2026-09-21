/**
 * Unit tests for saving a recording as a playbook: the request the server
 * gets, the draft marked published only if unchanged, and the error answers.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { saveRecording } = require('../../../../main/recording/publish.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('saveRecording', () => {
  let ctx, requests;
  beforeEach(() => {
    ctx = mainCtx();
    ctx.config.values = { serverUrl: 'wss://oya.test/ws', apiKey: 'k1' };
    ctx.recorder = { recording: false, recordedSteps: [{ action: 'click', t: 5 }], recordedSecrets: new Set(['pw']) };
    ctx.workspace = {
      draft: { id: 'd', revision: 3, variables: { a: {} } },
      persist() {
        this.persisted = true;
      },
    };
    requests = [];
    mock.method(globalThis, 'fetch', async (url, init) => {
      requests.push({ url, init });
      return { ok: true, status: 200, json: async () => ({ id: 'p1' }) };
    });
  });

  it('posts the steps without their timestamps, as this browser', async () => {
    assert.deepEqual(await saveRecording(ctx, 'Login', 'log in'), { id: 'p1' });
    assert.equal(requests[0].url, 'https://oya.test/api/browsers/b1/playbooks');
    assert.equal(requests[0].init.headers.Authorization, 'Bearer k1');
    assert.deepEqual(JSON.parse(requests[0].init.body), {
      schemaVersion: 2,
      variables: { a: {} },
      name: 'Login',
      prompt: 'log in',
      steps: [{ action: 'click' }],
      secrets: ['pw'],
    });
    assert.equal(ctx.workspace.persisted, true);
  });

  it('does not mark a draft published when it changed meanwhile', async () => {
    globalThis.fetch.mock.mockImplementation(async () => {
      ctx.workspace.draft.revision++;
      return { ok: true, status: 200, json: async () => ({}) };
    });
    await saveRecording(ctx, 'n', 'd');
    assert.equal(ctx.workspace.draft.publishedAt, undefined);
  });

  it('answers errors instead of throwing', async () => {
    ctx.socket.ready = false;
    assert.deepEqual(await saveRecording(ctx), { error: 'Not connected to server' });
    ctx.socket.ready = true;
    ctx.recorder.recordedSteps = [];
    assert.deepEqual(await saveRecording(ctx), { error: 'Nothing recorded yet' });
    ctx.recorder.recordedSteps = [{ action: 'click' }];
    globalThis.fetch.mock.mockImplementation(async () => ({
      ok: false,
      status: 502,
      json: async () => {
        throw new Error('html');
      },
    }));
    assert.deepEqual(await saveRecording(ctx), { error: 'Server returned 502' });
    globalThis.fetch.mock.mockImplementation(async () => {
      throw new Error('offline');
    });
    assert.deepEqual(await saveRecording(ctx), { error: 'offline' });
  });

  it('gives up on a server that does not answer, saying so', async () => {
    globalThis.fetch.mock.mockImplementation(async (url, init) => {
      assert.ok(init.signal instanceof AbortSignal, 'the save carries a timeout');
      throw Object.assign(new Error('aborted'), { name: 'TimeoutError' });
    });
    assert.deepEqual(await saveRecording(ctx, 'n', 'd'), { error: 'The server took too long to save. Try again.' });
  });

  it('stops a running recording before saving it', async () => {
    let stopped = false;
    ctx.recorder.recording = true;
    ctx.recorder.stopRecording = async () => (stopped = true);
    await saveRecording(ctx, 'n', 'd');
    assert.equal(stopped, true);
  });
});
