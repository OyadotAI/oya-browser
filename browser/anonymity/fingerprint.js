/**
 * Fingerprint profile generation and injection script builder.
 * Generates coherent per-profile fingerprints and builds a JS string
 * for injection via Page.addScriptToEvaluateOnNewDocument.
 */

const crypto = require('crypto');

// ── Realistic GPU databases by platform ──

const GPU_DB = {
  Win32: [
    { vendor: 'Google Inc. (NVIDIA)', renderer: 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)', unmaskedVendor: 'NVIDIA Corporation', unmaskedRenderer: 'NVIDIA GeForce RTX 3060/PCIe/SSE2' },
    { vendor: 'Google Inc. (NVIDIA)', renderer: 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3070 Direct3D11 vs_5_0 ps_5_0, D3D11)', unmaskedVendor: 'NVIDIA Corporation', unmaskedRenderer: 'NVIDIA GeForce RTX 3070/PCIe/SSE2' },
    { vendor: 'Google Inc. (NVIDIA)', renderer: 'ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)', unmaskedVendor: 'NVIDIA Corporation', unmaskedRenderer: 'NVIDIA GeForce GTX 1660 SUPER/PCIe/SSE2' },
    { vendor: 'Google Inc. (AMD)', renderer: 'ANGLE (AMD, AMD Radeon RX 6700 XT Direct3D11 vs_5_0 ps_5_0, D3D11)', unmaskedVendor: 'ATI Technologies Inc.', unmaskedRenderer: 'AMD Radeon RX 6700 XT' },
    { vendor: 'Google Inc. (Intel)', renderer: 'ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)', unmaskedVendor: 'Intel Inc.', unmaskedRenderer: 'Intel(R) UHD Graphics 630' },
  ],
  MacIntel: [
    { vendor: 'Google Inc. (Apple)', renderer: 'ANGLE (Apple, Apple M1, OpenGL 4.1)', unmaskedVendor: 'Apple', unmaskedRenderer: 'Apple M1' },
    { vendor: 'Google Inc. (Apple)', renderer: 'ANGLE (Apple, Apple M1 Pro, OpenGL 4.1)', unmaskedVendor: 'Apple', unmaskedRenderer: 'Apple M1 Pro' },
    { vendor: 'Google Inc. (Apple)', renderer: 'ANGLE (Apple, Apple M2, OpenGL 4.1)', unmaskedVendor: 'Apple', unmaskedRenderer: 'Apple M2' },
    { vendor: 'Google Inc. (Apple)', renderer: 'ANGLE (Apple, Apple M3, OpenGL 4.1)', unmaskedVendor: 'Apple', unmaskedRenderer: 'Apple M3' },
    { vendor: 'Google Inc. (Apple)', renderer: 'ANGLE (Apple, Apple M2 Pro, OpenGL 4.1)', unmaskedVendor: 'Apple', unmaskedRenderer: 'Apple M2 Pro' },
  ],
  'Linux x86_64': [
    { vendor: 'Google Inc. (NVIDIA)', renderer: 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3080/PCIe/SSE2, OpenGL 4.5)', unmaskedVendor: 'NVIDIA Corporation', unmaskedRenderer: 'NVIDIA GeForce RTX 3080/PCIe/SSE2' },
    { vendor: 'Google Inc. (Intel)', renderer: 'ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)', unmaskedVendor: 'Intel', unmaskedRenderer: 'Mesa Intel(R) UHD Graphics 630 (CFL GT2)' },
    { vendor: 'Google Inc. (AMD)', renderer: 'ANGLE (AMD, AMD Radeon RX 580, OpenGL 4.6)', unmaskedVendor: 'ATI Technologies Inc.', unmaskedRenderer: 'AMD Radeon RX 580' },
  ],
};

const SCREEN_RESOLUTIONS = {
  Win32: [
    { width: 1920, height: 1080, dpr: 1 },
    { width: 2560, height: 1440, dpr: 1 },
    { width: 1366, height: 768, dpr: 1 },
    { width: 1680, height: 1050, dpr: 1 },
    { width: 3840, height: 2160, dpr: 1.5 },
  ],
  MacIntel: [
    { width: 1440, height: 900, dpr: 2 },
    { width: 1680, height: 1050, dpr: 2 },
    { width: 1920, height: 1080, dpr: 2 },
    { width: 2560, height: 1440, dpr: 2 },
    { width: 1280, height: 800, dpr: 2 },
  ],
  'Linux x86_64': [
    { width: 1920, height: 1080, dpr: 1 },
    { width: 2560, height: 1440, dpr: 1 },
    { width: 1366, height: 768, dpr: 1 },
  ],
};

