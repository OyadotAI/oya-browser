/** Native channel lifecycle executes unchanged recorder scripts in isolated document seams. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createContext, runInContext } from 'node:vm';
import { NATIVE_RECORDING_SETTLE_MS } from '../../../../src/main/recording/constants.ts';
import { NativeRecordingChannel } from '../../../../src/main/recording/native-channel.ts';
/** Analyzer seam exercises actual lifecycle ownership, stop cancellation and final draining. */
const analyzer = `window.__acRecordStart=()=>{starts++};window.__acRecordStop=()=>{stops++};window.__acRecordDrain=final=>({steps:[{id:final?'final':'buffered',action:'type',text:'text'}],secrets:['password']});`;
/** The native frame survives document replacement, while each isolated context receives a fresh bridge. */
function fixture(loading = false) {
  let context: any;
  const batches: any[] = [];
  const frame = Object.assign(new EventEmitter(), {
    detached: false,
    url: 'https://fixture.test/',
    framesInSubtree: [] as any[],
    _executeJavaScriptInOyaWorld: async (script: string) => runInContext(script, context),
  });
  frame.framesInSubtree = [frame];
  const page = Object.assign(new EventEmitter(), {
    mainFrame: frame,
    isDestroyed: () => false,
    _supportsOyaFrameLifecycle: () => true,
  });
  Object.defineProperty(page, 'debugger', {
    get() {
      assert.fail('Internal CDP forbidden');
    },
  });
  const channel = new NativeRecordingChannel(page as any, analyzer, (batch) => batches.push(batch));
  const replace = (id: string, pending = false) => {
    const document = Object.assign(new EventTarget(), { readyState: pending ? 'loading' : 'complete' });
    const events = new EventTarget();
    context = createContext({ document, starts: 0, stops: 0, __oyaNativeRecording: { documentId: id, emit() {} } });
    context.window = context;
    context.addEventListener = events.addEventListener.bind(events);
    context.removeEventListener = events.removeEventListener.bind(events);
    return context;
  };
  replace('first', loading);
  return {
    channel,
    page,
    frame,
    batches,
    replace,
    get context() {
      return context;
    },
  };
}
test('arms once and preserves native frame attribution on drained final data', async () => {
  const f = fixture();
  const ready = f.channel.start();
  assert.equal(f.channel.start(), ready);
  await ready;
  assert.equal(f.context.starts, 1);
  await f.channel.drain();
  const stop = f.channel.stop();
  assert.equal(f.channel.stop(), stop);
  await stop;
  assert.equal(f.context.stops, 1);
  assert.equal(f.batches.at(-1).steps[0].id, 'final');
  assert.deepEqual(f.batches.at(-1).steps[0].frames, []);
  assert.equal(f.page.listenerCount('ipc-message'), 0);
  assert.equal(f.frame.listenerCount('dom-ready'), 0);
  await assert.rejects(f.channel.start(), /stopped/);
});
test('native readiness automatically arms replacement documents without touching old context state', async () => {
  const f = fixture();
  await f.channel.start();
  const old = f.context;
  f.replace('second');
  f.frame.emit('dom-ready');
  f.frame.emit('dom-ready');
  await f.channel.drain(true);
  assert.equal(f.context.starts, 1);
  assert.equal(old.starts, 1);
  assert.equal(old.stops, 0);
  await f.channel.stop();
  assert.equal(f.context.stops, 1);
});
test('stop cancels DOM-ready waiting without waiting for a timeout or resurrecting a recorder', async () => {
  const f = fixture(true);
  const ready = f.channel.start();
  const rejected = assert.rejects(ready, /cancelled/);
  for (let i = 0; i < 20; i++) await Promise.resolve();
  assert.ok(f.context.__oyaDocumentRecorder);
  await f.channel.stop();
  await rejected;
  f.context.document.dispatchEvent(new Event('DOMContentLoaded'));
  assert.equal(f.context.starts, 0);
  assert.equal(f.context.__oyaDocumentRecorder, undefined);
});
test('unsupported native lifecycle refuses startup and removes inbox listeners', async () => {
  const f = fixture();
  f.page._supportsOyaFrameLifecycle = () => false;
  await assert.rejects(f.channel.start(), /native recording frame lifecycle/);
  assert.equal(f.page.listenerCount('ipc-message'), 0);
});
test('a replaced document is never stopped through a stale frame wrapper', async () => {
  const f = fixture();
  await f.channel.start();
  f.replace('replacement');
  await f.channel.stop();
  assert.equal(f.context.starts, 0);
  assert.equal(f.context.stops, 0);
});

test('a stalled native identity lookup refuses startup within its deadline and releases listeners', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture();
  f.frame._executeJavaScriptInOyaWorld = async () => new Promise(() => {});
  const rejected = assert.rejects(f.channel.start(), /authorization timed out/);
  t.mock.timers.tick(NATIVE_RECORDING_SETTLE_MS);
  await rejected;
  assert.equal(f.page.listenerCount('ipc-message'), 0);
  assert.equal(f.frame.listenerCount('dom-ready'), 0);
});

test('clear targets the current document and leaves the channel armed', async () => {
  const f = fixture();
  f.context.__acRecordClear = () => {
    f.context.cleared = true;
  };
  await f.channel.start();
  await f.channel.clear();
  assert.equal(f.context.cleared, true);
  assert.equal(f.context.stops, 0);
  f.replace('replacement');
  f.context.__acRecordClear = () => assert.fail('Stale controller must not clear a replacement');
  await f.channel.clear();
  await f.channel.stop();
  await assert.rejects(f.channel.clear(), /stopped/);
});
