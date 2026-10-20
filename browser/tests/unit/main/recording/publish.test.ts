/**
 * Unit tests for src/main/recording/publish.ts, saving a recording as a playbook: the request the server
 * gets, the draft marked published only if unchanged, and the error answers.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { RecordingPublisher } from '../../../../src/main/recording/publish.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
import { idOf } from '../../../../src/main/app/saved-accounts.ts';

/** The account the tests' browser is on: its server and key, as a draft records it. */
const HERE = idOf('wss://oya.test/ws', 'k1');

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
    assert.equal(ctx.workspace.draft.publishedName, 'Login', 'the draft remembers the playbook it became');
  });

  describe('a draft saved before under another name', () => {
    /** Answers the rename with `patch`, the save with `post` and the taken-name check with `get`, recording each. */
    const answers = (
      patch: any,
      post: any = { ok: true, status: 200, json: async () => ({ id: 'p1' }) },
      get: any = { ok: false, status: 404, json: async () => ({ error: 'No playbook named new' }) },
    ) =>
      (globalThis.fetch as any).mock.mockImplementation(async (url: any, init: any) => {
        requests.push({ url, init });
        const answer = { PATCH: patch, GET: get }[init.method as string] ?? post;
        if (answer instanceof Error) throw answer;
        return answer;
      });
    const refused = (status: number, error: string) => ({ ok: false, status, json: async () => ({ error }) });
    const methods = () => requests.map((r) => r.init.method);

    beforeEach(() =>
      Object.assign(ctx.workspace.draft, {
        publishedAt: 1,
        publishedRevision: 2,
        publishedName: 'old',
        publishedTo: HERE,
      }),
    );

    it('never renames a playbook saved to another server or project', async () => {
      answers({ ok: true, status: 200, json: async () => ({}) });
      ctx.config.values.apiKey = 'k2';
      await saveRecording(ctx, 'new', 'd');
      ctx.config.values = { serverUrl: 'wss://elsewhere.test/ws', apiKey: 'k1' };
      ctx.workspace.draft.publishedName = 'old';
      await saveRecording(ctx, 'new', 'd');
      delete ctx.workspace.draft.publishedTo;
      ctx.config.values = { serverUrl: 'wss://oya.test/ws', apiKey: 'k1' };
      ctx.workspace.draft.publishedName = 'old';
      await saveRecording(ctx, 'new', 'd');
      assert.deepEqual(methods(), ['POST', 'POST', 'POST']);
    });

    it('remembers which server and project a save went to', async () => {
      answers({ ok: true, status: 200, json: async () => ({}) });
      ctx.config.values.apiKey = 'k2';
      await saveRecording(ctx, 'new', 'd');
      assert.equal(ctx.workspace.draft.publishedTo, idOf('wss://oya.test/ws', 'k2'));
    });

    it('renames the saved playbook with the server’s own rename, then saves', async () => {
      answers({ ok: true, status: 200, json: async () => ({ name: 'new' }) });
      assert.deepEqual(await saveRecording(ctx, 'new', 'd'), { id: 'p1', renamed: { from: 'old' } });
      assert.deepEqual(methods(), ['PATCH', 'POST']);
      assert.equal(requests[0].url, 'https://oya.test/api/playbooks/old');
      assert.equal(requests[0].init.headers.Authorization, 'Bearer k1');
      assert.equal(requests[0].init.body, '{"name":"new"}');
      assert.ok(requests[0].init.signal instanceof AbortSignal, 'the rename carries a time limit');
      assert.equal(ctx.workspace.draft.publishedName, 'new');
    });

    it('url-encodes the saved name, which was read back from disk', async () => {
      ctx.workspace.draft.publishedName = 'a b/c';
      answers({ ok: true, status: 200, json: async () => ({}) });
      await saveRecording(ctx, 'new', 'd');
      assert.equal(requests[0].url, 'https://oya.test/api/playbooks/a%20b%2Fc');
    });

    it('does not rename for the same name, a copy, or a draft saved before drafts kept the name', async () => {
      answers({ ok: true, status: 200, json: async () => ({}) });
      await saveRecording(ctx, 'old', 'd');
      ctx.workspace.draft.publishedName = 'old';
      await new RecordingPublisher(ctx).save('copy', 'd', true);
      delete ctx.workspace.draft.publishedName;
      await saveRecording(ctx, 'other', 'd');
      ctx.workspace.draft.publishedName = 42;
      await saveRecording(ctx, 'other', 'd');
      assert.deepEqual(methods(), ['POST', 'POST', 'POST', 'POST']);
    });

    it('a copy leaves the saved playbook alone and now follows the copy', async () => {
      answers({ ok: true, status: 200, json: async () => ({}) });
      await new RecordingPublisher(ctx).save('copy', 'd', true);
      assert.equal(ctx.workspace.draft.publishedName, 'copy');
    });

    it('a taken name stops the save and keeps the name it was saved as', async () => {
      answers(refused(409, 'A playbook named new already exists'));
      assert.deepEqual(await saveRecording(ctx, 'new', 'd'), {
        error: 'A playbook named new already exists',
        renameFailed: { from: 'old', status: 409 },
      });
      assert.deepEqual(methods(), ['PATCH']);
      assert.equal(ctx.workspace.draft.publishedName, 'old');
    });

    it('a playbook already gone is saved anew under the new name, once the name is free', async () => {
      answers(refused(404, 'No playbook named old'));
      assert.deepEqual(await saveRecording(ctx, 'new', 'd'), { id: 'p1', renamed: { from: 'old', missing: true } });
      assert.deepEqual(methods(), ['PATCH', 'GET', 'POST']);
      assert.equal(requests[1].url, 'https://oya.test/api/playbooks/new/export');
      assert.equal(ctx.workspace.draft.publishedName, 'new');
    });

    it('a playbook already gone never overwrites another under the new name', async () => {
      answers(refused(404, 'No playbook named old'), undefined, { ok: true, status: 200, json: async () => ({}) });
      assert.deepEqual(await saveRecording(ctx, 'new', 'd'), {
        error: 'A playbook named new already exists',
        renameFailed: { from: 'old', status: 409 },
      });
      assert.deepEqual(methods(), ['PATCH', 'GET']);
      assert.equal(ctx.workspace.draft.publishedName, 'old');
    });

    it('a playbook already gone stops the save when the name check fails', async () => {
      answers(refused(404, 'No playbook named old'), undefined, refused(500, 'boom'));
      assert.deepEqual(await saveRecording(ctx, 'new', 'd'), { error: 'boom', renameFailed: { from: 'old' } });
      answers(
        refused(404, 'No playbook named old'),
        undefined,
        Object.assign(new Error('x'), { name: 'TimeoutError' }),
      );
      const timedOut: any = await saveRecording(ctx, 'new', 'd');
      assert.deepEqual(timedOut, { error: 'The server did not answer in time', renameFailed: { from: 'old' } });
      assert.ok(!methods().includes('POST'));
    });

    it('any other rename failure stops the save', async () => {
      for (const failure of [
        refused(500, 'boom'),
        new Error('offline'),
        Object.assign(new Error('aborted'), { name: 'TimeoutError' }),
      ]) {
        requests = [];
        answers(failure);
        const answer: any = await saveRecording(ctx, 'new', 'd');
        assert.deepEqual(methods(), ['PATCH'], String(answer.error));
        assert.equal(answer.renameFailed.from, 'old');
      }
      const timedOut: any = await saveRecording(ctx, 'new', 'd');
      assert.deepEqual(timedOut.renameFailed, { from: 'old', timedOut: true }, 'a timeout says the outcome is unknown');
      assert.equal(ctx.workspace.draft.publishedName, 'old');
    });

    it('a failed save after a rename keeps the new name, so a retry saves without renaming again', async () => {
      answers({ ok: true, status: 200, json: async () => ({}) }, { ok: false, status: 500, json: async () => ({}) });
      assert.deepEqual(await saveRecording(ctx, 'new', 'd'), {
        error: 'Server returned 500',
        renamed: { from: 'old' },
      });
      assert.equal(ctx.workspace.draft.publishedName, 'new');
      assert.equal(ctx.workspace.persisted, true);
      assert.equal(ctx.workspace.draft.publishedRevision, 2, 'a rename alone does not mark the draft saved');
      requests = [];
      answers({ ok: true, status: 200, json: async () => ({}) });
      await saveRecording(ctx, 'new', 'd');
      assert.deepEqual(methods(), ['POST']);
    });

    it('a rename does not write over another draft opened meanwhile', async () => {
      answers({ ok: true, status: 200, json: async () => ({}) }, { ok: false, status: 500, json: async () => ({}) });
      const other = { id: 'other', revision: 1, variables: {}, publishedName: 'x' };
      (globalThis.fetch as any).mock.mockImplementation(async (url: any, init: any) => {
        if (init.method === 'PATCH') ctx.workspace.draft = other;
        return init.method === 'PATCH'
          ? { ok: true, status: 200, json: async () => ({}) }
          : { ok: false, status: 500, json: async () => ({}) };
      });
      await saveRecording(ctx, 'new', 'd');
      assert.equal(other.publishedName, 'x');
    });
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
