#!/usr/bin/env node
/**
 * Stealth benchmark.
 *
 * "Zero detection" is neither measurable nor achievable — the published leader
 * sits near 77% bypass. This produces a number instead: a scored set of the
 * probes commercial detectors actually run, so a change can be attributed and
 * a provider can be compared.
 *
 * Local probes by default (deterministic, no network). --live also drives the
 * public test pages, which move around and are therefore not a regression gate.
 *
 * Usage:
 *   node test-stealth.js                # protected vs bare baseline
 *   node test-stealth.js --live         # also hit the public detectors
 *   node test-stealth.js --json         # machine-readable
 */

import { spawn } from 'child_process';
import { createServer } from 'http';
import { mkdtempSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { createRequire } from 'module';
import { CDPConnection } from '../../src/drivers/cdp.ts';
import { userAgentFor, metadataFor } from '../../src/modules/personas/ua.ts';
import { removeScratch } from '../support/scratch.js';

const require = createRequire(import.meta.url);
const { createPersonaApplier } = require('../../../browser/anonymity/apply.js');
const { generateProfile } = require('../../../browser/anonymity/fingerprint.js');

const args = process.argv.slice(2);
const LIVE = args.includes('--live');
const JSON_OUT = args.includes('--json');

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].find((p) => existsSync(p));
if (!CHROME) {
  console.log('⏭  No Chrome binary — skipping stealth benchmark');
  process.exit(0);
}

/**
 * Each probe returns true when the browser looks like a real one.
 * Weight reflects how commonly and how cheaply a detector reads that signal.
 */
