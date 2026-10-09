/** Live Oya-native automation audit; isolated profile, no debugger or external browser. */
const fs = require('node:fs');
const assert = require('node:assert/strict');
const { app, BrowserWindow } = require('electron');
const path = require('node:path');
const os = require('node:os');
const root = path.resolve(__dirname, '../..');
const { World, watchNativeDialogs, evaluateFrame } = require(root + '/src/main/native/index.ts');
const { Mouse } = require(root + '/src/main/input/mouse.ts');
const { Keyboard } = require(root + '/src/main/input/keyboard.ts');
const { findElementJs, selectOptionJs } = require(root + '/src/main/actions/scripts.ts');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-internet-audit-'));
const profile = process.env.OYA_AUDIT_PROFILE || dir + '/profile';
if (
  path.dirname(path.dirname(profile)) !== os.tmpdir().replace(/\/$/, '') ||
  !path.basename(path.dirname(profile)).startsWith('oya-internet-audit-') ||
  path.basename(profile) !== 'profile'
)
  throw Error('Only a disposable Oya audit profile may be reused');
app.setPath('userData', profile);
app.setName('Oya Native Automation QA');
const report = [];
// Cold responses from this public demo have measured 30-second script fetches.
// Keep external readiness separate from the fast hermetic native regressions.
const liveReadinessMs = 45000;
let catalog = [];
const covered = new Set();
let complete = false;
let win, wc, view, world;
const mouse = new Mouse(),
  keyboard = new Keyboard(process.platform);
