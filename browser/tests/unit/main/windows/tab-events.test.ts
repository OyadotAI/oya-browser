/** Native tab observers cannot interrupt browser lifecycle or keep receiving after unsubscribe. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { WindowTabEvents } from '../../../../src/main/windows/tab-events.ts';
test('tab changes notify subscribers once and unsubscribe is idempotent', () => {
  const events = new WindowTabEvents();
  let calls = 0;
  const stop = events.subscribe(() => {
    calls++;
  });
  events.changed();
  assert.equal(calls, 1);
  stop();
  stop();
  events.changed();
  assert.equal(calls, 1);
});
