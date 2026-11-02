/** Direct selection owns one native session across timeout/cancel and never treats identification as capture. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  NativeWindowSelector,
  nativeWindowSupported,
} from '../../../../src/main/screen-share/native-window-selector.ts';
import { NATIVE_WINDOW_TIMEOUT_MS } from '../../../../src/main/screen-share/constants.ts';

/** A terminal callback is distinct from native deactivation, matching Apple's undocumented dismissal acknowledgment. */
function fixture() {
  let active = false,
    callback: ((result: unknown) => void) | undefined,
    cancels = 0;
  const adapter = {
    ready: () => !active,
    start: (answer: (result: unknown) => void) => {
      active = true;
      callback = answer;
      return true;
    },
    cancel: () => {
      cancels++;
    },
    current: () => true,
  };
  const selector = new NativeWindowSelector('/owned/app', { platform: 'darwin', version: '15.2', load: () => adapter });
  const finish = (result: unknown) => {
    active = false;
    callback?.(result);
    callback = undefined;
  };
  return { selector, adapter, finish, cancels: () => cancels };
}
const selected = { status: 'selected', windowId: 43, processId: 123, processStarted: 456 };

test('direct selection is limited to supported macOS without changing other platform startup', () => {
  for (const version of ['13.0', '14.7', '15.0', '15.1', ''])
    assert.equal(nativeWindowSupported('darwin', version), false);
  for (const version of ['15.2', '15.7', '26.6.2']) assert.equal(nativeWindowSupported('darwin', version), true);
  assert.equal(nativeWindowSupported('win32', '26.0'), false);
  let loads = 0;
  const selector = new NativeWindowSelector('/owned/app', {
    platform: 'linux',
    version: '26.0',
    load: () => {
      loads++;
      throw Error('unsupported');
    },
  });
  assert.equal(selector.state(), 'unsupported');
  assert.equal(loads, 0);
});

test('native identification returns a candidate without starting capture', async () => {
  const f = fixture();
  const choice = f.selector.select([99], new AbortController().signal);
  assert.equal(f.selector.state(), 'unavailable');
  f.finish(selected);
  assert.deepEqual(await choice, selected);
  assert.equal(f.selector.state(), 'ready');
});

test('cancelling releases the caller but quarantines native selection until its late terminal answer', async () => {
  const f = fixture(),
    controller = new AbortController();
  const choice = f.selector.select([], controller.signal);
  controller.abort();
  assert.deepEqual(await choice, { status: 'cancelled' });
  assert.equal(f.cancels(), 1);
  assert.equal(f.selector.state(), 'unavailable');
  assert.deepEqual(await f.selector.select([], new AbortController().signal), { status: 'unavailable' });
  f.finish(selected);
  assert.equal(f.selector.state(), 'ready');
  const next = f.selector.select([], new AbortController().signal);
  f.finish({ status: 'cancelled' });
  assert.deepEqual(await next, { status: 'cancelled' });
});

test('native timeout keeps the abandoned observer quarantined while permitting list fallback', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture();
  const choice = f.selector.select([], new AbortController().signal);
  t.mock.timers.tick(NATIVE_WINDOW_TIMEOUT_MS);
  assert.deepEqual(await choice, { status: 'unavailable' });
  assert.equal(f.cancels(), 1);
  assert.equal(f.selector.state(), 'unavailable');
  f.finish(selected);
  assert.equal(f.selector.state(), 'ready');
});

test('a failed deactivation still settles the caller without reopening native admission', async () => {
  const f = fixture(),
    controller = new AbortController();
  f.adapter.cancel = () => {
    throw Error('native unavailable');
  };
  const choice = f.selector.select([], controller.signal);
  controller.abort();
  assert.deepEqual(await choice, { status: 'cancelled' });
  assert.equal(f.selector.state(), 'unavailable');
  f.finish({ status: 'cancelled' });
});

test('malformed native identities cannot become source candidates', async () => {
  for (const result of [
    null,
    {},
    { ...selected, windowId: NaN },
    { ...selected, processId: 0 },
    { ...selected, processStarted: Infinity },
  ]) {
    const f = fixture();
    const choice = f.selector.select([], new AbortController().signal);
    f.finish(result);
    assert.deepEqual(await choice, { status: 'unavailable' });
  }
});

test('missing native artifact falls back without repeated module loading or pretending feature support succeeded', () => {
  let loads = 0;
  const selector = new NativeWindowSelector('/owned/app', {
    platform: 'darwin',
    version: '26.0',
    load: () => {
      loads++;
      throw Error('missing');
    },
  });
  assert.equal(selector.state(), 'unavailable');
  assert.equal(selector.state(), 'unavailable');
  assert.equal(loads, 1);
});