const FONT_SETS = {
  Win32: ['Arial', 'Arial Black', 'Calibri', 'Cambria', 'Comic Sans MS', 'Consolas', 'Courier New', 'Georgia', 'Impact', 'Lucida Console', 'Segoe UI', 'Tahoma', 'Times New Roman', 'Trebuchet MS', 'Verdana'],
  MacIntel: ['Arial', 'Arial Black', 'Courier New', 'Georgia', 'Helvetica', 'Helvetica Neue', 'Impact', 'Lucida Grande', 'Menlo', 'Monaco', 'SF Pro', 'Times New Roman', 'Trebuchet MS', 'Verdana'],
  'Linux x86_64': ['Arial', 'Courier New', 'DejaVu Sans', 'DejaVu Serif', 'FreeMono', 'FreeSans', 'FreeSerif', 'Liberation Mono', 'Liberation Sans', 'Liberation Serif', 'Noto Sans', 'Times New Roman', 'Ubuntu', 'Verdana'],
};

const HARDWARE_CONCURRENCY = [4, 6, 8, 12, 16];
const DEVICE_MEMORY = [4, 8, 16];

const TIMEZONES = {
  Win32: ['America/New_York', 'America/Chicago', 'America/Denver', 'America/Los_Angeles', 'America/Phoenix', 'America/Detroit', 'America/Indianapolis'],
  MacIntel: ['America/New_York', 'America/Chicago', 'America/Denver', 'America/Los_Angeles', 'America/Phoenix', 'Pacific/Honolulu'],
  'Linux x86_64': ['America/New_York', 'America/Chicago', 'America/Los_Angeles', 'Europe/London', 'Europe/Berlin', 'Asia/Tokyo', 'UTC'],
};

const LOCALES = {
  Win32: ['en-US', 'en-US', 'en-US', 'en-GB'],
  MacIntel: ['en-US', 'en-US', 'en-US', 'en-GB'],
  'Linux x86_64': ['en-US', 'en-US', 'en-GB', 'de-DE', 'ja-JP'],
};

// ── Simple seeded PRNG (LCG) ──

function createPRNG(seed) {
  let s = seed;
  return function() {
    s = (s * 1664525 + 1013904223) & 0xffffffff;
    return (s >>> 0) / 0xffffffff;
  };
}

function seedFromString(str) {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash + str.charCodeAt(i)) | 0;
  }
  return hash >>> 0;
}

function pick(arr, rng) {
  return arr[Math.floor(rng() * arr.length)];
}

// ── Profile Generation ──

function generateProfile(options = {}) {
  const id = options.id || 'profile-' + crypto.randomBytes(6).toString('hex');
  // If a seed is provided (e.g. API key), use it for deterministic fingerprints
  // so all browsers with the same API key produce identical profiles.
  const seedStr = options.seed || id;
  const rng = createPRNG(seedFromString(seedStr));

  const platform = options.platform || pick(['Win32', 'MacIntel', 'Linux x86_64'], rng);
  const gpus = GPU_DB[platform] || GPU_DB.Win32;
  const gpu = pick(gpus, rng);
  const screens = SCREEN_RESOLUTIONS[platform] || SCREEN_RESOLUTIONS.Win32;
  const screen = pick(screens, rng);
  const fonts = FONT_SETS[platform] || FONT_SETS.Win32;
  const hardwareConcurrency = pick(HARDWARE_CONCURRENCY, rng);
  const deviceMemory = pick(DEVICE_MEMORY, rng);
  const canvasNoise = rng() * 0.01;
  const audioNoise = rng() * 0.01;
  const rectsNoise = rng() * 0.001;
  const timezone = options.timezone || pick(TIMEZONES[platform] || TIMEZONES.Win32, rng);
  const locale = options.locale || pick(LOCALES[platform] || LOCALES.Win32, rng);
  const lang = locale.split('-')[0];

  return {
    id,
    createdAt: new Date().toISOString(),
    navigator: {
      platform,
      hardwareConcurrency,
      deviceMemory,
      maxTouchPoints: 0,
      languages: [locale, lang],
      vendor: 'Google Inc.',
    },
    screen: {
      width: screen.width,
      height: screen.height,
      availWidth: screen.width,
      availHeight: screen.height - 40,
      colorDepth: 24,
      pixelDepth: 24,
      devicePixelRatio: screen.dpr,
    },
    canvas: { noiseSeed: canvasNoise },
    webgl: gpu,
    audio: { noiseSeed: audioNoise },
    rects: { noiseSeed: rectsNoise },
    fonts: { available: fonts },
    timezone,
    locale,
    proxy: options.proxy || null,
  };
}

