/** Background replays retain the local exclusion gate and respect human ownership. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { PlaybookRunner } from '../../../../src/main/connection/playbook-run.ts';
import { ServerApi } from '../../../../src/main/connection/server-api.ts';
/** Allow pending promise continuations to settle without advancing timers. */
const settle = async () => {
  for (let i = 0; i < 12; i++) await Promise.resolve();
};
/** Shared services seam used by replay, Ask, and routine calls. */
function context() {
  const state = { mode: 'human', mine: true };
  const changes: string[] = [];
  const deps = {
    config: { values: { apiKey: 'test' } },
    socket: {},
    recorder: { recording: false },
    chatAbort: null as AbortController | null,
    control: {
      snapshot: () => state,
      change: async (action: string) => {
        changes.push(action);
        state.mode = action === 'return' ? 'agent' : 'human';
      },
    },
  };
  return { deps, state, changes, runner: new PlaybookRunner(deps as any) };
}
it('refuses a replay while recording or another agent operation is active', async () => {
  const { deps, runner } = context();
  deps.recorder.recording = true;
  await assert.rejects(runner.start({}), /Finish the current/);
  deps.recorder.recording = false;
  deps.chatAbort = new AbortController();
  await assert.rejects(runner.start({}), /Finish the current/);
});
it('returns control and clears its exclusion if submission is refused', async (t) => {
  const { runner, deps, changes } = context();
  t.mock.method(ServerApi.prototype, 'postToBrowser', async () => {
    throw new Error('Server refused run');
  });
  await assert.rejects(runner.start({ playbook: 'lookup' }), /Server refused/);
  assert.deepEqual(changes, ['return', 'acquire']);
  assert.equal(deps.chatAbort, null);
});
it('keeps exclusion through polling errors and restores the previous hold when finished', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const { runner, deps, changes } = context();
  let polls = 0;
  t.mock.method(ServerApi.prototype, 'postToBrowser', async () => ({
    ok: true,
    json: async () => ({ id: 'run1', status: 'running' }),
  }));
  t.mock.method(ServerApi.prototype, 'get', async () => {
    if (++polls === 1) throw new Error('Disconnected');
    return { status: 'succeeded' };
  });
  await runner.start({ playbook: 'lookup' });
  assert.ok(deps.chatAbort);
  t.mock.timers.tick(1000);
  await settle();
  assert.ok(deps.chatAbort);
  t.mock.timers.tick(1000);
  await settle();
  assert.equal(deps.chatAbort, null);
  assert.deepEqual(changes, ['return', 'acquire']);
});
