/**
 * Unit tests for src/main/mirror/device.ts: the device probe is plain
 * JavaScript that runs in a page and reports the real machine in the server's
 * profile shape, and the capture runs it in a hidden window it then destroys.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { deviceScript, captureDevice } from '../../../../src/main/mirror/device.ts';

/** A page's globals, as the hidden window would offer them, with a GPU that names itself. */
function fakePage(languages = ['en-GB']) {
  const dbg = { UNMASKED_VENDOR_WEBGL: 1, UNMASKED_RENDERER_WEBGL: 2 };
  const gl = { getExtension: () => dbg, getParameter: (p: number) => (p === 1 ? 'Apple' : 'Apple M2') };
  return {
    navigator: { platform: 'MacIntel', hardwareConcurrency: 8, maxTouchPoints: 0, languages, language: languages[0] },
    screen: { width: 1512, height: 982, availWidth: 1512, availHeight: 944, colorDepth: 30, pixelDepth: 30 },
    document: { createElement: () => ({ getContext: () => gl }) },
    window: { devicePixelRatio: 2 },
    Intl,
  };
}

/** Runs the probe in a fresh page context; the result crosses back as JSON, as executeJavaScript returns it. */
const probe = (languages?: string[]) =>
  JSON.parse(JSON.stringify(vm.runInNewContext(deviceScript(), fakePage(languages))));

describe('deviceScript', () => {
  it('runs as plain JavaScript in a page and reports the real GPU, unmasked', () => {
    const device = probe();
    assert.deepEqual(device.webgl, {
      vendor: 'Apple',
      renderer: 'Apple M2',
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M2',
    });
    assert.equal(device.screen.devicePixelRatio, 2);
    assert.equal(device.navigator.deviceMemory, 8, 'a browser that hides its memory is given a common value');
    assert.deepEqual([device.canvas, device.webglChrome, device.proxy], [{ noiseSeed: null }, true, null]);
  });

  it('adds the bare language after a lone regional one, as Chrome lists them', () => {
    const device = probe(['en-GB']);
    assert.deepEqual(device.navigator.languages, ['en-GB', 'en']);
  });
});

describe('captureDevice', () => {
  it('runs the probe in a hidden blank window and destroys it after', async () => {
    const calls: string[] = [];
    /** A hidden window that records what it was asked to do. */
    class FakeWindow {
      /** Records the window's options. */
      constructor(options: any) {
        calls.push(`new show=${options.show}`);
      }
      /** Records the page it loaded. */
      loadURL = async (url: string) => void calls.push(`load ${url}`);
      /** Runs the probe. */
      webContents = { executeJavaScript: async (code: string) => (calls.push('run'), code === deviceScript()) };
      /** Records its end. */
      destroy = () => void calls.push('destroy');
    }
    const result = await captureDevice({ BrowserWindow: FakeWindow } as any);
    assert.equal(result, true);
    assert.deepEqual(calls, ['new show=false', 'load about:blank', 'run', 'destroy']);
  });
});
