#!/usr/bin/env node
/**
 * Anti-detection unit checks against a real Chrome.
 *
 * These are the signals a detector reads first. Each assertion is written as
 * "what a real browser does", so a regression shows up as a lie rather than a
 * missing feature.
 *
 * Skipped when no Chrome binary is present.
 */

import { spawn } from 'child_process';
import { mkdtempSync, existsSync, readFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { createRequire } from 'module';
import { CDPConnection } from '../../src/drivers/cdp.ts';
import { removeScratch } from '../support/scratch.js';

const require = createRequire(import.meta.url);
const { buildInjectionScript } = require('../../../browser/anonymity/inject.js');
const { generateProfile } = require('../../../browser/anonymity/fingerprint.js');
const profile = generateProfile({ id: 'test-persona' });

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].find((p) => existsSync(p));
if (!CHROME) {
  console.log('⏭  No Chrome binary, skipping anonymity test');
  process.exit(0);
}

const profilePlatform = profile.navigator.platform;
let passed = 0,
  failed = 0;
const assert = (c, label) => {
  if (c) {
    console.log(`  ✅ ${label}`);
    passed++;
  } else {
    console.log(`  ❌ ${label}`);
    failed++;
  }
};

const userDataDir = mkdtempSync(join(tmpdir(), 'oya-anon-'));
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
});

