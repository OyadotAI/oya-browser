/** Preview staging preserves native owners, tab lifetimes and layout while waking detached renderers. */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { TabPreviews } from '../../../../src/main/screen-share/tab-previews.ts';
import { PICKER } from '../../../../src/main/screen-share/constants.ts';

/** A decoded native image stands in for fixture pixels, never real desktop content. */
const image = { isEmpty: () => false, resize: () => image, toDataURL: () => 'data:image/png;base64,pixels' };

/** Native parent ownership can change while a capture promise is pending. */
function fixture(platform = 'darwin') {
  const state = { owner: null as any, destroyed: false, bounds: { x: 0, y: 0, width: 0, height: 0 } };
  const contents = { isDestroyed: () => state.destroyed, capturePage: async () => image };
  const view = {
    webContents: contents,
    getBounds: () => state.bounds,
    setBounds: (bounds) => {
      state.bounds = bounds;
    },
  };
  const windows: Window[] = [];
  /** Minimal window seam preserves ownership and fails if cleanup destroys a borrowed renderer. */
  class Window {
    /** Observe native privacy and focus creation options. */
    options;
    /** Only the temporary parent can be closed by this service. */
    destroyed = false;
    /** The surface must not intercept pointer events. */
    ignoresMouse = false;
    /** Track an invisible surface without constructing an OS window. */
    constructor(options) {
      this.options = options;
      windows.push(this);
    }
    /** Match Electron's owner lookup for the captured view. */
    static fromWebContents() {
      return state.owner;
    }
    /** Report native lifecycle state. */
    isDestroyed() {
      return this.destroyed;
    }
    /** Record pointer isolation. */
    setIgnoreMouseEvents(value) {
      this.ignoresMouse = value;
    }
    /** A native identifier lets enumeration exclude this internal window. */
    getMediaSourceId() {
      return 'window:preview:0';
    }
    /** Attach only the requested tab. */
    addBrowserView() {
      state.owner = this;
    }
    /** Cleanup must release the renderer before destroying its parent. */
    removeBrowserView() {
      state.owner = null;
    }
    /** Display is deliberately inactive; focus cannot be requested. */
    showInactive() {}
    /** Destruction must not take the person's tab with it. */
    destroy() {
      assert.notEqual(state.owner, this);
      this.destroyed = true;
    }
  }
  const previews = new TabPreviews({ electron: { BrowserWindow: Window } } as any, platform);
  const tab = { view } as any;
  return { previews, tab, state, windows, contents };
}

it('renders a never-shown tab with a usable viewport on an invisible noninteractive parent', async () => {
  const f = fixture();
  assert.equal(await f.previews.capture(f.tab), image.toDataURL());
  assert.equal(f.windows.length, 1);
  assert.equal(f.windows[0].options.opacity, 0);
  assert.equal(f.windows[0].options.focusable, false);
  assert.equal(f.windows[0].ignoresMouse, true);
  assert.equal(f.state.bounds.width, PICKER.PREVIEW_WIDTH);
  assert.equal(f.state.owner, null);
  f.previews.dispose();
  assert.equal(f.windows[0].destroyed, true);
  assert.equal(f.previews.sourceId, 'window:preview:0');
});

it('captures mounted tabs in place and never creates a staging surface on unsupported platforms', async () => {
  for (const mode of ['attached', 'linux']) {
    const f = fixture(mode === 'linux' ? 'linux' : 'darwin');
    const owner = mode === 'attached' ? {} : null;
    f.state.owner = owner;
    assert.equal(await f.previews.capture(f.tab), image.toDataURL());
    assert.equal(f.windows.length, 0);
    assert.equal(f.state.owner, owner);
    f.previews.dispose();
  }
});

it('restores an existing background viewport after capture', async () => {
  const f = fixture();
  const bounds = { x: 10, y: 80, width: 1100, height: 720 };
  f.state.bounds = bounds;
  await f.previews.capture(f.tab);
  assert.deepEqual(f.state.bounds, bounds);
  f.previews.dispose();
});

it('never undoes a tab activation or transfer while a thumbnail is pending', async () => {
  const f = fixture();
  const pending = Promise.withResolvers<any>();
  f.contents.capturePage = () => pending.promise;
  const capture = f.previews.capture(f.tab);
  const owner = {};
  const bounds = { x: 30, y: 60, width: 1400, height: 900 };
  f.state.owner = owner;
  f.state.bounds = bounds;
  pending.resolve(image);
  await capture;
  f.previews.dispose();
  assert.equal(f.state.owner, owner);
  assert.deepEqual(f.state.bounds, bounds);
});

it('waits for first paint after an initially empty native capture', async () => {
  const f = fixture();
  let calls = 0;
  f.contents.capturePage = async () => (++calls === 1 ? { ...image, isEmpty: () => true } : image);
  assert.equal(await f.previews.capture(f.tab), image.toDataURL());
  assert.equal(calls, 2);
  f.previews.dispose();
});

it('a stalled capture times out and releases the temporary parent without closing its tab', async () => {
  mock.timers.enable({ apis: ['setTimeout', 'Date'] });
  const f = fixture();
  try {
    f.contents.capturePage = () => new Promise(() => {});
    const capture = f.previews.capture(f.tab);
    mock.timers.tick(PICKER.PREVIEW_TIMEOUT_MS);
    assert.equal(await capture, '');
    assert.equal(f.state.owner, null);
    f.previews.dispose();
    assert.equal(f.windows[0].destroyed, true);
  } finally {
    mock.timers.reset();
  }
});