const PROBES = [
  { id: 'webdriver.value', weight: 3, expr: `navigator.webdriver === false` },
  { id: 'webdriver.notOwn', weight: 3, expr: `!Object.getOwnPropertyNames(navigator).includes('webdriver')` },
  {
    id: 'toString.mask',
    weight: 3,
    expr: `Object.getOwnPropertyDescriptor(Navigator.prototype,'webdriver').get.toString().includes('[native code]')`,
  },
  { id: 'toString.notOverbroad', weight: 2, expr: `!(function probe(){}).toString().includes('[native code]')` },
  {
    id: 'plugins.type',
    weight: 2,
    expr: `Object.prototype.toString.call(navigator.plugins) === '[object PluginArray]'`,
  },
  { id: 'plugins.populated', weight: 1, expr: `navigator.plugins.length > 0` },
  {
    id: 'mimeTypes.type',
    weight: 1,
    expr: `Object.prototype.toString.call(navigator.mimeTypes) === '[object MimeTypeArray]'`,
  },
  {
    id: 'chrome.runtimeAbsent',
    weight: 2,
    expr: `typeof window.chrome === 'object' && window.chrome.runtime === undefined`,
  },
  { id: 'prepareStackTrace', weight: 2, expr: `typeof Error.prepareStackTrace === 'undefined'` },
  {
    id: 'rects.deterministic',
    weight: 3,
    expr: `(() => { const e=document.body; return JSON.stringify(e.getBoundingClientRect())===JSON.stringify(e.getBoundingClientRect()); })()`,
  },
  {
    id: 'canvas.deterministic',
    weight: 3,
    expr: `(() => { const c=document.createElement('canvas'); c.width=c.height=64; const x=c.getContext('2d');
       x.fillStyle='#123'; x.fillRect(0,0,40,40); x.fillText('probe',4,50); return c.toDataURL()===c.toDataURL(); })()`,
  },
  {
    id: 'rects.type',
    weight: 1,
    expr: `Object.prototype.toString.call(document.body.getClientRects()) === '[object DOMRectList]'`,
  },
  {
    id: 'noProductGlobals',
    weight: 3,
    expr: `Object.getOwnPropertyNames(window).filter(k => /^__(ac|oya)/i.test(k) || k === 'analyzePage').length === 0`,
  },
  { id: 'noBrandedDom', weight: 2, expr: `document.querySelectorAll('[data-ac-id]').length === 0` },
  { id: 'outerDimensions', weight: 1, expr: `window.outerWidth > 0 && window.outerHeight > 0` },
  {
    id: 'permissions.notifications',
    weight: 1,
    expr: `(async () => { const r = await navigator.permissions.query({name:'notifications'}); return typeof r.state === 'string'; })()`,
  },
  // Coherence: two spoofed values checked against one unspoofed one is the
  // whole modern detection playbook.
  {
    id: 'coherence.userAgentData',
    weight: 3,
    expr: `(() => { if (!navigator.userAgentData) return true;
       const p = (navigator.userAgentData.platform || '').toLowerCase();
       const nav = navigator.platform.toLowerCase();
       if (!p) return true;
       if (p.includes('mac')) return nav.includes('mac');
       if (p.includes('win')) return nav.includes('win');
       if (p.includes('linux')) return nav.includes('linux');
       return true; })()`,
  },
  {
    id: 'coherence.uaPlatform',
    weight: 2,
    expr: `(() => { const ua = navigator.userAgent.toLowerCase(), p = navigator.platform.toLowerCase();
       if (p.includes('win')) return ua.includes('windows');
       if (p.includes('mac')) return ua.includes('mac');
       if (p.includes('linux')) return ua.includes('linux') || ua.includes('x11');
       return true; })()`,
  },
  {
    id: 'coherence.languages',
    weight: 1,
    expr: `Array.isArray(navigator.languages) && navigator.languages.length > 0
       && navigator.language === navigator.languages[0]`,
  },
  {
    id: 'mediaDevices.present',
    weight: 2,
    expr: `(async () => { if (!navigator.mediaDevices?.enumerateDevices) return false;
       const d = await navigator.mediaDevices.enumerateDevices(); return d.length > 0; })()`,
  },
  {
    id: 'iframe.patched',
    weight: 3,
    expr: `(() => { const f = document.createElement('iframe'); document.body.appendChild(f);
       const w = f.contentWindow; const ok = w.navigator.webdriver === false; f.remove(); return ok; })()`,
  },
  { id: 'performance.memory', weight: 2, expr: `!!(performance.memory && performance.memory.jsHeapSizeLimit > 0)` },
  { id: 'ua.notHeadless', weight: 3, expr: `!/HeadlessChrome/i.test(navigator.userAgent)` },
  // Client hints must survive the override. Blanking them is worse than not
  // spoofing at all: no real browser reports an empty brand list.
  {
    id: 'uaData.brandsPopulated',
    weight: 3,
    expr: `Array.isArray(navigator.userAgentData?.brands) && navigator.userAgentData.brands.length > 0`,
  },
  { id: 'uaData.platformSet', weight: 3, expr: `!!navigator.userAgentData?.platform` },
  {
    id: 'uaData.highEntropy',
    weight: 2,
    expr: `navigator.userAgentData.getHighEntropyValues(['platform','architecture'])
       .then((v) => !!v.platform && !!v.architecture).catch(() => false)`,
  },
  {
    id: 'ua.platformCoherent',
    weight: 3,
    expr: `(() => { const ua = navigator.userAgent, p = navigator.platform;
       if (p === 'Win32') return /Windows NT/.test(ua);
       if (p === 'MacIntel') return /Macintosh/.test(ua);
       return /Linux|X11/.test(ua); })()`,
  },
  // A Worker is a separate global that addScriptToEvaluateOnNewDocument does
  // not reach, so anything the injection spoofs disagrees between the two
  // scopes and the real machine shows through. CreepJS compares them directly.
  // apply.js covers workers by pausing each one and applying the persona first.
  {
    id: 'worker.coherent',
    weight: 3,
    expr: `(async () => {
       const src = 'self.onmessage=()=>postMessage(navigator.hardwareConcurrency+"|"+navigator.deviceMemory+"|"+navigator.platform)';
       const w = new Worker(URL.createObjectURL(new Blob([src], { type: 'text/javascript' })));
       const got = await new Promise((res) => {
         w.onmessage = (e) => res(e.data);
         w.postMessage(1);
         setTimeout(() => res(null), 4000);
       });
       w.terminate();
       if (!got) return false;
       return got === [navigator.hardwareConcurrency, navigator.deviceMemory, navigator.platform].join('|');
     })()`,
  },
  {
    id: 'webgl.vendorPresent',
    weight: 1,
    expr: `(() => { const c = document.createElement('canvas').getContext('webgl');
       if (!c) return false; const e = c.getExtension('WEBGL_debug_renderer_info');
       return !!(e && c.getParameter(e.UNMASKED_RENDERER_WEBGL)); })()`,
  },
];