let current = 'startup';
const deadline = setTimeout(() => {
  report.push({ name: current, status: 'FATAL', error: 'Live audit exceeded its twenty-minute deadline' });
  save();
  app.exit(1);
}, 1200000);
/** Bound even a native promise that never settles, and release every successful operation's timer. */
async function bounded(operation, ms, message) {
  let timer;
  try {
    return await Promise.race([
      operation(),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error(message)), ms);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
/** Persist results and explicit coverage gaps so an incomplete suite cannot appear green. */
function save() {
  fs.writeFileSync(
    dir + '/report.json',
    JSON.stringify(
      {
        scope: 'Oya native operations; not the full production agent loop',
        complete,
        reusedAuditProfile: !!process.env.OYA_AUDIT_PROFILE,
        catalog,
        results: report,
        uncovered: catalog.filter((p) => !covered.has(p)),
      },
      null,
      2,
    ),
  );
  console.log('REPORT ' + dir);
}
for (const signal of ['SIGTERM', 'SIGINT'])
  process.on(signal, () => {
    report.push({ name: current, status: 'INTERRUPTED', error: `Audit interrupted by ${signal}` });
    save();
    app.exit(1);
  });
/** Wait for observable native state, bounded separately from external resource completion. */
async function until(fn, ms = liveReadinessMs) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await bounded(fn, Math.max(1, end - Date.now()), 'Condition observation timed out')) return;
    await new Promise((r) => setTimeout(r, 80));
  }
  throw Error('Condition timed out');
}
/** Use the production isolated analyzer and retain each observation as audit evidence. */
async function analyze() {
  const a = await bounded(() => world.evaluate(view, 'analyzePage({})'), 15000, 'Native analysis timed out');
  assert.equal(a.ok, true, a.error || JSON.stringify(a));
  fs.appendFileSync(dir + '/analysis.jsonl', JSON.stringify({ test: current, ...a.data }) + '\n');
  return a.data;
}
/** Read text through the exact native main frame without a load-completion gate. */
async function body() {
  return evaluateFrame(wc.mainFrame, "document.body?.innerText || ''");
}
/** Navigate only to the user-authorized test site and wait for a committed usable document. */
async function nav(route) {
  const url = 'https://the-internet.herokuapp.com' + route;
  await new Promise((resolve, reject) => {
    const done = (_event, landed, status) => {
      cleanup();
      if (status === 503) return reject(Error('Site unavailable: native navigation returned HTTP 503'));
      if (new URL(landed).origin !== new URL(url).origin) reject(Error('Unexpected cross-origin redirect'));
      else resolve();
    };
    const timer = setTimeout(() => {
      cleanup();
      reject(Error('Navigation did not commit'));
    }, 75000);
    const cleanup = () => {
      clearTimeout(timer);
      wc.off('did-navigate', done);
    };
    wc.once('did-navigate', done);
    win.loadURL(url).catch((error) => {
      cleanup();
      reject(error);
    });
  });
  await until(async () => {
    try {
      return await evaluateFrame(wc.mainFrame, 'document.readyState !== "loading" && !!document.body');
    } catch {
      return false;
    }
  }, 75000);
  const result = await analyze();
  fs.appendFileSync(
    dir + '/resources.jsonl',
    JSON.stringify({
      url,
      timing: await world.evaluate(
        view,
        'performance.getEntriesByType("resource").map(r=>({name:r.name,duration:r.duration}))',
      ),
    }) + '\n',
  );
  return result;
}
/** Resolve a fresh analyzer id instead of guessing a site selector. */
async function element(match) {
  await until(async () => (await analyze()).elements.some(match));
  const a = await analyze();
  const el = a.elements.find(match);
  if (!el)
    throw Error('Analyzer did not expose expected element: ' + a.elements.map((e) => JSON.stringify(e)).join('\n'));
  return el;
}
/** Use production targeting and native mouse input, refusing covered elements. */
async function click(el) {
  app.focus({ steal: true });
  win.focus();
  wc.focus();
  await until(() => win.isFocused() && wc.isFocused());
  const located = await world.evaluate(view, findElementJs(String(el.id)));
  assert.equal(located.ok, true);
  if (located.data.covered) throw Error('Target is covered');
  await mouse.click(view, located.data.x, located.data.y);
}
/** Inspect the analyzer-owned element reference without mutating the page. */
async function read(el, property) {
  return world.evaluate(view, `window.__acFindElement(${el.id})?.${property}`);
}
/** Record each independent behavioral result and its rendered screenshot. */
async function run(name, fn) {
  current = name;
  const start = Date.now();
  try {
    await fn();
    report.push({ name, status: 'PASS', ms: Date.now() - start });
  } catch (e) {
    report.push({
      name,
      status: 'FAIL',
      error: e?.message || JSON.stringify(e) || String(e),
      url: wc.getURL(),
      focus: { window: win.isFocused(), page: wc.isFocused() },
      stack: e?.stack,
      ms: Date.now() - start,
    });
  }
  try {
    const capture = await bounded(() => wc.capturePage(), 5000, 'Evidence capture timed out');
    fs.writeFileSync(dir + '/' + name + '.png', capture.toPNG());
  } catch (error) {
    report.at(-1).evidenceError = error.message;
  }
  console.log(JSON.stringify(report.at(-1)));
  save();
}
app
  .whenReady()
  .then(async () => {
    win = new BrowserWindow({
      width: 1280,
      height: 900,
      webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
    });
    wc = win.webContents;
    const resourceLog = (details) =>
      fs.appendFileSync(
        dir + '/network.jsonl',
        JSON.stringify({
          url: details.url,
          status: details.statusCode,
          resourceType: details.resourceType,
          error: details.error,
          cached: details.fromCache,
        }) + '\n',
      );
    wc.session.webRequest.onCompleted({ urls: ['https://the-internet.herokuapp.com/*'] }, resourceLog);
    wc.session.webRequest.onErrorOccurred({ urls: ['https://the-internet.herokuapp.com/*'] }, resourceLog);
    view = { webContents: wc };
    Object.defineProperty(wc, 'debugger', {
      get() {
        throw Error('Internal CDP forbidden');
      },
    });
    world = new World({
      analyzerScript: fs.readFileSync(root + '/scripts/analyzer.js', 'utf8'),
      worldName: 'oya-live-audit',
    });
    win.show();
    app.focus({ steal: true });
    win.focus();
    wc.focus();
    await until(() => win.isFocused());
    await nav('/');
    await until(async () => (await analyze()).elements.filter((e) => e.rawHref?.startsWith('/')).length > 40);
    catalog = [...new Set((await analyze()).elements.map((e) => e.rawHref).filter((h) => h?.startsWith('/')))];
    save();
    covered.add('/checkboxes');
    await run('checkboxes', async () => {
      await nav('/checkboxes');
      let el = await element((e) => e.type === 'checkbox');
      const before = await read(el, 'checked');
      await click(el);
      await until(async () => (await read(el, 'checked')) !== before);
      await analyze();
    });
    covered.add('/add_remove_elements/');
    await run('add-remove', async () => {
      await nav('/add_remove_elements/');
      await click(await element((e) => e.text === 'Add Element'));
      await until(async () => (await analyze()).elements.some((e) => e.text === 'Delete'));
      await click(await element((e) => e.text === 'Delete'));
      await until(async () => !(await analyze()).elements.some((e) => e.text === 'Delete'));
    });
    covered.add('/login');
    await run('form-login', async () => {
      await nav('/login');
      await until(async () => (await body()).includes('SuperSecretPassword!'));
      const text = await body();
      assert.ok(text.includes('tomsmith') && text.includes('SuperSecretPassword!'));
      await click(await element((e) => e.type === 'input' && /username/i.test(JSON.stringify(e))));
      await keyboard.type(view, 'tomsmith');
      await click(await element((e) => e.type === 'input' && /password/i.test(JSON.stringify(e))));
      await keyboard.type(view, 'SuperSecretPassword!');
      await click(await element((e) => e.type === 'button' && /Login/.test(e.text)));
      await until(async () => (await body()).includes('You logged into a secure area!'));
      await analyze();
    });
    covered.add('/key_presses');
    await run('keyboard', async () => {
      await nav('/key_presses');
      await click(await element((e) => e.tag === 'input'));
      await keyboard.press(view, 'ArrowLeft');
      await until(async () => (await body()).includes('You entered: LEFT'));
      await analyze();
    });
    covered.add('/dropdown');
    await run('dropdown', async () => {
      await nav('/dropdown');
      const el = await element((e) => e.tag === 'select');
      await click(el);
      await keyboard.press(view, 'End');
      await keyboard.press(view, 'Enter');
      await until(async () => (await read(el, 'value')) === '2');
      await analyze();
    });
    await run('dropdown-agent-select', async () => {
      await nav('/dropdown');
      const el = await element((e) => e.tag === 'select');
      assert.equal((await world.evaluate(view, selectOptionJs(String(el.id), '2'))).ok, true);
      assert.equal(await read(el, 'value'), '2');
    });
    covered.add('/dynamic_controls');
    await run('dynamic-controls', async () => {
      await nav('/dynamic_controls');
      await click(await element((e) => e.text === 'Remove'));
      await until(async () => (await body()).includes("It's gone!"));
      await click(await element((e) => e.text === 'Add'));
      await until(async () => (await body()).includes("It's back!"));
      await analyze();
    });
    covered.add('/dynamic_loading');
    await run('dynamic-loading', async () => {
      await nav('/dynamic_loading');
      const el = await element((e) => /Example 2/.test(e.text));
      const url = new URL(el.href);
      await nav(url.pathname);
      await click(await element((e) => e.text === 'Start'));
      await until(async () => (await body()).includes('Hello World!'));
      await analyze();
    });
    covered.add('/javascript_alerts');
    await run('native-dialogs', async () => {
      await nav('/javascript_alerts');
      let last;
      const off = watchNativeDialogs(
        wc,
        (info, reply) => {
          last = info.dialogType;
          reply(info.dialogType !== 'confirm', 'Oya native prompt');
        },
        () => {},
      );
      try {
        await click(await element((e) => e.text === 'Click for JS Alert'));
        await until(async () => (await body()).includes('You successfully clicked an alert'));
        assert.equal(last, 'alert');
        await click(await element((e) => e.text === 'Click for JS Confirm'));
        await until(async () => (await body()).includes('You clicked: Cancel'));
        await click(await element((e) => e.text === 'Click for JS Prompt'));
        await until(async () => (await body()).includes('You entered: Oya native prompt'));
        await analyze();
      } finally {
        off();
      }
    });
    covered.add('/nested_frames');
    await run('nested-frame-analysis', async () => {
      await nav('/nested_frames');
      let native = [];
      await until(async () => {
        native = [];
        for (const frame of wc.mainFrame.framesInSubtree) {
          try {
            native.push(await evaluateFrame(frame, 'document.body?.innerText'));
          } catch {}
        }
        return ['LEFT', 'MIDDLE', 'RIGHT', 'BOTTOM'].every((t) => native.includes(t));
      });
      const rendered = JSON.stringify(await analyze());
      for (const text of ['LEFT', 'MIDDLE', 'RIGHT', 'BOTTOM'])
        assert.ok(rendered.includes(text), 'Agent analyzer omitted loaded nested-frame text: ' + text);
    });
    covered.add('/windows');
    await run('multiple-windows', async () => {
      await nav('/windows');
      let child;
      wc.setWindowOpenHandler(() => ({
        action: 'allow',
        overrideBrowserWindowOptions: {
          webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
        },
      }));
      wc.once('did-create-window', (w) => {
        child = w;
        Object.defineProperty(w.webContents, 'debugger', {
          get() {
            throw Error('Internal CDP forbidden');
          },
        });
      });
      try {
        await click(await element((e) => e.text === 'Click Here'));
        await until(() => !!child);
        await until(async () =>
          (await child.webContents.executeJavaScript('document.body.innerText')).includes('New Window'),
        );
      } finally {
        child?.destroy();
        wc.setWindowOpenHandler(() => ({ action: 'deny' }));
        win.focus();
        wc.focus();
      }
    });
    covered.add('/shadowdom');
    await run('shadow-dom-analysis', async () => {
      await nav('/shadowdom');
      const rendered = JSON.stringify(await analyze());
      assert.ok(rendered.includes("Let's have some different text!"));
      assert.ok(rendered.includes("Let's have some different text!") && rendered.includes('In a list!'));
    });
    await require('./internet-extra-cases.cjs')({
      check: async (name, route, test) => {
        covered.add(route);
        await run(name, test);
      },
      nav,
      body,
      analyze,
      element,
      click,
      read,
      keyboard,
      mouse,
      view,
      world,
      until,
      app,
      win,
      dir,
      bounded,
      locate: async (el) => {
        const result = await world.evaluate(view, findElementJs(String(el.id)));
        assert.equal(result.ok, true, result.error);
        assert.equal(result.data.covered, false);
        return result.data;
      },
    });
    complete = true;
    save();
    win.destroy();
    clearTimeout(deadline);
    app.exit(report.some((r) => r.status !== 'PASS') || catalog.some((p) => !covered.has(p)) ? 1 : 0);
  })
  .catch((e) => {
    report.push({ name: current, status: 'FATAL', error: e.stack });
    save();
    app.exit(1);
  });
