/** History capabilities cannot cross pages, connections, policy changes or native snapshot changes. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeNavigationHistory } from '../../../../src/main/native/navigation-history.ts';
/** Mutable native seam exercises traversal without a renderer or debugging backend. */
function fixture() {
  const entries = [
    { url: 'https://a.test', title: 'A', pageState: 'private-state' },
    { url: 'https://b.test', title: 'B' },
  ];
  let index = 1,
    destroyed = false;
  const calls: number[] = [];
  const page = {
    webContents: {
      isDestroyed: () => destroyed,
      navigationHistory: {
        getAllEntries: () => entries,
        getActiveIndex: () => index,
        goToIndex: (value: number) => calls.push(value),
      },
    },
  } as any;
  return {
    page,
    entries,
    calls,
    setIndex: (value: number) => {
      index = value;
    },
    destroy: () => {
      destroyed = true;
    },
  };
}
test('history exports real positions without native page state and consumes a traversal capability once', () => {
  const f = fixture(),
    history = new NativeNavigationHistory();
  const result = history.read(f.page) as any;
  assert.equal(result.currentIndex, 1);
  assert.deepEqual(result.entries[0], { index: 0, url: 'https://a.test', title: 'A' });
  assert.ok(!JSON.stringify(result).includes('private-state'));
  const params = { snapshot: result.snapshot, index: 0 };
  history.navigate(f.page, params, () => true);
  assert.deepEqual(f.calls, [0]);
  assert.throws(() => history.navigate(f.page, params, () => true), /snapshot/);
});
test('history tokens cannot cross connections or tabs and newer reads revoke old tokens', () => {
  const f = fixture(),
    history = new NativeNavigationHistory();
  const result = history.read(f.page) as any,
    params = { snapshot: result.snapshot, index: 0 };
  assert.throws(() => new NativeNavigationHistory().navigate(f.page, params, () => true), /snapshot/);
  assert.throws(() => history.navigate(fixture().page, params, () => true), /snapshot/);
  history.read(f.page);
  assert.throws(() => history.navigate(f.page, params, () => true), /snapshot/);
  assert.deepEqual(f.calls, []);
});
test('changed native state or current position rejects a snapshot even when URLs match', () => {
  const f = fixture(),
    history = new NativeNavigationHistory();
  let result = history.read(f.page) as any;
  f.entries[0].pageState = 'replaced-document';
  assert.throws(() => history.navigate(f.page, { snapshot: result.snapshot, index: 0 }, () => true), /changed/);
  result = history.read(f.page) as any;
  f.setIndex(0);
  assert.throws(() => history.navigate(f.page, { snapshot: result.snapshot, index: 0 }, () => true), /changed/);
});
test('invalid destinations and policy denial never reach native traversal', () => {
  const f = fixture(),
    history = new NativeNavigationHistory(),
    result = history.read(f.page) as any;
  for (const index of [-1, 2, 0.5, '0', NaN])
    assert.throws(() => history.navigate(f.page, { snapshot: result.snapshot, index }, () => true), /index/);
  assert.throws(() => history.navigate(f.page, { snapshot: result.snapshot, index: 0 }, () => false), /authorized/);
  assert.deepEqual(f.calls, []);
  f.destroy();
  assert.throws(() => history.read(f.page), /destroyed/);
});
test('oversized native history fails explicitly rather than truncating entry indexes', () => {
  const f = fixture(),
    history = new NativeNavigationHistory();
  f.entries[0].pageState = 'x'.repeat(1024 * 1024);
  assert.throws(() => history.read(f.page), /limit/);
  f.entries[0].pageState = '';
  while (f.entries.length <= 500) f.entries.push({ url: 'https://a.test', title: '' });
  assert.throws(() => history.read(f.page), /limit/);
});
