/** OS waits obey cancellation and deadlines without exposing late completion to setup. */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { awaitCancelable } from '../../../../src/main/chatgpt/cancellation.ts';
import { boundedSignal } from '../../../../src/main/chatgpt/oauth.ts';
it('a completed operation releases its abort listener', async () => {
  const controller = new AbortController();
  const removed = mock.method(controller.signal, 'removeEventListener');
  assert.equal(await awaitCancelable(Promise.resolve('done'), controller.signal), 'done');
  assert.equal(removed.mock.callCount(), 1);
});
it('a pre-aborted operation rejects even when the OS promise never settles', async () => {
  const controller = new AbortController();
  controller.abort(Error('Canceled fixture'));
  await assert.rejects(awaitCancelable(new Promise(() => {}), controller.signal), /Canceled fixture/);
});
it('the configured deadline settles an OS wait without user cancellation', async (t) => {
  const deadline = new AbortController();
  t.mock.method(AbortSignal, 'timeout', (ms: number) => {
    assert.equal(ms, 30000);
    return deadline.signal;
  });
  const pending = awaitCancelable(new Promise(() => {}), boundedSignal());
  deadline.abort(Error('Timeout fixture'));
  await assert.rejects(pending, /Timeout fixture/);
});
