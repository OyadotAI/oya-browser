/** Real analyzer DOM checks in Oya only; debugger access is forbidden. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { World } = require('../../src/main/native/index.ts');
const { findElementJs } = require('../../src/main/actions/scripts.ts');
const { PageDriver } = require('../../src/main/actions/driver.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-analyzer-dom-'));
app.setPath('userData', profile);
const world = new World({
  analyzerScript: fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  worldName: 'native-analyzer-dom',
});
const deadline = setTimeout(() => app.exit(1), 30000);
const FIXTURE = `<!doctype html><title>Portal widgets</title>
<div class="datepicker datepicker-dropdown">
  <div class="datepicker-days">
    <table class="table-condensed">
      <thead><tr><th>Su</th><th>Mo</th></tr></thead>
      <tbody><tr><td class="old day">22</td><td class="day">23</td></tr></tbody>
    </table>
  </div>
</div>
<ul class="results">
  <li><button id="pickPayer0"><b>PAYER</b> - CA</button></li>
</ul>
<table><tr class="result-row">
  <td>Jordan Example Provider NPI 0000000000 In Network: Y</td>
  <td><button id="selectProvider0" name="selectProvider0"></button></td>
</tr></table>
<button id="npiSearch"><i class="fa fa-search"></i></button>
<button id="spriteButton"><svg><use href="#icon-trash"></use></svg></button>
<div class="rating">
  <input type="radio" name="stars" id="s1" style="display:none"><label for="s1">★</label>
  <input type="radio" name="stars" id="s2" style="display:none" checked><label for="s2">★</label>
</div>
<div class="suggestions" style="width:840px">
  <div class="row" style="cursor:pointer;padding:8px"><div>Tartine Bakery</div><div>600 Guerrero St</div></div>
  <div class="row" style="cursor:pointer;padding:8px"><div>Tartine Manufactory</div><div>595 Alabama St</div></div>
</div>
<div class="panel" style="cursor:pointer;width:840px;height:400px"><p>A whole panel is not one control</p></div>`;

/** Analyze the existing portal fixture without launching a third-party browser. */
async function analyse(win) {
  const file = path.join(profile, 'portal.html');
  fs.writeFileSync(file, FIXTURE);
  await win.loadFile(file);
  return (await world.evaluate({ webContents: win.webContents }, 'analyzePage({highlight:false})')).data.elements;
}
/** Nested frames must be readable and their coordinates must reach the real target. */
async function frames(win) {
  fs.writeFileSync(
    path.join(profile, 'frames.html'),
    '<frameset rows="65%,35%"><frame src="row.html"><frame src="bottom.html"></frameset>',
  );
  fs.writeFileSync(
    path.join(profile, 'row.html'),
    '<frameset cols="30%,35%,35%"><frame src="left.html"><frame src="middle.html"><frame src="right.html"></frameset>',
  );
  for (const name of ['left', 'middle', 'right', 'bottom'])
    fs.writeFileSync(
      path.join(profile, name + '.html'),
      `<body>${name.toUpperCase()}<button onclick="this.textContent=event.isTrusted?'TRUSTED':'SYNTHETIC'">${name} action</button>`,
    );
  await win.loadFile(path.join(profile, 'frames.html'));
  const view = { webContents: win.webContents };
  const before = await world.evaluate(view, 'analyzePage({})');
  for (const word of ['LEFT', 'MIDDLE', 'RIGHT', 'BOTTOM'])
    assert.ok(JSON.stringify(before).includes(word), word + ' missing');
  const target = before.data.elements.find((e) => e.text === 'middle action');
  assert.ok(target?.visible, 'nested target must be visible to the agent');
  const located = await world.evaluate(view, findElementJs(String(target.id)));
  assert.equal(located.ok, true);
  win.show();
  win.focus();
  win.webContents.focus();
  const driver = new PageDriver({
    mouse: new Mouse(),
    keyboard: new Keyboard(process.platform),
    getActiveView: () => view,
    injectScripts: (view) => world.ensure(view),
    worldEval: (view, code) => world.evaluate(view, code),
    sendResult: (_id, ok, _data, error) => assert.equal(ok, true, error),
  });
  await driver.runPageAction('nested-click', 'click', { selector: String(target.id) }, view);
  const after = await world.evaluate(view, 'analyzePage({})');
  assert.ok(
    after.data.elements.some((e) => e.text === 'TRUSTED'),
    'coordinates must deliver a trusted click into the nested frame',
  );
}
/** Non-click pointer targets and listeners on table cells must remain addressable by analyzer id. */
async function pointerTargets(win) {
  const file = path.join(profile, 'pointer-targets.html');
  fs.writeFileSync(
    file,
    `<!doctype html><div draggable="true">Move me</div>
    <div draggable="false">Not draggable</div><div oncontextmenu="return false" style="width:100px;height:100px"></div>
    <input type="submit" value="Upload"><input type="button" value="Choose"><input type="reset" value="Clear">
    <table><thead><tr><th onclick="this.textContent='Sorted'" style="cursor:pointer">Sort name</th><th>Plain</th></tr></thead>
    <tbody><tr><td>Alice</td><td>One</td></tr><tr><td>Bob</td><td>Two</td></tr></tbody></table>
    <script>document.querySelectorAll('th')[1].addEventListener('click', event => {
      event.currentTarget.dataset.trusted = String(event.isTrusted);
    });</script>`,
  );
  await win.loadFile(file);
  const result = await world.evaluate({ webContents: win.webContents }, 'analyzePage({})');
  const elements = result.data.elements;
  assert.equal(elements.filter((e) => e.type === 'draggable').length, 1);
  assert.equal(elements.filter((e) => e.type === 'contextmenu').length, 1);
  assert.equal(elements.filter((e) => e.text === 'Sort name').length, 1);
  for (const label of ['Upload', 'Choose', 'Clear'])
    assert.ok(elements.some((e) => e.type === 'button' && e.text === label));
  assert.ok(elements.some((e) => e.text === 'Plain' && e.type === 'columnheader'));
  assert.ok(!elements.some((e) => e.text === 'Not draggable'));
  const header = elements.find((e) => e.text === 'Plain');
  const view = { webContents: win.webContents };
  const point = await world.evaluate(view, findElementJs(String(header.id)));
  assert.equal(point.ok, true);
  win.focus();
  win.webContents.focus();
  await new Mouse().click(view, point.data.x, point.data.y);
  assert.equal(await world.evaluate(view, `window.__acFindElement(${header.id}).dataset.trusted`), 'true');
}
/** An iframe body is a real editing target, not just text to flatten into the outer document. */
async function frameEditor(win) {
  fs.writeFileSync(
    path.join(profile, 'editor.html'),
    '<body contenteditable="plaintext-only" style="height:200px" oninput="this.dataset.trusted=event.isTrusted">Draft',
  );
  fs.writeFileSync(
    path.join(profile, 'editor-host.html'),
    '<iframe src="editor.html" style="width:500px;height:250px"></iframe>',
  );
  await win.loadFile(path.join(profile, 'editor-host.html'));
  const view = { webContents: win.webContents };
  const before = await world.evaluate(view, 'analyzePage({})');
  const field = before.data.elements.find((e) => e.type === 'editable');
  assert.ok(field, 'the iframe body must have an agent-addressable id');
  const point = await world.evaluate(view, findElementJs(String(field.id)));
  assert.equal(point.ok, true);
  win.focus();
  win.webContents.focus();
  await new Mouse().click(view, point.data.x, point.data.y);
  await new Keyboard(process.platform).type(view, 'Oya');
  const after = await world.evaluate(view, 'analyzePage({})');
  assert.ok(JSON.stringify(after).includes('Oya'));
  assert.equal(await world.evaluate(view, `window.__acFindElement(${field.id}).dataset.trusted`), 'true');
}
/** Run the existing portal assertions before nested-frame targeting. */
async function run() {
  const win = new BrowserWindow({
    show: false,
    width: 1200,
    height: 850,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  Object.defineProperty(win.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  /** The first element whose name matches, or undefined. */
  const named = (elements, re) => elements.find((e) => re.test(e.text || ''));

  const elements = await analyse(win);

  // A date picker's days: clickable although the markup says nothing.
  const day = named(elements, /^23$/);
  assert.ok(day, 'a date picker day is an element an agent can click');
  assert.equal(day.type, 'button');
  assert.ok(named(elements, /^22$/), 'a day outside the month still counts');

  // A search list styles the part that matched; the name is still the whole text.
  const payer = named(elements, /PAYER/);
  assert.ok(payer, 'an option keeps its whole name when part of it is highlighted');
  assert.match(payer.text, /PAYER\s*-\s*CA/);

  // A results-list button named by its DOM id says what it selects.
  const select = elements.find((e) => e.domId === 'selectProvider0');
  assert.ok(select, 'the select button is registered');
  assert.match(select.text, /Jordan Example Provider/, 'it is named after the row it selects');
  assert.doesNotMatch(select.text, /selectProvider/, 'never named after the developer id');

  // An icon-only button is named after its icon.
  assert.match(elements.find((e) => e.domId === 'npiSearch').text, /search/i);
  assert.match(elements.find((e) => e.domId === 'spriteButton').text, /trash/i);

  // A hidden radio behind its label keeps its kind, its state and its place.
  const stars = elements.filter((e) => e.type === 'radio');
  assert.equal(stars.length, 2, 'both stars are elements');
  assert.equal(stars.filter((s) => s.checked).length, 1, 'the chosen star reads as checked');

  // A no-code site's suggestion list: full-width rows with a pointer cursor, no role and no handler attribute.
  const rows = elements.filter((e) => /Tartine/.test(e.text || ''));
  assert.equal(rows.length, 2, 'each full-width suggestion row is one element an agent can click');
  assert.match(rows[0].text, /Tartine Bakery\s*600 Guerrero St/, 'a row is named by all of its text');
  assert.ok(!named(elements, /whole panel/), 'a tall panel with a pointer cursor is not a control');

  console.log(`Analyzer DOM checks passed: ${elements.length} elements across six portal widgets.`);

  await frames(win);
  await pointerTargets(win);
  await frameEditor(win);
  await require('./analyzer-text-index.cjs')(win, world, profile);
  await require('./analyzer-selector.cjs')(win, profile);
  console.log('PASS: nested frameset analysis, visibility and trusted native targeting');
  win.destroy();
}
app
  .whenReady()
  .then(run)
  .then(
    () => finish(0),
    (error) => {
      console.error(error);
      finish(1);
    },
  );
/** Release only this disposable test profile. */
function finish(code) {
  clearTimeout(deadline);
  fs.rmSync(profile, { recursive: true, force: true });
  app.exit(code);
}