let conn;
try {
  conn = await new CDPConnection(wsUrl).connect();
  const { targetId } = await conn.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await conn.send('Target.attachToTarget', { targetId, flatten: true });
  await conn.send('Page.enable', {}, sessionId);
  await conn.send('Runtime.enable', {}, sessionId);
  await conn.send('Page.addScriptToEvaluateOnNewDocument', { source: buildInjectionScript(profile) }, sessionId);
  await conn.send('Page.navigate', { url: 'about:blank' }, sessionId);
  await new Promise((r) => setTimeout(r, 400));

  const evaluate = async (expr) => {
    const res = await conn.send(
      'Runtime.evaluate',
      { expression: expr, returnByValue: true, awaitPromise: true },
      sessionId,
    );
    if (res.exceptionDetails) throw new Error(res.exceptionDetails.exception?.description || expr);
    return res.result?.value;
  };

  console.log('\n1️⃣  toString masking, without this every patch below is readable...');
  assert(
    await evaluate(`Function.prototype.toString.toString().includes('[native code]')`),
    'Function.prototype.toString itself reports native',
  );
  assert(
    (await evaluate(`Object.getOwnPropertyDescriptor(Navigator.prototype,'webdriver').get.toString()`)) ===
      'function get webdriver() { [native code] }',
    'a patched accessor reports as a native getter',
  );
  assert(
    await evaluate(`(function realOne(){}).toString().includes('realOne')`),
    'unpatched functions still report their real source',
  );

  console.log('\n2️⃣  navigator.webdriver...');
  assert((await evaluate('navigator.webdriver')) === false, 'is false (undefined is itself the tell)');
  assert(
    (await evaluate(`Object.getOwnPropertyNames(navigator).includes('webdriver')`)) === false,
    'is not an own property of navigator, real Chrome has it on the prototype',
  );
  assert(
    await evaluate(`Object.getOwnPropertyNames(Navigator.prototype).includes('webdriver')`),
    'is on Navigator.prototype',
  );

  console.log('\n3️⃣  plugins and mimeTypes carry the right types...');
  assert(
    (await evaluate(`Object.prototype.toString.call(navigator.plugins)`)) === '[object PluginArray]',
    'navigator.plugins is a PluginArray, not [object Object]',
  );
  assert(
    (await evaluate(`Object.prototype.toString.call(navigator.mimeTypes)`)) === '[object MimeTypeArray]',
    'navigator.mimeTypes is a MimeTypeArray',
  );
  assert(await evaluate('navigator.plugins instanceof PluginArray'), 'instanceof PluginArray holds');
  assert((await evaluate('navigator.plugins.length')) === 5, 'five plugins, matching modern Chrome');
  assert(
    (await evaluate(`Object.prototype.toString.call(navigator.plugins[0])`)) === '[object Plugin]',
    'entries are Plugin instances',
  );
  assert(await evaluate(`navigator.plugins['PDF Viewer'] !== undefined`), 'named access works');

  console.log('\n4️⃣  window.chrome shape...');
  assert((await evaluate('typeof window.chrome.app')) === 'object', 'chrome.app exists');
  assert((await evaluate('typeof window.chrome.csi')) === 'function', 'chrome.csi exists');
  assert((await evaluate('typeof window.chrome.loadTimes')) === 'function', 'chrome.loadTimes exists');
  assert(
    (await evaluate('window.chrome.runtime')) === undefined,
    'chrome.runtime is absent, defining it on a normal page is evidence of automation',
  );

  console.log('\n5️⃣  No self-inflicted flags...');
  assert(
    (await evaluate('typeof Error.prepareStackTrace')) === 'undefined',
    'Error.prepareStackTrace is undefined, as on a real page',
  );
  assert(await evaluate('window.outerWidth > 0 && window.outerHeight > 0'), 'outerWidth/outerHeight are non-zero');

  console.log('\n6️⃣  Nothing identifies the product...');
  const globals = await evaluate(
    `JSON.stringify(Object.getOwnPropertyNames(window).filter(k => /^__(ac|oya)/i.test(k) || k === 'analyzePage'))`,
  );
  assert(globals === '[]', `no product globals on window (found ${globals})`);

  console.log('\n7️⃣  Spoofed surfaces are deterministic...');
  // An advancing RNG made these differ between consecutive calls. No real
  // browser does that, and it is exactly the "lies that lie inconsistently"
  // class CreepJS tests for.
  await conn.send(
    'Page.navigate',
    {
      url: 'data:text/html,<div id=d style="width:200px;height:50px">x</div><canvas id=c width=64 height=64></canvas>',
    },
    sessionId,
  );
  await new Promise((r) => setTimeout(r, 400));

  assert(
    await evaluate(`(() => {
    const el = document.getElementById('d');
    const a = JSON.stringify(el.getBoundingClientRect());
    const b = JSON.stringify(el.getBoundingClientRect());
    return a === b;
  })()`),
    'getBoundingClientRect returns the same value twice',
  );

  assert(
    await evaluate(`(() => {
    const c = document.getElementById('c');
    const ctx = c.getContext('2d');
    ctx.fillStyle = '#abc'; ctx.fillRect(0, 0, 40, 40); ctx.fillText('fp', 5, 30);
    return c.toDataURL() === c.toDataURL();
  })()`),
    'toDataURL returns the same value twice',
  );

  assert(
    (await evaluate(`(() => {
    const el = document.getElementById('d');
    return Object.prototype.toString.call(el.getClientRects());
  })()`)) === '[object DOMRectList]',
    'getClientRects returns a DOMRectList, not an Array',
  );

  assert(
    await evaluate(`Element.prototype.getBoundingClientRect.toString().includes('[native code]')`),
    'the patched getBoundingClientRect reports as native',
  );
  assert(
    await evaluate(`HTMLCanvasElement.prototype.toDataURL.toString().includes('[native code]')`),
    'the patched toDataURL reports as native, fingerprint patches are masked too',
  );

  assert((await evaluate('navigator.platform')) === profilePlatform, 'the profile platform is applied');

  console.log('\n8️⃣  The analyzer works from an isolated world, invisibly...');
  // This is the mechanism browser/main.js now uses: Page.createIsolatedWorld
  // returns the context id directly, so it needs no Runtime.enable, that
  // domain is itself a detection vector.
  await conn.send(
    'Page.navigate',
    { url: 'data:text/html,<button>Press me</button><input placeholder=name>' },
    sessionId,
  );
  await new Promise((r) => setTimeout(r, 400));
  const { frameTree } = await conn.send('Page.getFrameTree', {}, sessionId);
  const { executionContextId } = await conn.send(
    'Page.createIsolatedWorld',
    {
      frameId: frameTree.frame.id,
      worldName: 'w' + Math.random().toString(16).slice(2),
      grantUniveralAccess: true,
    },
    sessionId,
  );
  assert(typeof executionContextId === 'number', 'an isolated world can be created for the frame');

  const tagAttr = 'data-' + Math.random().toString(16).slice(2, 10).padEnd(8, '0');
  const analyzer = readFileSync(new URL('../../../browser/scripts/analyzer.js', import.meta.url), 'utf8').replace(
    '__OYA_ATTR__',
    tagAttr,
  );
  await conn.send(
    'Runtime.evaluate',
    { expression: analyzer, contextId: executionContextId, returnByValue: true },
    sessionId,
  );

  const inWorld = async (expr) => {
    const r = await conn.send(
      'Runtime.evaluate',
      { expression: expr, contextId: executionContextId, returnByValue: true, awaitPromise: true },
      sessionId,
    );
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || expr);
    return r.result?.value;
  };

  assert((await inWorld('typeof analyzePage')) === 'function', 'analyzePage exists inside the isolated world');
  const analyzed = await inWorld('analyzePage({})');
  assert(analyzed?.ok === true, 'analyzePage runs there and returns a result');
  assert(
    (analyzed.data?.elements || []).length >= 2,
    `it indexes the page (${analyzed.data?.elements?.length} elements)`,
  );

  // The whole point: the page cannot see any of it.
  const leaked = await evaluate(
    `JSON.stringify(Object.getOwnPropertyNames(window).filter(k => /^__(ac|oya)/i.test(k) || k === 'analyzePage'))`,
  );
  assert(leaked === '[]', `the page's own world stays clean (found ${leaked})`);
  assert(
    (await evaluate('typeof window.analyzePage')) === 'undefined',
    'window.analyzePage is undefined to the page, the product-specific detector is gone',
  );

  console.log('\n9️⃣  The DOM carries no constant to match on...');
  assert(
    (await evaluate(`document.querySelectorAll('[data-ac-id]').length`)) === 0,
    'no data-ac-id attributes are left on the page',
  );
  // The analyzer still tags elements, but under a name that changes per
  // document, so a MutationObserver has no constant to watch for.
  const tagged = await evaluate(
    `JSON.stringify([...document.querySelectorAll('*')].flatMap(e => [...e.attributes].map(a => a.name)).filter(n => /^data-[0-9a-f]{8}$/.test(n)).slice(0, 1))`,
  );
  assert(tagged !== '[]', `elements are tagged under a randomised name (${tagged})`);
  assert(
    analyzed.data.elements[0].selector.includes('data-'),
    'the returned selector still resolves within this document',
  );
} catch (e) {
  console.log(`  ❌ threw: ${e.message}`);
  failed++;
} finally {
  conn?.close();
  chrome.kill('SIGKILL');
  // Chrome holds the profile briefly after SIGKILL; retry rather than throw
  // over a temp directory and mask the test result.
  await new Promise((r) => setTimeout(r, 300));
  removeScratch(userDataDir);
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