// ── Build injection script string with profile baked in ──

/**
 * WebGL, shared by page and worker scopes so both report the same GPU —
 * CreepJS's hasBadWebGL compares the page against its service worker.
 *
 * Chrome answers the UNMASKED_* constants with the ANGLE strings ("Google
 * Inc. (NVIDIA)", "ANGLE (NVIDIA, …)") and VENDOR/RENDERER with "WebKit" /
 * "WebKit WebGL". Older personas reported the bare driver strings instead;
 * they keep doing so (profile.webglChrome unset), because a device's reported
 * GPU must not change under its cookie jar.
 */
function buildWebGLBody() {
  return `
  const _glVendor = __fp.webglChrome ? __fp.webgl.vendor : __fp.webgl.unmaskedVendor;
  const _glRenderer = __fp.webglChrome ? __fp.webgl.renderer : __fp.webgl.unmaskedRenderer;
  const _patchWebGL = (proto) => {
    _patch(proto, 'getParameter', (orig) => function getParameter(pname) {
      if (pname === 0x9245) return _glVendor;     // UNMASKED_VENDOR_WEBGL
      if (pname === 0x9246) return _glRenderer;   // UNMASKED_RENDERER_WEBGL
      if (!__fp.webglChrome && pname === 0x1F00) return __fp.webgl.vendor;
      if (!__fp.webglChrome && pname === 0x1F01) return __fp.webgl.renderer;
      return _apply(orig, this, arguments);
    });
  };
  if (typeof WebGLRenderingContext === 'function') _patchWebGL(WebGLRenderingContext.prototype);
  if (typeof WebGL2RenderingContext === 'function') _patchWebGL(WebGL2RenderingContext.prototype);
`;
}