const LIVE_TARGETS = [
  {
    id: 'sannysoft',
    url: 'https://bot.sannysoft.com/',
    // Name the failing rows: a count says you are caught, the row says why.
    extract: `(() => {
      const rows = [...document.querySelectorAll('tr')];
      const failed = rows
        .filter((r) => r.querySelector('td.failed, .failed'))
        .map((r) => (r.cells[0]?.innerText || '').trim())
        .filter(Boolean);
      const passed = rows.filter((r) => r.querySelector('td.passed, .passed')).length;
      return { passed, failed: failed.length, failing: failed };
    })()`,
  },
  {
    id: 'creepjs',
    url: 'https://abrahamjuliot.github.io/creepjs/',
    // CreepJS computes for tens of seconds and prints its metrics with the
    // number BEFORE the label ("38% like headless", "100% headless"), which is
    // why the obvious "label: value" patterns read nothing. Poll for the
    // fingerprint id, then pull the headless verdicts — those are the numbers
    // that say whether this browser reads as automated.
    budgetMs: 90_000,
    ready: `/FP ID:/i.test(document.body.innerText || '') && /headless/i.test(document.body.innerText || '')`,
    extract: `(() => {
      const t = (document.body.innerText || '').replace(/\\s+/g, ' ');
      const grab = (re) => { const m = t.match(re); return m ? m[1] : null; };
      const pct = (label) => { const v = grab(new RegExp('(\\\\d+)%\\\\s*' + label, 'i')); return v === null ? null : Number(v); };
      return {
        fingerprintId: grab(/FP ID:\\s*([0-9a-f]{16,})/i),
        fuzzy: grab(/Fuzzy:\\s*([0-9a-f]{16,})/i),
        // "like headless" must be read before the bare "headless", or the
        // generic pattern swallows it.
        likeHeadless: pct('like headless'),
        headless: pct('headless'),
        stealth: pct('stealth'),
        lies: grab(/lies\\s*\\((\\d+)\\)/i) ?? grab(/(\\d+)\\s*lies/i),
        trust: grab(/trust score[:\\s]*([0-9.]+)/i),
        readable: t.length > 500,
        excerpt: /FP ID:/i.test(t) ? undefined : t.slice(0, 200),
      };
    })()`,
  },
];

async function launch() {
  const userDataDir = mkdtempSync(join(tmpdir(), 'oya-bench-'));
  const chrome = spawn(
    CHROME,
    ['--headless=new', '--remote-debugging-port=0', '--no-first-run', `--user-data-dir=${userDataDir}`, 'about:blank'],
    { stdio: ['ignore', 'ignore', 'pipe'] },
  );
  const wsUrl = await new Promise((resolve, reject) => {
    let buf = '';
    const t = setTimeout(() => reject(new Error('Chrome did not report an endpoint')), 20000);
    chrome.stderr.on('data', (d) => {
      buf += d.toString();
      const m = buf.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
      if (m) {
        clearTimeout(t);
        resolve(m[0]);
      }
    });
    chrome.on('exit', () => {
      clearTimeout(t);
      reject(new Error('Chrome exited'));
    });
  });
  return { chrome, wsUrl, userDataDir };
}

/** 127.0.0.1 is a secure context, so gated APIs actually exist. */
const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end('<!doctype html><title>probe</title><body><div id=probe>x</div></body>');
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const PROBE_URL = `http://127.0.0.1:${site.address().port}/`;

