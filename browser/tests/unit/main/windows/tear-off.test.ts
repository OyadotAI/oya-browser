/**
 * Unit tests for src/main/windows/tear-off.ts: a tab pulled off its strip rides in
 * a real window under the pointer, as in Chrome, and lands on the strip it is let
 * go over.
 */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { BrowserWindows } from '../../../../src/main/windows/index.ts';
import { DRAG_FRAME_MS } from '../../../../src/main/windows/constants.ts';
import type { Point } from 'electron';
import { mainCtx, FakeWindow } from '../../support/main-ctx.cjs';

/** How the window was asked to be built. */
interface WindowOptions {
  /** False for a window built hidden. */
  show?: boolean;
}

/** A fake native window with a screen position and a real close order. */
class Window extends FakeWindow {
  /** Where it is on screen. */
  position = { x: 0, y: 0 };
  /** Hidden until presented when built hidden. */
  shown: boolean;
  /** As Electron's. */
  constructor(options: WindowOptions = {}) {
    super();
    this.shown = options.show !== false;
  }
  /** As Electron's. */
  destroy() {
    this.destroyed = true;
    this.emit('closed');
  }
  /** As Electron's, with no prompt to refuse it. */
  close() {
    this.emit('close', { preventDefault() {} });
    this.destroy();
  }
  /** As Electron's. */
  focus() {
    this.emit('focus');
  }
  /** As Electron's. */
  getBounds() {
    return { ...this.getContentBounds(), ...this.position };
  }
  /** As Electron's. */
  getContentBounds() {
    return { x: this.position.x, y: this.position.y, width: 1280, height: 800 };
  }
  /** As Electron's. */
  setPosition(x: number, y: number) {
    this.position = { x, y };
  }
  /** Its opacity, which hides it while a strip shows its tab. */
  opacity = 1;
  /** As Electron's. */
  setOpacity(value: number) {
    this.opacity = value;
  }
  /** As Electron's. */
  isVisible() {
    return !this.destroyed && this.shown;
  }
}

/** The real window graph with a pointer the test moves. */
function setup(t: any) {
  mock.timers.enable({ apis: ['setInterval', 'Date'] });
  const root: any = mainCtx();
  const pointer = { x: 100, y: 20 };
  root.electron.BrowserWindow = Window;
  root.electron.screen = {
    getCursorScreenPoint: () => ({ ...pointer }),
    getDisplayNearestPoint: () => ({ workArea: { x: -1600, y: 0, width: 4800, height: 1600 } }),
  };
  root.windows = new BrowserWindows(root);
  const primary = root.windows.current;
  primary.shell.create();
  primary.shell.browsingMode = true;
  t.after(() => {
    mock.timers.reset();
    root.callGuard.commit();
    root.windows.each((scope: any) => scope.shell.window.close());
  });
  return { root, primary, pointer };
}

/** Moves the pointer and lets one follow frame run. */
function moveTo(pointer: Point, x: number, y: number): void {
  Object.assign(pointer, { x, y });
  mock.timers.tick(DRAG_FRAME_MS);
}

it('carries a one-tab window itself, keeping the grip, and merges it into the strip it is let go over', async (t) => {
  const { root, primary, pointer } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  const other = root.windows.create();
  other.tabs.createTab('https://two.test/');
  other.shell.window.setPosition(2000, 500);
  root.windows.present(other);
  await root.windows.drag.begin(primary, id);
  moveTo(pointer, 600, 320);
  assert.deepEqual(primary.shell.window.position, { x: 500, y: 300 }, 'the window moved with the pointer');
  moveTo(pointer, 2100, 510);
  await root.windows.drag.drop();
  assert.equal(root.windows.owner(id), other);
  assert.equal(primary.shell.window.isDestroyed(), true, 'the emptied window closes');
});