/** Fingerprint patches. Page scope; assumes the mask preamble (_mark, _noise, _patch, _ensure). */
function buildFingerprintBody(profile) {
  const p = JSON.stringify(profile);

  return `
  const __fp = ${p};

  // ── Navigator ──
  // CDP emulation (UA override with platform and accept-language, hardware
  // concurrency, locale) sets these natively; only a value still wrong gets
  // a getter. maxTouchPoints and vendor are right natively on desktop Chrome.
  _ensure(Navigator.prototype, navigator, 'platform', __fp.navigator.platform);
  _ensure(Navigator.prototype, navigator, 'hardwareConcurrency', __fp.navigator.hardwareConcurrency);
  if ('deviceMemory' in Navigator.prototype) {
    _ensure(Navigator.prototype, navigator, 'deviceMemory', __fp.navigator.deviceMemory);
  }
  if (navigator.language !== __fp.navigator.languages[0]) {
    _defineGetter(Navigator.prototype, 'languages', Object.freeze(__fp.navigator.languages.slice()));
    _defineGetter(Navigator.prototype, 'language', __fp.navigator.languages[0]);
  }

  // ── Screen ──
  // Emulation.setDeviceMetricsOverride sets width/height natively, so matchMedia
  // agrees with them. The avail* pair is always ours: an emulated screen has
  // no taskbar, which is its own tell. devicePixelRatio is left native — a
  // spoofed one disagrees with matchMedia('(resolution)').
  for (const key of ['width', 'height', 'availWidth', 'availHeight', 'colorDepth', 'pixelDepth']) {
    _ensure(Screen.prototype, screen, key, __fp.screen[key]);
  }

  // ── Canvas fingerprint noise ──
  // Only the anti-aliased edges of a real drawing move, by a stable ±1 keyed
  // to the persona and the pixel's absolute position. Flat fills and cleared
  // pixels read back exactly as drawn: CreepJS clears a canvas and expects
  // zeros ("pixel data modified"), and tiny reads are probes, not fingerprints.
  const __canvasSeed = Math.floor(__fp.canvas.noiseSeed * 100000);
  const _origGetImageData = CanvasRenderingContext2D.prototype.getImageData;

  const _noiseImageData = (img, sx, sy) => {
    const d = img.data, w = img.width;
    if (d.length < 4 * 256) return img;
    for (let row = 0; row < img.height; row++) {
      let i = row * w * 4;
      let pr = d[i], pg = d[i + 1], pb = d[i + 2];
      for (let x = 1; x < w; x++) {
        i += 4;
        const r = d[i], g = d[i + 1], b = d[i + 2];
        const edge = r !== pr || g !== pg || b !== pb;
        pr = r; pg = g; pb = b;
        if (!edge || d[i + 3] === 0) continue;
        const up = _noise(__canvasSeed, sx + x, sy + row, r) >= 0;
        d[i] = up ? (r < 255 ? r + 1 : r - 1) : (r > 0 ? r - 1 : r + 1);
      }
    }
    return img;
  };

  // Noise a COPY, never the source: writing back would noise already-noised
  // data on the next read, and corrupt the page's own canvas.
  const _noisyCopy = (source) => {
    const copy = document.createElement('canvas');
    copy.width = source.width;
    copy.height = source.height;
    const cctx = copy.getContext('2d');
    cctx.drawImage(source, 0, 0);
    const w = Math.min(source.width, 512), h = Math.min(source.height, 512);
    cctx.putImageData(_noiseImageData(_apply(_origGetImageData, cctx, [0, 0, w, h]), 0, 0), 0, 0);
    return copy;
  };

  _patch(HTMLCanvasElement.prototype, 'toDataURL', (orig) => function toDataURL(type, quality) {
    if (!this.width || !this.height) return _apply(orig, this, arguments);
    try { return _apply(orig, _noisyCopy(this), arguments); }
    catch { return _apply(orig, this, arguments); }
  });

  _patch(HTMLCanvasElement.prototype, 'toBlob', (orig) => function toBlob(cb, type, quality) {
    if (!this.width || !this.height) return _apply(orig, this, arguments);
    try { return _apply(orig, _noisyCopy(this), arguments); }
    catch { return _apply(orig, this, arguments); }
  });

  // The same noise on direct reads, or a detector compares them with the export.
  _patch(CanvasRenderingContext2D.prototype, 'getImageData', (orig) => function getImageData(sx, sy) {
    return _noiseImageData(_apply(orig, this, arguments), sx | 0, sy | 0);
  });

  ${buildWebGLBody()}

  // Audio and client-rect noise are gone: CreepJS detects both directly
  // ("sample noise", rect mismatches), and a detected lie costs more than the
  // entropy they added.

  // ── performance.memory ──
  // Present on desktop Chrome, absent in some headless builds — the CHR_MEMORY
  // check. Only filled in where missing, derived from the persona so it is
  // stable and consistent with its deviceMemory.
  if (typeof Performance === 'function' && !('memory' in Performance.prototype)) {
    const limit = (__fp.navigator.deviceMemory || 8) * 1024 * 1024 * 1024 / 4;
    const used = Math.floor(limit * (0.08 + Math.abs(_noise(__canvasSeed, 'heap')) * 0.1));
    const memory = Object.create(null);
    Object.defineProperties(memory, {
      jsHeapSizeLimit: { value: Math.floor(limit), enumerable: true },
      totalJSHeapSize: { value: Math.floor(used * 1.4), enumerable: true },
      usedJSHeapSize: { value: used, enumerable: true },
    });
    _defineGetter(Performance.prototype, 'memory', memory);
  }

  // ── mediaDevices ──
  // A headless container enumerates zero devices; a real desktop never does.
  // Labels stay empty, which is what a real browser returns without permission,
  // and the ids are derived from the persona so they are stable for it.
  if (typeof MediaDevices === 'function' && navigator.mediaDevices) {
    const deviceId = (n) => {
      let h = Math.floor(Math.abs(__fp.canvas.noiseSeed) * 1e9) ^ (n * 2654435761);
      let out = '';
      for (let i = 0; i < 8; i++) { h = (Math.imul(h, 31) + n + i) >>> 0; out += h.toString(16).padStart(8, '0'); }
      return out.slice(0, 64);
    };
    const devices = [
      { kind: 'audioinput', label: '', deviceId: deviceId(1), groupId: deviceId(9) },
      { kind: 'videoinput', label: '', deviceId: deviceId(2), groupId: deviceId(10) },
      { kind: 'audiooutput', label: '', deviceId: 'default', groupId: deviceId(9) },
    ].map((d) => (typeof InputDeviceInfo === 'function' && d.kind !== 'audiooutput'
      ? Object.setPrototypeOf({ ...d, toJSON() { return d; } }, InputDeviceInfo.prototype)
      : Object.setPrototypeOf({ ...d, toJSON() { return d; } }, MediaDeviceInfo.prototype)));

    _patch(MediaDevices.prototype, 'enumerateDevices', (orig) => function enumerateDevices() {
      return _apply(orig, this, arguments).then((real) => (real && real.length ? real : devices));
    });
  }

  // ── WebRTC leak prevention ──
  // Empty ICE servers and no host/srflx candidates, so STUN cannot reveal the
  // machine's real address behind the persona's proxy.
  if (typeof RTCPeerConnection !== 'undefined') {
    const OrigRTC = RTCPeerConnection;
    const RTC = function RTCPeerConnection(config, constraints) {
      config = Object.assign({}, config || {}, { iceServers: [] });
      const pc = new OrigRTC(config, constraints);
      const origSetLocal = pc.setLocalDescription.bind(pc);
      pc.setLocalDescription = function(desc) {
        if (desc && desc.sdp) {
          desc.sdp = desc.sdp.replace(/a=candidate:.*typ (host|srflx).*\\r\\n/g, '');
        }
        return origSetLocal(desc);
      };
      return pc;
    };
    RTC.prototype = OrigRTC.prototype;
    _mark(RTC, 'RTCPeerConnection');
    window.RTCPeerConnection = RTC;
    if (typeof webkitRTCPeerConnection !== 'undefined') window.webkitRTCPeerConnection = RTC;
  }

  // ── Timezone ──
  // Emulation.setTimezoneOverride is primary and native. This backup fires
  // only where it did not apply.
  try {
    if (new Intl.DateTimeFormat().resolvedOptions().timeZone !== __fp.timezone) {
      const tz = __fp.timezone;
      _patch(Intl.DateTimeFormat.prototype, 'resolvedOptions', (orig) => function resolvedOptions() {
        const opts = _apply(orig, this, arguments);
        opts.timeZone = tz;
        return opts;
      });
    }
  } catch {}
`;
}

