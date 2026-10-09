/** Input readiness depends on newly observed native main-document navigation, not subresources. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import type { WebContents } from 'electron';
import { actionNavigation } from '../../../../src/main/actions/action-navigation.ts';
import { NAVIGATION_START_MS, LOAD_TIMEOUT_MS } from '../../../../src/main/actions/constants.ts';

/** A native emitter whose background resource loading never settles. */
function page() {
  return {
    webContents: Object.assign(new EventEmitter(), {
      isDestroyed: () => false,
      isLoading: () => true,
    }) as unknown as WebContents,
  };
}
/** Let the async action return before advancing its navigation-start grace. */
async function grace(t) {
  await Promise.resolve();
  t.mock.timers.tick(NAVIGATION_START_MS);
  await Promise.resolve();
}
/** Emit a new main-document navigation with the native details shape. */
function start(p) {
  p.webContents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false });
}

it('unrelated background loading does not add a navigation deadline', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const p = page();
  const pending = actionNavigation(p, async () => {});
  await grace(t);
  assert.equal(await pending, false);
  assert.deepEqual(p.webContents.eventNames(), []);
});

it('child and same-document navigation never wait for main-document readiness', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const p = page();
  const pending = actionNavigation(p, async () => {
    p.webContents.emit('did-start-navigation', { isMainFrame: false });
    p.webContents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: true });
  });
  await grace(t);
  assert.equal(await pending, false);
});

it('new main navigation waits for DOM readiness and keeps unrelated listeners', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const p = page();
  const listener = () => {};
  p.webContents.on('dom-ready', listener);
  const pending = actionNavigation(p, async () => start(p));
  await grace(t);
  p.webContents.emit('dom-ready');
  assert.equal(await pending, true);
  assert.deepEqual(p.webContents.listeners('dom-ready'), [listener]);
});

it('readiness observed synchronously during input is retained', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const p = page();
  const pending = actionNavigation(p, async () => {
    start(p);
    p.webContents.emit('dom-ready');
  });
  await grace(t);
  assert.equal(await pending, true);
});

it('a new navigation timeout is explicit and cleans every listener', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const p = page();
  const pending = actionNavigation(p, async () => start(p));
  const rejected = assert.rejects(pending, /readiness timed out/);
  await grace(t);
  t.mock.timers.tick(LOAD_TIMEOUT_MS);
  await rejected;
  assert.deepEqual(p.webContents.eventNames(), []);
});

it('an unrelated old load failure does not fail an input action', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const p = page();
  const pending = actionNavigation(
    p,
    async () => p.webContents.emit('did-fail-load', {}, -1, '', '', true) && undefined,
  );
  await grace(t);
  assert.equal(await pending, false);
});

for (const event of ['destroyed', 'render-process-gone', 'did-fail-load']) {
  it(event + ' rejects a pending navigation and cleans listeners', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const p = page();
    const pending = actionNavigation(p, async () => start(p));
    const rejected = assert.rejects(pending, /failed|destroyed|lost/);
    await grace(t);
    p.webContents.emit(event, {}, -1, '', '', true);
    await rejected;
    assert.deepEqual(p.webContents.eventNames(), []);
  });
}

it('a dispatch exception cleans listeners without hiding the original error', async () => {
  const p = page();
  await assert.rejects(
    actionNavigation(p, async () => {
      throw Error('dispatch failed');
    }),
    /dispatch failed/,
  );
  assert.deepEqual(p.webContents.eventNames(), []);
});
