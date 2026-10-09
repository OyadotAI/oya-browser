/** Native shell motion policy never obtains a debugger or changes website media settings. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { holdStill, inContainer, stillWhileAway } from '../../../../src/main/shell/hold-still.ts';
/** A native-only page whose forbidden debugger accessor fails immediately. */
function fixture() {
  const events = new EventEmitter();
  const scripts: string[] = [];
  let destroyed = false;
  const webContents = Object.assign(new EventEmitter(), {
    isDestroyed: () => destroyed,
    executeJavaScript: async (code: string) => {
      scripts.push(code);
    },
  });
  Object.defineProperty(webContents, 'debugger', {
    get() {
      assert.fail('No internal CDP');
    },
  });
  const view = { webContents, on: (event: string, listener: () => void) => events.on(event, listener) };
  return {
    view,
    events,
    scripts,
    destroy: () => {
      destroyed = true;
    },
  };
}
describe('native shell motion', () => {
  it('holds containers still and reapplies after reload', async () => {
    const f = fixture();
    await holdStill(f.view, true);
    f.view.webContents.emit('dom-ready');
    assert.deepEqual(f.scripts, Array(2).fill("document.documentElement.dataset.oyaStill = 'true';"));
  });
  it('leaves non-container pages alone', async () => {
    const f = fixture();
    await holdStill(f.view, false);
    f.view.webContents.emit('dom-ready');
    assert.deepEqual(f.scripts, []);
  });
  it('does not execute on destroyed pages', async () => {
    const f = fixture();
    f.destroy();
    await holdStill(f.view, true);
    assert.deepEqual(f.scripts, []);
  });
  it('recovers after a document replacement rejects execution', async () => {
    const f = fixture();
    f.view.webContents.executeJavaScript = async () => {
      throw new Error('document replaced');
    };
    await holdStill(f.view, true);
  });
  it('remembers the latest window focus state across reloads', () => {
    const f = fixture();
    stillWhileAway(f.view);
    f.events.emit('blur');
    f.view.webContents.emit('dom-ready');
    f.events.emit('focus');
    f.view.webContents.emit('dom-ready');
    assert.deepEqual(
      f.scripts,
      [true, true, false, false].map((still) => `document.documentElement.dataset.oyaStill = '${still}';`),
    );
  });
  it('recognizes only the explicit container environment', () => {
    assert.equal(inContainer({ OYA_DOCKER: 'true' }), true);
    assert.equal(inContainer({}), false);
  });
});
