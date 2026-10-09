/** Opt-in public search diagnostic through Oya's production analyzer and native command driver. */
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const { app, BrowserWindow, BrowserView } = require('electron');
const { World } = require('../../src/main/native/index.ts');
const { PageDriver } = require('../../src/main/actions/driver.ts');
const { Shortcuts } = require('../../src/main/shell/shortcuts.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-search-diagnostic-'));
app.setPath('userData', profile);
const timeout = setTimeout(() => {
  console.error('FAIL: diagnostic deadline');
  app.exit(1);
}, 120000);
/** Opt-in isolated-world timings; no instrumentation is exposed in the visited page's global. */
function diagnosticSource() {
  let source = fs.readFileSync(path.join(__dirname, '../../scripts/analyzer.js'), 'utf8');
  if (!process.env.OYA_PROFILE_ANALYZER) return source;
  const names = [
    'isHardHidden',
    'queryShadow',
    'registerElement',
    'stableOf',
    'cssPath',
    'getLabel',
    'hasInteractiveChild',
    'getInteractiveType',
    'skipped',
    'scrollPanel',
    'stableName',
    'scopedText',
    '_analyzePageInner',
  ];
  let wrappers = 'const timings = {};';
  for (const name of names) {
    source = source.replace('function ' + name + '(', 'function original_' + name + '(');
    wrappers += `function ${name}(...args) { const start=performance.now(); try { return original_${name}(...args); } finally { const row=timings['${name}'] ||= {count:0,ms:0}; row.count++;row.ms+=performance.now()-start; ${name === '_analyzePageInner' ? "console.log('OYA_PERF '+JSON.stringify(timings));" : ''} } }`;
  }
  return source.replace(/\}\)\(\);\s*$/, wrappers + '})();');
}
process.on('uncaughtException', (error) => {
  console.error(error);
  clearTimeout(timeout);
  app.exit(1);
});
app
  .whenReady()
  .then(async () => {
    const win = new BrowserWindow({ width: 1280, height: 850, webPreferences: { sandbox: true } });
    const page = new BrowserView({ webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false } });
    win.addBrowserView(page);
    page.setBounds({ x: 0, y: 0, width: 1280, height: 800 });
    page.webContents.on('focus', () => win.webContents.focus());
    new Shortcuts({ shell: { window: win }, control: { snapshot: () => ({ interactive: false }) } }).install(
      page.webContents,
    );
    Object.defineProperty(page.webContents, 'debugger', {
      get() {
        throw Error('Internal CDP forbidden');
      },
    });
    page.webContents.on('console-message', (details) => {
      if (typeof details.message === 'string' && details.message.startsWith('OYA_PERF ')) console.log(details.message);
    });
    const world = new World({
      analyzerScript: diagnosticSource(),
      worldName: 'search-diagnostic',
    });
    const tab = { view: page, id: 1, protection: 'ready' };
    let result;
    const driver = new PageDriver({
      mouse: new Mouse(),
      keyboard: new Keyboard(process.platform),
      getActiveView: () => page,
      tabs: () => [tab],
      injectScripts: (v) => world.ensure(v),
      worldEval: (v, code) => world.evaluate(v, code),
      sendResult: (...args) => {
        result = args;
      },
      pullCookiesFor: async () => {},
      analysisStarted() {},
      analysisFinished() {},
    });
    const run = async (action, params = {}) => {
      const start = performance.now();
      result = undefined;
      await driver.runPageAction(action, action, params, page);
      console.log(
        JSON.stringify({
          action,
          ms: Math.round(performance.now() - start),
          ok: result?.[1],
          error: result?.[3],
          elements: result?.[2]?.elements?.length,
          shown: result?.[2]?.shown,
        }),
      );
      assert.equal(result?.[1], true, result?.[3]);
      return result[2];
    };
    await run('navigate', { url: 'https://www.amazon.com' });
    const analysis = await run('analyze');
    fs.writeFileSync(path.join(profile, 'analysis.json'), JSON.stringify(analysis));
    const search = analysis.elements.find(
      (e) =>
        e.domId === 'twotabsearchtextbox' ||
        (e.type?.startsWith('input') && /search amazon/i.test(e.label || e.placeholder || e.text || '')),
    );
    assert.ok(search, 'Search field missing from first analysis; inspect ' + profile);
    await run('type', { selector: String(search.id), text: 'jordans' });
    const value = await world.evaluate(page, `window.__acFindElement(${search.id}).value`);
    assert.equal(value, 'jordans');
    await run('press_key', { key: 'Enter' });
    const found = await run('analyze');
    fs.writeFileSync(path.join(profile, 'results.json'), JSON.stringify(found));
    console.log('Search URL:', page.webContents.getURL(), 'evidence:', profile);
    assert.match(page.webContents.getURL(), /[?&]k=jordans/);
    win.destroy();
  })
  .then(
    () => {
      clearTimeout(timeout);
      app.exit(0);
    },
    (error) => {
      console.error(error);
      clearTimeout(timeout);
      app.exit(1);
    },
  );
