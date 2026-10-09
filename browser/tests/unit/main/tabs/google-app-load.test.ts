/** The handoff releases every listener on readiness, failure, destruction and timeout. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { GoogleAppLoad } from '../../../../src/main/tabs/google-app-load.ts';
import { GOOGLE_APP_HANDOFF_TIMEOUT } from '../../../../src/main/tabs/constants.ts';

/** A main document whose complete load can deliberately remain pending. */
function setup(accept = () => true) {
  const contents = new EventEmitter();
  const ready = Promise.withResolvers();
  const tab = { view: { webContents: contents }, ready: ready.promise };
  return { contents, ready, wait: new GoogleAppLoad(tab as any, accept).wait() };
}
it('times out without retaining page listeners', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const load = setup();
  const rejected = assert.rejects(load.wait, /timed out/);
  t.mock.timers.tick(GOOGLE_APP_HANDOFF_TIMEOUT);
  await rejected;
  assert.deepEqual(load.contents.eventNames(), []);
});
it('rejects a destroyed candidate and removes its readiness listeners', async () => {
  const load = setup();
  const rejected = assert.rejects(load.wait, /closed/);
  load.contents.emit('destroyed');
  await rejected;
  assert.deepEqual(load.contents.eventNames(), []);
});
it('rejects a completed load that landed outside the approved destination', async () => {
  const load = setup(() => false);
  const rejected = assert.rejects(load.wait, /changed/);
  load.ready.resolve();
  await rejected;
  assert.deepEqual(load.contents.eventNames(), []);
});
