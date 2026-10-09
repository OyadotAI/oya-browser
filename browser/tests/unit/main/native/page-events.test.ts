/** Page readiness is native, delivery-authorized, connection-local and never replayed. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { watchNativePage, nativePageCommand } from '../../../../src/main/native/page-events.ts';
test('native readiness gates each event and disposal removes only the caller listeners', () => {
  const contents = Object.assign(new EventEmitter(), { isDestroyed: () => false });
  let allowed = true;
  const events: any[] = [],
    other: any[] = [];
  const a = watchNativePage(
    { webContents: contents } as any,
    () => allowed,
    (...e) => events.push(e),
  );
  const b = watchNativePage(
    { webContents: contents } as any,
    () => true,
    (...e) => other.push(e),
  );
  contents.emit('dom-ready');
  assert.equal(events[0][0], 'Page.domContentEventFired');
  assert.equal(typeof events[0][1].timestamp, 'number');
  allowed = false;
  contents.emit('did-finish-load');
  assert.equal(events.length, 1);
  assert.equal(other.length, 2);
  a();
  a();
  contents.emit('dom-ready');
  assert.equal(other.length, 3);
  b();
  assert.equal(contents.listenerCount('dom-ready'), 0);
});
test('reload honors cache bypass and stopping never reloads a different page', () => {
  const calls: string[] = [];
  const page = {
    webContents: {
      isDestroyed: () => false,
      reload: () => calls.push('reload'),
      reloadIgnoringCache: () => calls.push('bypass'),
      stop: () => calls.push('stop'),
    },
  } as any;
  nativePageCommand(page, 'page:reload', {});
  nativePageCommand(page, 'page:reload', { ignoreCache: true });
  nativePageCommand(page, 'page:stop', {});
  assert.deepEqual(calls, ['reload', 'bypass', 'stop']);
  assert.throws(() => nativePageCommand(page, 'unsupported', {}), /Unsupported/);
  page.webContents.isDestroyed = () => true;
  assert.throws(() => nativePageCommand(page, 'page:reload', {}), /destroyed/);
});
