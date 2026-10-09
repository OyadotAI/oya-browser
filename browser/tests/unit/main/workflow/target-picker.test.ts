/** Native picker behavior with debugger access forbidden and explicit lifecycle events. */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { pickTarget } from '../../../../src/main/workflow/target-picker.ts';
/** Native surface seam records only isolated script execution. */
function viewWith(element = { type: 'button', role: 'button', text: 'Save' }, error?: Error) {
  const scripts: string[] = [];
  const webContents = Object.assign(new EventEmitter(), {
    isDestroyed: () => false,
    getZoomFactor: () => 1,
    executeJavaScriptInIsolatedWorld: async (
      _world: number,
      entries: { /** Trusted isolated source captured by the fixture. */ code: string }[],
    ) => {
      const code = entries[0].code;
      scripts.push(code);
      if (error && code !== 'globalThis.__oyaPicker?.stop();') throw error;
      return code.includes('?.pick(') ? element : undefined;
    },
  });
  Object.defineProperty(webContents, 'debugger', {
    get() {
      assert.fail('Internal CDP is forbidden');
    },
  });
  return { webContents, scripts };
}
/** Flush native promise continuations without a real timeout. */
const flush = () => new Promise((resolve) => setImmediate(resolve));
/** Native events carry a cancelable dispatch gate before website event delivery. */
function mouse(view, type = 'mouseUp', button = 'left', x = 20, y = 30) {
  let prevented = false;
  view.webContents.emit(
    'before-mouse-event',
    {
      preventDefault() {
        prevented = true;
      },
    },
    { type, button, x, y },
  );
  return prevented;
}
describe('native pickTarget', () => {
  afterEach(() => mock.timers.reset());
  it('selects without forwarding the click or attaching a debugger', async () => {
    const view = viewWith();
    const pick = pickTarget(view);
    assert.equal(mouse(view, 'mouseDown'), true);
    assert.equal(mouse(view), true);
    assert.deepEqual((await pick)[0], { kind: 'role', role: 'button', value: 'Save' });
    await flush();
    assert.equal(view.webContents.listenerCount('before-mouse-event'), 0);
    assert.equal(view.scripts.at(-1), 'globalThis.__oyaPicker?.stop();');
  });
  it('maps zoomed native input to CSS hit-test coordinates', async () => {
    const view = viewWith();
    view.webContents.getZoomFactor = () => 2;
    const pick = pickTarget(view);
    mouse(view, 'mouseUp', 'left', 40, 60);
    await pick;
    assert.ok(view.scripts.includes('globalThis.__oyaPicker?.pick(20,30)'));
  });
  it('cancels with Escape and removes every listener', async () => {
    const view = viewWith();
    const pick = pickTarget(view);
    let prevented = false;
    view.webContents.emit(
      'before-input-event',
      {
        preventDefault() {
          prevented = true;
        },
      },
      { key: 'Escape' },
    );
    await assert.rejects(pick, /canceled/);
    assert.equal(prevented, true);
    assert.equal(view.webContents.listenerCount('destroyed'), 0);
    assert.equal(view.webContents.listenerCount('did-start-loading'), 0);
  });
  it('refuses frames rather than creating an incorrect top-level locator', async () => {
    const view = viewWith({ unsupported: true } as never);
    const pick = pickTarget(view);
    mouse(view);
    await assert.rejects(pick, /top-level targets/);
  });
  it('refuses elements without stable locators', async () => {
    const view = viewWith({ type: 'div' } as never);
    const pick = pickTarget(view);
    mouse(view);
    await assert.rejects(pick, /no stable target/);
  });
  for (const event of ['destroyed', 'did-start-loading'])
    it(`cancels on ${event}`, async () => {
      const view = viewWith();
      const pick = pickTarget(view);
      view.webContents.emit(event);
      await assert.rejects(pick, /closed|navigated/);
    });
  it('propagates initialization failure', async () => {
    await assert.rejects(pickTarget(viewWith(undefined, new Error('No native world'))), /No native world/);
  });
  it('refuses overlapping picks and permits a new pick after cleanup', async () => {
    const view = viewWith();
    const first = pickTarget(view);
    await assert.rejects(pickTarget(view), /already active/);
    mouse(view);
    await first;
    await flush();
    const second = pickTarget(view);
    mouse(view);
    await second;
  });
  it('times out and settles only once', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const view = viewWith();
    const pick = pickTarget(view);
    mock.timers.tick(60000);
    await assert.rejects(pick, /timed out/);
    await flush();
    assert.equal(view.scripts.filter((code) => code === 'globalThis.__oyaPicker?.stop();').length, 1);
  });
});