/**
 * The persona inside a worker. Workers are separate globals that
 * addScriptToEvaluateOnNewDocument never reaches; apply.js evaluates this in
 * each one before its script runs. The UA arrives natively through
 * Network.setUserAgentOverride on the worker's session.
 */
function buildWorkerBody(profile, { userAgent = null } = {}) {
  return `
  const __fp = ${JSON.stringify(profile)};
  const __ua = ${JSON.stringify(userAgent)};
  if (typeof WorkerNavigator === 'function') {
    const _wn = WorkerNavigator.prototype;
    // Network.setUserAgentOverride on the worker session is primary; a
    // service worker can start with the browser UA regardless, so fall back.
    if (__ua) {
      _ensure(_wn, navigator, 'userAgent', __ua);
      _ensure(_wn, navigator, 'appVersion', __ua.replace(/^Mozilla\\//, ''));
    }
    _ensure(_wn, navigator, 'platform', __fp.navigator.platform);
    _ensure(_wn, navigator, 'hardwareConcurrency', __fp.navigator.hardwareConcurrency);
    if ('deviceMemory' in _wn) _ensure(_wn, navigator, 'deviceMemory', __fp.navigator.deviceMemory);
    if (navigator.language !== __fp.navigator.languages[0]) {
      _defineGetter(_wn, 'languages', Object.freeze(__fp.navigator.languages.slice()));
      _defineGetter(_wn, 'language', __fp.navigator.languages[0]);
    }
  }
  ${buildWebGLBody()}
`;
}

/** Fingerprint alone, self-contained. */
function buildFingerprintInjectScript(profile) {
  const { buildMaskPreamble } = require('./stealth');
  return `(function() {\n'use strict';\n${buildMaskPreamble()}\n${buildFingerprintBody(profile)}\n})();`;
}

module.exports = { generateProfile, buildFingerprintInjectScript, buildFingerprintBody, buildWorkerBody };
