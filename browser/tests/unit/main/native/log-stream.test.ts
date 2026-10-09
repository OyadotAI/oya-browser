/** Native logs remain scoped to their page and are suppressed as soon as human ownership applies. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { watchNativeLog } from '../../../../src/main/native/index.ts';
test('native console events honor ownership and unsubscribe without altering other observers', () => {
  const contents = Object.assign(new EventEmitter(), { isDestroyed: () => false });
  const events: any[] = [];
  let allowed = true;
  const stop = watchNativeLog(
    { webContents: contents } as any,
    () => allowed,
    (...args) => events.push(args),
  );
  contents.emit('console-message', { level: 'warning', message: 'test', lineNumber: 2 });
  assert.equal(events[0][0], 'Log.entryAdded');
  assert.equal(events[0][1].entry.level, 'warning');
  assert.equal(events[0][1].entry.lineNumber, 1);
  allowed = false;
  contents.emit('console-message', { level: 'error', message: 'private human activity' });
  assert.equal(events.length, 1);
  stop();
  assert.equal(contents.listenerCount('console-message'), 0);
});
