/**
 * Unit tests for src/main/recording/publish.ts, saving a recording as a playbook: the request the server
 * gets, the draft marked published only if unchanged, and the error answers.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { RecordingPublisher } from '../../../../src/main/recording/publish.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** Saves through a publisher over `ctx`, as the IPC handler does. */
const saveRecording = (ctx: any, name?: any, description?: any) => new RecordingPublisher(ctx).save(name, description);

describe('saveRecording', () => {
  let ctx: any, requests: any[];
  beforeEach(() => {
    ctx = mainCtx();
    ctx.config.values = { serverUrl: 'wss://oya.test/ws', apiKey: 'k1' };
    ctx.recorder = { recording: false, recordedSteps: [{ action: 'click', t: 5 }], recordedSecrets: new Set(['pw']) };
    ctx.workspace = {
      draft: { id: 'd', revision: 3, variables: { a: {} } },
      persist(this: any) {
        this.persisted = true;
      },
    };
    requests = [];
    mock.method(globalThis, 'fetch', async (url: any, init: any) => {
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
    (globalThis.fetch as any).mock.mockImplementation(async () => {
      ctx.workspace.draft.revision++;
      return { ok: true, status: 200, json: async () => ({}) };
    });
    await saveRecording(ctx, 'n', 'd');
    assert.equal(ctx.workspace.draft.publishedAt, undefined);
  });

  it('reports a refused save as an error even when the server gives no reason', async () => {
    (globalThis.fetch as any).mock.mockImplementation(async () => ({ ok: false, status: 500, json: async () => ({}) }));
    assert.deepEqual(await saveRecording(ctx, 'n', 'd'), { error: 'Server returned 500' });
    assert.equal(ctx.workspace.draft.publishedAt, undefined);
  });

  it('marks a saved draft with the revision it saved', async () => {
    (globalThis.fetch as any).mock.mockImplementation(async () => ({ ok: true, status: 200, json: async () => ({}) }));
    await saveRecording(ctx, 'n', 'd');
    assert.equal(ctx.workspace.draft.publishedRevision, ctx.workspace.draft.revision);
  });

  it('answers errors instead of throwing', async () => {
    ctx.socket.ready = false;
    assert.deepEqual(await saveRecording(ctx), { error: 'Not connected to server' });
    ctx.socket.ready = true;
    ctx.recorder.recordedSteps = [];
    assert.deepEqual(await saveRecording(ctx), { error: 'Nothing recorded yet' });
    ctx.recorder.recordedSteps = [{ action: 'click' }];
    (globalThis.fetch as any).mock.mockImplementation(async () => ({
      ok: false,
      status: 502,
      json: async () => {
        throw new Error('html');
      },
    }));
    assert.deepEqual(await saveRecording(ctx), { error: 'Server returned 502' });
    (globalThis.fetch as any).mock.mockImplementation(async () => {
      throw new Error('offline');
    });
    assert.deepEqual(await saveRecording(ctx), { error: 'offline' });
  });

  it('gives up on a server that does not answer, saying so', async () => {
    (globalThis.fetch as any).mock.mockImplementation(async (url: any, init: any) => {
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