async function score({ protect }) {
  const { chrome, wsUrl, userDataDir } = await launch();
  const results = {};
  let conn;
  try {
    conn = await new CDPConnection(wsUrl).connect();
    const { targetId } = await conn.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await conn.send('Target.attachToTarget', { targetId, flatten: true });
    await conn.send('Page.enable', {}, sessionId);

    if (protect) {
      // A persona created today, so the corrected WebGL strings are measured.
      const profile = { ...generateProfile({ id: 'bench-persona' }), webglChrome: true };
      // Measure the real configuration: exactly what the drivers apply
      // (apply.js) — emulation, injection and child targets. The UA is an HTTP
      // header as well as a JS property, so no injected script fixes it alone.
      const version = (await conn.send('Browser.getVersion')).userAgent;
      const brands = await conn
        .send(
          'Runtime.evaluate',
          {
            expression: 'JSON.stringify(navigator.userAgentData?.brands || [])',
            returnByValue: true,
          },
          sessionId,
        )
        .then((r) => {
          try {
            return JSON.parse(r.result?.value || '[]');
          } catch {
            return [];
          }
        })
        .catch(() => []);
      const persona = createPersonaApplier({
        send: (method, params, sid) => conn.send(method, params, sid),
        on: (event, fn) => conn.on(event, fn),
        profile,
        userAgent: {
          userAgent: userAgentFor(profile, version),
          acceptLanguage: profile.navigator.languages.join(','),
          platform: profile.navigator.platform,
          // Without this Chrome empties navigator.userAgentData, which no real
          // browser does — the probes below assert it stays populated.
          userAgentMetadata: metadataFor(profile, version, brands),
        },
      });
      await persona.browser();
      await persona.page(sessionId);
    }
    await conn.send('Page.navigate', { url: PROBE_URL }, sessionId);
    await new Promise((r) => setTimeout(r, 400));

    for (const probe of PROBES) {
      try {
        const res = await conn.send(
          'Runtime.evaluate',
          { expression: probe.expr, returnByValue: true, awaitPromise: true },
          sessionId,
        );
        results[probe.id] = res.exceptionDetails ? false : res.result?.value === true;
      } catch {
        results[probe.id] = false;
      }
    }

    if (LIVE) {
      results.live = {};
      for (const target of LIVE_TARGETS) {
        try {
          await conn.send('Page.navigate', { url: target.url }, sessionId);
          const evaluate = async (expression) =>
            (
              await conn.send(
                'Runtime.evaluate',
                { expression, returnByValue: true, awaitPromise: true },
                sessionId,
                30000,
              )
            ).result?.value;

          // Detectors finish when they finish. Poll for the signal the target
          // names rather than guessing a wait long enough for the slowest one.
          const deadline = Date.now() + (target.budgetMs ?? 15_000);
          await new Promise((r) => setTimeout(r, 3000));
          if (target.ready) {
            while (Date.now() < deadline) {
              if (await evaluate(target.ready).catch(() => false)) break;
              await new Promise((r) => setTimeout(r, 1500));
            }
          } else {
            await new Promise((r) => setTimeout(r, 5000));
          }

          const value = await evaluate(target.extract);
          results.live[target.id] = value ?? { error: 'no result' };
          if (target.ready && !(await evaluate(target.ready).catch(() => false))) {
            results.live[target.id].timedOut = true;
          }
        } catch (e) {
          results.live[target.id] = { error: e.message };
        }
      }
    }
  } finally {
    conn?.close();
    chrome.kill('SIGKILL');
    await new Promise((r) => setTimeout(r, 300));
    removeScratch(userDataDir);
  }
  return results;
}

const total = PROBES.reduce((n, p) => n + p.weight, 0);
const weighted = (r) => PROBES.reduce((n, p) => n + (r[p.id] ? p.weight : 0), 0);

console.log('Running stealth benchmark' + (LIVE ? ' (with live detectors)' : '') + '…\n');
const bare = await score({ protect: false });
const armed = await score({ protect: true });

const bareScore = weighted(bare),
  armedScore = weighted(armed);

if (JSON_OUT) {
  console.log(
    JSON.stringify(
      {
        total,
        bare: bareScore,
        protected: armedScore,
        probes: PROBES.map((p) => ({
          id: p.id,
          weight: p.weight,
          bare: bare[p.id],
          protected: armed[p.id],
        })),
        live: { bare: bare.live, protected: armed.live },
      },
      null,
      2,
    ),
  );
} else {
  console.log('  probe                          weight   bare   protected');
  console.log('  ' + '─'.repeat(58));
  for (const p of PROBES) {
    const b = bare[p.id] ? ' ok ' : 'FAIL';
    const a = armed[p.id] ? ' ok ' : 'FAIL';
    const moved = bare[p.id] !== armed[p.id] ? (armed[p.id] ? '  ←fixed' : '  ←REGRESSED') : '';
    console.log(`  ${p.id.padEnd(30)}   ${String(p.weight).padStart(2)}    ${b}    ${a}${moved}`);
  }
  console.log('  ' + '─'.repeat(58));
  const pct = (n) => `${((n / total) * 100).toFixed(0)}%`;
  console.log(
    `  score                                  ${pct(bareScore).padStart(4)}   ${pct(armedScore).padStart(4)}   (${armedScore}/${total})`,
  );

  if (armed.live) {
    // Both sides, because a live number on its own says nothing about whether
    // any of this is working.
    console.log('\n  live detectors:');
    for (const [id, v] of Object.entries(armed.live)) {
      console.log(`    ${id}`);
      console.log(`      bare      ${JSON.stringify(bare.live?.[id] ?? null)}`);
      console.log(`      protected ${JSON.stringify(v)}`);
    }
  }

  const regressed = PROBES.filter((p) => bare[p.id] && !armed[p.id]);
  if (regressed.length) {
    console.log(
      `\n  ⚠  ${regressed.length} probe(s) are WORSE with protection on: ${regressed.map((p) => p.id).join(', ')}`,
    );
    console.log('     A patch that makes a surface less realistic than leaving it alone is a net loss.');
  }
  const failing = PROBES.filter((p) => !armed[p.id]);
  if (failing.length) {
    console.log(`\n  still failing: ${failing.map((p) => p.id).join(', ')}`);
  }
}

await new Promise((r) => site.close(r));
process.exit(0);
