/** Native window choices match exact identity and re-enumerate only after explicit confirmation. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeWindowChoice, nativeWindowId } from '../../../../src/main/screen-share/native-window-choice.ts';

/** OS identity and native source contents are independent so title/PID substitution can be tested. */
function choiceFixture() {
  const state = { current: true, window: true, permission: 'granted', calls: 0 };
  const source = (id = 'window:43:0') => ({ id, name: 'Same title', thumbnail: { isEmpty: () => true } });
  let sources = [source()];
  const electron = {
    systemPreferences: { getMediaAccessStatus: () => state.permission },
    desktopCapturer: {
      getSources: async () => {
        state.calls++;
        return sources;
      },
    },
  };
  const identity = { windowId: 43, processId: 123, processStarted: 456 };
  const selector = { current: (value) => state.window && value === identity };
  const choice = new NativeWindowChoice(electron as any, selector as any, () => state.current);
  return {
    choice,
    state,
    source,
    identity,
    electron,
    set: (next) => {
      sources = next;
    },
  };
}

test('native source ids require the exact documented anchored window format', () => {
  assert.equal(nativeWindowId('window:43:0'), 43);
  for (const value of [
    'screen:43:0',
    'window:43:0extra',
    'window:0:0',
    'window:-43:0',
    'window:1e3:0',
    'window:9007199254740993:0',
  ])
    assert.equal(nativeWindowId(value), null);
});

test('an offered candidate cannot supply capture before a fresh explicit confirmation', async () => {
  const f = choiceFixture();
  const offer = await f.choice.offer(f.identity);
  assert.ok(offer);
  assert.notEqual(offer.id, 'window:43:0');
  assert.equal(f.choice.take(offer.id), null);
  const fresh = f.source();
  f.set([fresh]);
  assert.equal(await f.choice.resolve(offer.id), fresh);
  assert.equal(f.choice.take(offer.id), fresh);
  assert.equal(f.choice.take(offer.id), null);
  assert.equal(f.state.calls, 2);
});

test('a vanished window cannot be replaced by another window with the same title', async () => {
  const f = choiceFixture(),
    offer = await f.choice.offer(f.identity);
  f.set([f.source('window:44:0')]);
  assert.equal(await f.choice.resolve(offer!.id), null);
  assert.equal(f.choice.take(offer!.id), null);
});

test('OS denial, owner invalidation and native process replacement all refuse the final source', async () => {
  for (const invalidate of [
    (f) => {
      f.state.current = false;
    },
    (f) => {
      f.state.window = false;
    },
    (f) => {
      f.state.permission = 'denied';
    },
  ]) {
    const f = choiceFixture(),
      offer = await f.choice.offer(f.identity);
    await f.choice.resolve(offer!.id);
    invalidate(f);
    assert.equal(f.choice.take(offer!.id), null);
  }
});

test('duplicate native ids are ambiguous and never resolved by title or list order', async () => {
  const f = choiceFixture();
  f.set([f.source(), f.source()]);
  assert.equal(await f.choice.offer(f.identity), null);
});

test('retiring a candidate during enumeration prevents its late offer or confirmation from reviving', async () => {
  const f = choiceFixture(),
    pending = Promise.withResolvers<any[]>();
  f.electron.desktopCapturer.getSources = () => pending.promise;
  const offered = f.choice.offer(f.identity);
  f.choice.retire();
  pending.resolve([f.source()]);
  assert.equal(await offered, null);
  f.electron.desktopCapturer.getSources = async () => [f.source()];
  const offer = await f.choice.offer(f.identity),
    later = Promise.withResolvers<any[]>();
  f.electron.desktopCapturer.getSources = () => later.promise;
  const confirming = f.choice.resolve(offer!.id);
  f.choice.retire();
  later.resolve([f.source()]);
  assert.equal(await confirming, null);
  assert.equal(f.choice.take(offer!.id), null);
});
