/** Apple selection yields only a candidate; explicit Oya confirmation and request lifetime own final capture. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeShareFlow } from '../../../../src/main/screen-share/native-share-flow.ts';
import { NativeWindowChoice } from '../../../../src/main/screen-share/native-window-choice.ts';

/** Independent native completion, requester lifetime and OS enumeration model their real asynchronous boundary. */
function flowFixture() {
  const state = { live: true, destroyed: false, hidden: 0, shown: 0, focused: 0, cancelled: 0, enumerated: 0 };
  const pending = Promise.withResolvers<any>();
  const identity = { status: 'selected', windowId: 43, processId: 123, processStarted: 456 };
  const source = { id: 'window:43:0', name: 'Owned test window', thumbnail: { isEmpty: () => true } };
  const calls: any[] = [];
  const selector = {
    state: () => 'ready',
    current: () => state.live,
    select: (excluded, signal) => {
      calls.push({ excluded, signal });
      return pending.promise;
    },
  };
  const electron = {
    systemPreferences: { getMediaAccessStatus: () => 'granted' },
    desktopCapturer: {
      getSources: async () => {
        state.enumerated++;
        return [source];
      },
    },
  };
  const choice = new NativeWindowChoice(electron as any, selector as any, () => state.live);
  const window = {
    isDestroyed: () => state.destroyed,
    getMediaSourceId: () => 'window:99:0',
    hide: () => {
      state.hidden++;
    },
    show: () => {
      state.shown++;
    },
    focus: () => {
      state.focused++;
    },
  };
  const cancel = () => {
    state.cancelled++;
    flow.dispose();
  };
  const flow = new NativeShareFlow(window as any, { selector: selector as any, choice }, () => state.live, cancel);
  return { flow, choice, pending, identity, calls, state, electron, source };
}

test('native choice hides the catalog and requires final confirmation before any capture handle exists', async () => {
  const f = flowFixture();
  try {
    const pending = f.flow.choose();
    assert.equal(f.flow.acceptsList, false);
    assert.equal(f.state.hidden, 1);
    assert.deepEqual(f.calls[0].excluded, [99]);
    f.pending.resolve(f.identity);
    const result = await pending;
    assert.equal(result.status, 'confirmation');
    if (result.status !== 'confirmation') throw Error('Expected candidate');
    assert.equal(f.choice.take(result.source.id), null);
    assert.equal(f.state.shown, 1);
    assert.equal(f.state.enumerated, 1);
    assert.equal(await f.flow.confirm(result.source.id), true);
    assert.equal(f.state.enumerated, 2);
    assert.equal(f.choice.take(result.source.id), f.source);
  } finally {
    f.flow.dispose();
  }
});

test('late native completion cannot reopen or focus a cancelled sheet', async () => {
  const f = flowFixture(),
    pending = f.flow.choose();
  f.flow.dispose();
  assert.equal(f.calls[0].signal.aborted, true);
  f.pending.resolve(f.identity);
  assert.deepEqual(await pending, { status: 'ended' });
  assert.equal(f.state.shown, 0);
  assert.equal(f.state.enumerated, 0);
});

test('native cancellation ends the whole attempt without returning a list or candidate', async () => {
  const f = flowFixture(),
    pending = f.flow.choose();
  f.pending.resolve({ status: 'cancelled' });
  assert.deepEqual(await pending, { status: 'ended' });
  assert.equal(f.state.cancelled, 1);
  assert.equal(f.state.shown, 0);
});

test('native unavailability keeps a still-authorized source list usable without automatic selection', async () => {
  const f = flowFixture(),
    pending = f.flow.choose();
  try {
    f.pending.resolve({ status: 'unavailable' });
    const result = await pending;
    assert.equal(result.status, 'list');
    assert.equal(f.flow.acceptsList, true);
    assert.equal(f.state.shown, 1);
    assert.equal(f.state.enumerated, 0);
    assert.equal(await f.flow.confirm('unknown'), false);
  } finally {
    f.flow.dispose();
  }
});

test('switching back to cards permanently retires a previously offered native candidate', async () => {
  const f = flowFixture(),
    pending = f.flow.choose();
  try {
    f.pending.resolve(f.identity);
    const result = await pending;
    if (result.status !== 'confirmation') throw Error('Expected candidate');
    assert.equal(f.flow.back().status, 'list');
    assert.equal(await f.flow.confirm(result.source.id), false);
    assert.equal(f.choice.take(result.source.id), null);
  } finally {
    f.flow.dispose();
  }
});

test('duplicate starts are refused and a list switch during native selection cancels the outstanding request', async () => {
  const f = flowFixture(),
    pending = f.flow.choose();
  assert.deepEqual(await f.flow.choose(), { status: 'ended' });
  assert.equal(f.calls.length, 1);
  assert.deepEqual(f.flow.back(), { status: 'ended' });
  f.pending.resolve(f.identity);
  assert.deepEqual(await pending, { status: 'ended' });
  assert.equal(f.state.shown, 0);
});

test('request invalidation during candidate lookup cannot show a confirmation or steal focus', async () => {
  const f = flowFixture(),
    enumeration = Promise.withResolvers<any[]>();
  f.electron.desktopCapturer.getSources = () => enumeration.promise;
  const pending = f.flow.choose();
  f.pending.resolve(f.identity);
  await new Promise(setImmediate);
  f.state.live = false;
  enumeration.resolve([f.source]);
  assert.deepEqual(await pending, { status: 'ended' });
  assert.equal(f.state.shown, 0);
  f.flow.dispose();
});
