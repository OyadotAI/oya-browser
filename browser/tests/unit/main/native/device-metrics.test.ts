/** Native device metrics are validated, scoped to one connection, and restored on disconnect. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { NativeDeviceMetrics, metricParameters } from '../../../../src/main/native/index.ts';
/** Browser-owned surface fake records native calls only. */
function fixture() {
  const applied: any[] = [];
  let restored = 0;
  const contents = Object.assign(new EventEmitter(), {
    isDestroyed: () => false,
    enableDeviceEmulation: (value: unknown) => {
      applied.push(value);
    },
    disableDeviceEmulation: () => {
      restored++;
    },
  });
  return { page: { webContents: contents } as any, contents, applied, restored: () => restored };
}
const metrics = { width: 480, height: 640, mobile: true, deviceScaleFactor: 2 };
test('valid metrics map directly to native viewport emulation', () => {
  assert.deepEqual(metricParameters(metrics), {
    screenPosition: 'mobile',
    screenSize: { width: 480, height: 640 },
    viewSize: { width: 480, height: 640 },
    viewPosition: { x: 0, y: 0 },
    deviceScaleFactor: 2,
    scale: 1,
  });
});
test('invalid metrics fail before altering native state', () => {
  const f = fixture(),
    owner = new NativeDeviceMetrics();
  for (const value of [
    { width: -1 },
    { width: '480' },
    { height: 10001 },
    { mobile: 'true' },
    { deviceScaleFactor: NaN },
    { deviceScaleFactor: 5 },
  ])
    assert.throws(() => owner.set(f.page, { ...metrics, ...value }));
  assert.equal(f.applied.length, 0);
});
test('a foreign socket cannot overwrite or clear another socket’s viewport', () => {
  const f = fixture(),
    first = new NativeDeviceMetrics(),
    second = new NativeDeviceMetrics();
  first.set(f.page, metrics);
  assert.throws(() => second.set(f.page, metrics), /another connection/);
  assert.throws(() => second.clear(f.page), /another connection/);
  first.dispose();
  assert.equal(f.restored(), 1);
  second.set(f.page, metrics);
  second.clear(f.page);
  second.dispose();
  assert.equal(f.restored(), 2);
});
test('destroyed pages release listeners and connection ownership without native calls', () => {
  const f = fixture(),
    owner = new NativeDeviceMetrics();
  owner.set(f.page, metrics);
  f.contents.emit('destroyed');
  owner.dispose();
  assert.equal(f.restored(), 0);
  assert.equal(f.contents.listenerCount('destroyed'), 0);
});