it('tears a tab out of a busy window into a new one that follows the pointer and stays where it is let go', async (t) => {
  const { root, primary, pointer } = setup(t);
  primary.tabs.createTab('https://stay.test/');
  const id = primary.tabs.createTab('https://move.test/');
  pointer.y = 200;
  await root.windows.drag.begin(primary, id);
  const carrier = root.windows.owner(id);
  assert.notEqual(carrier, primary);
  const start = { ...carrier.shell.window.position };
  moveTo(pointer, 400, 500);
  assert.deepEqual(carrier.shell.window.position, { x: start.x + 300, y: start.y + 300 });
  await root.windows.drag.drop();
  assert.equal(root.windows.owner(id), carrier);
  assert.equal(primary.tabs.list.length, 1);
});

it('puts a torn-off tab back when it is let go over the strip it came from', async (t) => {
  const { root, primary, pointer } = setup(t);
  primary.tabs.createTab('https://stay.test/');
  const id = primary.tabs.createTab('https://move.test/');
  pointer.y = 200;
  await root.windows.drag.begin(primary, id);
  moveTo(pointer, 300, 10);
  await root.windows.drag.drop();
  assert.equal(root.windows.owner(id), primary);
  assert.equal(primary.tabs.list.length, 2);
});

it('stops carrying the window once an agent claims the tab', async (t) => {
  const { root, primary, pointer } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  root.control.snapshot = () => ({ connected: true, mode: 'agent', interactive: false, localClients: 0 });
  await root.windows.drag.begin(primary, id);
  root.agentTab.claim(id);
  moveTo(pointer, 900, 900);
  moveTo(pointer, 950, 950);
  assert.deepEqual(primary.shell.window.position, { x: 0, y: 0 });
  assert.equal(root.windows.drag.carrying(id), false);
});

it('refuses to tear off a tab an agent drives', (t) => {
  const { root, primary } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  root.control.snapshot = () => ({ connected: true, mode: 'agent', interactive: false, localClients: 0 });
  root.agentTab.claim(id);
  assert.throws(() => root.windows.drag.begin(primary, id), /Switch to Human Mode/);
});

it('hides the carried window over another strip, which shows the tab in a gap, and shows it again off the strip', async (t) => {
  const { root, primary, pointer } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  const other = root.windows.create();
  other.tabs.createTab('https://two.test/');
  other.shell.window.setPosition(2000, 500);
  root.windows.present(other);
  await root.windows.drag.begin(primary, id);
  moveTo(pointer, 2150, 510);
  assert.equal(primary.shell.window.opacity, 0);
  assert.deepEqual(other.shell.window.webContents.sentOn('tab-incoming').at(-1), {
    id,
    title: primary.tabs.find(id).title,
    favicon: null,
    home: false,
    x: 150,
  });
  moveTo(pointer, 600, 900);
  assert.equal(primary.shell.window.opacity, 1);
  assert.equal(other.shell.window.webContents.sentOn('tab-incoming').at(-1), null);
});

it('lets the strip place a tab let go over its gap, without closing the gap first', async (t) => {
  const { root, primary, pointer } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  const other = root.windows.create();
  other.tabs.createTab('https://two.test/');
  other.shell.window.setPosition(2000, 500);
  root.windows.present(other);
  await root.windows.drag.begin(primary, id);
  moveTo(pointer, 2150, 510);
  await root.windows.drag.drop();
  assert.equal(root.windows.owner(id), other);
  assert.notEqual(other.shell.window.webContents.sentOn('tab-incoming').at(-1), null);
});

it('closes the gap and shows the window again when the move onto the strip is refused', async (t) => {
  const { root, primary, pointer } = setup(t);
  const id = primary.tabs.createTab('https://one.test/');
  const other = root.windows.create();
  other.tabs.createTab('https://two.test/');
  other.shell.window.setPosition(2000, 500);
  root.windows.present(other);
  await root.windows.drag.begin(primary, id);
  moveTo(pointer, 2150, 510);
  other.overlays.names.add('address');
  await assert.rejects(root.windows.drag.drop(), /Close the open menu/);
  assert.equal(other.shell.window.webContents.sentOn('tab-incoming').at(-1), null);
  assert.equal(primary.shell.window.opacity, 1);
  other.overlays.names.clear();
});
