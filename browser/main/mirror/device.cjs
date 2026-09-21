/**
 * The real machine's device, read on the machine itself.
 *
 * A launched headless Chrome often falls back to SwiftShader and would report
 * the wrong GPU, so the hardware (GPU, screen, cores, timezone) is read from a
 * hidden Electron window, which runs with the real GPU. Only the browser
 * version and the cookies come from the user's actual browser (read.cjs).
 */

/** The in-page script that reports the real device in the server's profile shape. */
function deviceScript() {
  return `(${gather.toString()})()`;
}

/* eslint-disable no-undef, no-magic-numbers, max-lines-per-function -- runs inside the page, not Node. */
/** Runs in the page: gathers navigator, screen, WebGL and locale into a profile object. */
function gather() {
  const n = navigator;
  const s = screen;
  const gl = document.createElement('canvas').getContext('webgl');
  const dbg = gl && gl.getExtension('WEBGL_debug_renderer_info');
  const glp = (p) => {
    try {
      return gl.getParameter(p);
    } catch {
      return '';
    }
  };
  return {
    navigator: {
      platform: n.platform,
      hardwareConcurrency: n.hardwareConcurrency,
      deviceMemory: n.deviceMemory || 8,
      maxTouchPoints: n.maxTouchPoints || 0,
      languages:
        n.languages.length === 1 && n.language.includes('-')
          ? [n.language, n.language.split('-')[0]]
          : [...n.languages],
      vendor: n.vendor,
    },
    screen: {
      width: s.width,
      height: s.height,
      availWidth: s.availWidth,
      availHeight: s.availHeight,
      colorDepth: s.colorDepth,
      pixelDepth: s.pixelDepth,
      devicePixelRatio: window.devicePixelRatio,
    },
    // webglChrome profiles keep what Chrome answers UNMASKED_* with in vendor/renderer. The masked
    // pair ("WebKit", "WebKit WebGL") was stored here once, and pages saw that as the GPU.
    webgl:
      gl && dbg
        ? {
            vendor: glp(dbg.UNMASKED_VENDOR_WEBGL),
            renderer: glp(dbg.UNMASKED_RENDERER_WEBGL),
            unmaskedVendor: glp(dbg.UNMASKED_VENDOR_WEBGL),
            unmaskedRenderer: glp(dbg.UNMASKED_RENDERER_WEBGL),
          }
        : {},
    // Null noise seeds tell the injection to leave real rendering untouched.
    canvas: { noiseSeed: null },
    audio: { noiseSeed: null },
    rects: { noiseSeed: null },
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    locale: n.language,
    webglChrome: true,
    proxy: null,
  };
}
/* eslint-enable no-undef, no-magic-numbers, max-lines-per-function */

/** Reads the real device in a throwaway hidden window, with no persona partition or injection. */
async function captureDevice(electron) {
  const win = new electron.BrowserWindow({ show: false });
  try {
    await win.loadURL('about:blank');
    return await win.webContents.executeJavaScript(deviceScript());
  } finally {
    win.destroy();
  }
}

module.exports = { captureDevice, deviceScript };
