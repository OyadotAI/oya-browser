/** Execute lifecycle scripts against isolated document seams; never use a debugging transport. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createContext, runInContext } from 'node:vm';
import { NATIVE_RECORDING_COMMAND_MS } from '../../../../src/main/native/constants.ts';
import { NativeDocumentRecorder } from '../../../../src/main/native/index.ts';
/** A minimal analyzer records lifecycle calls while the production lifecycle scripts run unchanged. */
const analyzer = `
  window.__acRecordStart = () => { starts++; };
  window.__acRecordStop = () => { stops++; window.__acRecordSink?.({steps:[{id:'final'}]}); };
  window.__acRecordDrain = final => ({steps:[{id:final ? 'final' : 'buffered'}]});
`;
/** Native isolated execution runs in a VM only for hermetic unit coverage; Oya integration covers the real engine. */
function fixture(loading = false, source = analyzer) {
  const document = Object.assign(new EventTarget(), { readyState: loading ? 'loading' : 'complete' });
  const events = new EventTarget();
  const sent: unknown[] = [];
  const bridge = { documentId: 'original', emit: (...args: unknown[]) => sent.push(args) };
  const context = createContext({ document, __oyaNativeRecording: bridge, starts: 0, stops: 0 });
  context.window = context;
  context.addEventListener = events.addEventListener.bind(events);
  context.removeEventListener = events.removeEventListener.bind(events);
  const frame = {
    detached: false,
    _executeJavaScriptInOyaWorld: async (script: string) => runInContext(script, context),
  };
  const identity = { documentId: 'original', frame: frame as never, frames: [], url: 'https://original.test' };
  const recorder = new NativeDocumentRecorder(identity, 'epoch', source);
  return { recorder, identity, document, events, bridge, context, frame, sent };
}
test('starts once and drains only this authorized document', async () => {
  const f = fixture();
  const first = f.recorder.start();
  assert.equal(f.recorder.start(), first);
  await first;
  assert.equal(f.context.starts, 1);
  assert.equal(JSON.stringify(await f.recorder.drain()), '{"steps":[{"id":"buffered"}]}');
  const stop = f.recorder.stop();
  assert.equal(f.recorder.stop(), stop);
  assert.equal(JSON.stringify(await stop), '{"steps":[{"id":"final"}]}');
  assert.equal(f.context.stops, 1);
  assert.equal(f.sent.length, 0);
  assert.equal(f.context.__acRecordSink, undefined);
});
test('readiness waits for DOMContentLoaded', async () => {
  const f = fixture(true);
  const ready = f.recorder.start();
  assert.equal(f.context.starts, 0);
  f.document.dispatchEvent(new Event('DOMContentLoaded'));
  await ready;
  assert.equal(f.context.starts, 1);
  await f.recorder.stop();
});
test('stop before readiness cancels the listener and cannot resurrect the recorder', async () => {
  const f = fixture(true);
  const rejected = assert.rejects(f.recorder.start(), /cancelled/);
  await f.recorder.stop();
  await rejected;
  f.document.dispatchEvent(new Event('DOMContentLoaded'));
  assert.equal(f.context.starts, 0);
  assert.equal(f.context.__oyaDocumentRecorder, undefined);
  await assert.rejects(f.recorder.start(), /stopped/);
});
test('pagehide before readiness prevents late arming', async () => {
  const f = fixture(true);
  const rejected = assert.rejects(f.recorder.start(), /cancelled/);
  f.events.dispatchEvent(new Event('pagehide'));
  f.document.dispatchEvent(new Event('DOMContentLoaded'));
  await rejected;
  assert.equal(f.context.starts, 0);
});
test('replacement document is not touched by stale start drain or stop', async () => {
  const f = fixture();
  await f.recorder.start();
  f.bridge.documentId = 'replacement';
  await assert.rejects(f.recorder.drain(), /document changed/);
  await assert.rejects(f.recorder.stop(), /document changed/);
  assert.equal(f.context.stops, 0);
  const next = new NativeDocumentRecorder(f.identity, 'next', analyzer);
  await assert.rejects(next.start(), /document changed/);
  assert.equal(f.context.starts, 1);
});
test('another recorder cannot take ownership and failed cleanup leaves the current recorder alone', async () => {
  const f = fixture();
  await f.recorder.start();
  const other = new NativeDocumentRecorder(f.identity, 'other', analyzer);
  await assert.rejects(other.start(), /already has a recorder/);
  assert.equal(f.context.stops, 0);
  await f.recorder.stop();
});
test('a completed recorder can be replaced without an old stop affecting its successor', async () => {
  const f = fixture();
  await f.recorder.start();
  await f.recorder.stop();
  const next = new NativeDocumentRecorder(f.identity, 'next', analyzer);
  await next.start();
  await f.recorder.stop();
  assert.equal(f.context.starts, 2);
  assert.equal(f.context.stops, 1);
  await next.stop();
});
test('failed analyzer installation releases lifecycle ownership', async () => {
  const f = fixture(false, "throw new Error('analyzer failed')");
  await assert.rejects(f.recorder.start(), /analyzer failed/);
  assert.equal(f.context.__oyaDocumentRecorder, undefined);
});
test('stopping an unstarted recorder executes nothing and is terminal', async () => {
  const f = fixture();
  f.frame._executeJavaScriptInOyaWorld = async () => assert.fail('must not execute');
  await f.recorder.stop();
  await assert.rejects(f.recorder.start(), /stopped/);
  await assert.rejects(f.recorder.drain(), /not running/);
});
test('detached frames fail explicitly without retargeting another document', async () => {
  const f = fixture();
  f.frame.detached = true;
  await assert.rejects(f.recorder.start(), /detached/);
});

test('readiness timeout cancels pending arming without leaving a future DOM listener', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture(true);
  const rejected = assert.rejects(f.recorder.start(), /did not answer/);
  t.mock.timers.tick(NATIVE_RECORDING_COMMAND_MS);
  await rejected;
  f.document.dispatchEvent(new Event('DOMContentLoaded'));
  assert.equal(f.context.starts, 0);
  assert.equal(f.context.__oyaDocumentRecorder, undefined);
});

test('clear discards unfinished data without stopping or changing the authorized recorder owner', async () => {
  const f = fixture();
  f.context.__acRecordClear = () => {
    f.context.cleared = true;
  };
  await assert.rejects(f.recorder.clear(), /not running/);
  await f.recorder.start();
  const owner = f.context.__oyaDocumentRecorder.owner;
  await f.recorder.clear();
  assert.equal(f.context.cleared, true);
  assert.equal(f.context.__oyaDocumentRecorder.owner, owner);
  assert.equal(f.context.stops, 0);
  await f.recorder.stop();
  await assert.rejects(f.recorder.clear(), /not running/);
});

test('a stale clear never discards data belonging to a replacement document', async () => {
  const f = fixture();
  f.context.__acRecordClear = () => assert.fail('Replacement must not be cleared');
  await f.recorder.start();
  f.bridge.documentId = 'replacement';
  await assert.rejects(f.recorder.clear(), /document changed/);
});

test('clear waiting for readiness cannot run after stop cancels the pending recorder', async () => {
  const f = fixture(true);
  f.context.__acRecordClear = () => assert.fail('Stopped recorder must not be cleared');
  const starting = assert.rejects(f.recorder.start(), /cancelled/);
  const clearing = assert.rejects(f.recorder.clear(), /cancelled/);
  await f.recorder.stop();
  await Promise.all([starting, clearing]);
});
