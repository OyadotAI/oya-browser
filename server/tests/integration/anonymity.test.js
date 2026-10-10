/** Preserve persona and analyzer isolation checks on Oya's native session and authenticated external adapter. */
import { createServer } from 'node:http';
import { once } from 'node:events';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { CDPConnection } from '../../src/drivers/cdp.ts';
import { openNativeFixture } from '../support/native-browser.mjs';

const require = createRequire(import.meta.url);
const { generateProfile } = require('../../../browser/anonymity/fingerprint.js');
const profile = generateProfile({ id: 'test-persona' });

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

/** Loopback documents keep the fixture hermetic, including redirected and nested requests. */
const site = createServer((_req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(
    '<!doctype html><div id=d style="width:200px;height:50px">x</div><canvas id=c width=64 height=64></canvas><button>Press me</button><input placeholder=name>',
  );
});
site.listen(0, '127.0.0.1');
await once(site, 'listening');
const url = `http://127.0.0.1:${site.address().port}/`;
let conn, browser;
try {
  browser = await openNativeFixture();
  await browser.preparePersona({ fingerprint: profile, origins: {}, cookies: [], now: Date.now() });
  await browser.send('navigate', { url });
  const endpoint = await browser.frontDoor();
  conn = await new CDPConnection(endpoint.url, { bearerToken: endpoint.token }).connect();
  const { targetInfos } = await conn.send('Target.getTargets');
  const { sessionId } = await conn.send('Target.attachToTarget', { targetId: targetInfos[0].targetId, flatten: true });

  const evaluate = async (expr) => {
    const res = await conn.send(
      'Runtime.evaluate',
      { expression: expr, returnByValue: true, awaitPromise: true },
      sessionId,
    );
    if (res.exceptionDetails) throw new Error(res.exceptionDetails.exception?.description || expr);
    return res.result?.value;
  };

  /** Waits for the loopback page to finish loading; a fixed sleep raced slow CI runners. */
  const loaded = async (selector) => {
    const deadline = Date.now() + 10000;
    const ready = `location.protocol === 'http:' && document.readyState === 'complete' && !!document.querySelector('${selector}')`;
    while (!(await evaluate(ready).catch(() => false))) {
      if (Date.now() > deadline) throw new Error(`page with ${selector} did not load`);
      await new Promise((r) => setTimeout(r, 50));
    }
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
  await browser.send('navigate', { url });
  await loaded('#c');

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
  // External protocol requests terminate in owner-scoped native isolated worlds.
  await browser.send('navigate', { url });
  await loaded('input');
  const { frameTree } = await conn.send('Page.getFrameTree', {}, sessionId);
  const { executionContextId } = await conn.send(
    'Page.createIsolatedWorld',
    {
      frameId: frameTree.frame.id,
      worldName: 'w' + Math.random().toString(16).slice(2),
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
  await browser?.close();
  await new Promise((resolve) => site.close(resolve));
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
