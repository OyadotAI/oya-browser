/** Reproduce the live-site bug with a digit-bearing attribute and real native trusted input. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { World } = require('../../src/main/native/index.ts');
const { PageDriver } = require('../../src/main/actions/driver.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
/** Exact analyzer-generated selectors, not numeric-ID workarounds, must click and type correctly. */
module.exports = async function selectorRegression(win, profile) {
  const file = path.join(profile, 'selector.html');
  fs.writeFileSync(
    file,
    '<button id="wrong" onclick="document.title=\'WRONG\'">Wrong</button><input id="search"><button id="right" onclick="document.title=event.isTrusted?\'RIGHT\':\'UNTRUSTED\'">Right</button>',
  );
  await win.loadFile(file);
  const source = fs
    .readFileSync(path.join(__dirname, '../../scripts/analyzer.js'), 'utf8')
    .replace('__OYA_ATTR__', 'data-f9c3c8b3');
  const world = new World({ analyzerScript: source, worldName: 'selector-regression' });
  const view = { webContents: win.webContents };
  const elements = (await world.evaluate(view, 'analyzePage({highlight:false})')).data.elements;
  const driver = new PageDriver({
    mouse: new Mouse(),
    keyboard: new Keyboard(process.platform),
    getActiveView: () => view,
    injectScripts: (v) => world.ensure(v),
    worldEval: (v, code) => world.evaluate(v, code),
    sendResult: (_id, ok, _data, error) => assert.equal(ok, true, error),
  });
  win.show();
  win.focus();
  win.webContents.focus();
  const field = elements.find((e) => e.domId === 'search'),
    button = elements.find((e) => e.domId === 'right');
  assert.match(field.selector, /data-f9c3c8b3=/);
  await driver.runPageAction('type', 'type', { selector: field.selector, text: 'jordans' }, view);
  assert.equal(await win.webContents.executeJavaScript('document.querySelector("#search").value'), 'jordans');
  await driver.runPageAction('click', 'click', { selector: button.selector }, view);
  assert.equal(win.webContents.getTitle(), 'RIGHT');
  console.log('PASS: digit-bearing analyzer selectors type into and trusted-click the exact intended controls');
};
