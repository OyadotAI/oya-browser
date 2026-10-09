/** Recorder behavioral regressions in patched Oya only; every original assertion is retained. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { app, BrowserWindow } = require('electron');
const { evaluateFrame } = require('../../src/main/native/index.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const { nativePointer } = require('../../src/main/input/index.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const ANALYZER = fs
  .readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8')
  .replace('__OYA_ATTR__', 'data-oya-id')
  .replace('__OYA_RECORD__', 'true');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-recorder-dom-'));
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
const cases = [];
const test = (name, run) => cases.push({ name, run });
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const deadline = setTimeout(() => {
  console.error('Native recorder deadline exceeded');
  app.exit(1);
}, 60000);
let window, view, mouse;
const keyboard = new Keyboard(process.platform);
const evaluate = (source) => evaluateFrame(view.webContents.mainFrame, source);
const FIXTURE = `<!doctype html><title>Recorder widgets</title>
<style>
  .card .actions { display: none; }
  .card:hover .actions { display: block; }
  #menuList { display: none; }
  #menu[aria-expanded="true"] + #menuList { display: block; }
  .spacer { height: 1600px; }
</style>
<button id="dbl" ondblclick="this.textContent='done'">Double me</button>
<div class="card" style="padding:20px">Profile<div class="actions"><button id="reveal">View profile</button></div></div>
<button id="menu" aria-haspopup="true" aria-expanded="false"
  onmouseenter="this.setAttribute('aria-expanded','true')">Products</button>
<div id="menuList"><a href="#laptops" id="menuItem">Laptops</a></div>
<div contenteditable="true" id="note">start</div>
<label id="plain" ondblclick="this.textContent='editing'">Edit me</label>
<button id="guests" aria-haspopup="true" aria-expanded="false"
  onclick="this.setAttribute('aria-expanded','true'); popup.hidden=false">2 adults</button>
<div id="code" contenteditable="true"><div class="line" style="cursor:pointer"><span>&lt;</span><span>details</span><span>&gt;</span> 3 new</div></div>
<details><summary id="more">More</summary><p>Inside</p></details>
<div id="popup" hidden><button id="plus">+</button><button id="minus">-</button></div>
<ul><li><input type="checkbox" data-testid="row-toggle"></li><li><input type="checkbox" data-testid="row-toggle"></li></ul>
<x-field></x-field><x-field></x-field>
<div class="spacer"></div>
<button id="bottom">At the bottom</button>
<script>
  customElements.define('x-field', class extends HTMLElement {
    constructor() { super(); this.attachShadow({ mode: 'open' }).innerHTML = '<input id="input" name="q">'; }
  });
</script>`;

/** Reload a fresh document and recorder without carrying focus or scrolling between cases. */
async function recordingPage() {
  if (window) window.destroy();
  window = new BrowserWindow({ width: 1280, height: 900, webPreferences: { sandbox: true, contextIsolation: true } });
  view = { webContents: window.webContents };
  Object.defineProperty(view.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  mouse = new Mouse();
  await window.loadFile(path.join(profile, 'fixture.html'));
  app.focus({ steal: true });
  window.focus();
  view.webContents.focus();
  await evaluate(ANALYZER);
}
/** Locate fixture geometry in the native isolated world; no synthetic click events. */
async function point(selector, index = 0) {
  const result = await evaluate(`(()=>{const el=document.querySelectorAll(${JSON.stringify(selector)})[${index}];
    if(!el)throw Error('Missing fixture target'); el.scrollIntoView({block:'center'});
    const r=el.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  await pause(30);
  return result;
}
/** Deterministic native pointer input isolates recorder behavior from human-cadence path variations. */
async function click(selector, index = 0) {
  const p = await point(selector, index);
  nativePointer(view, { type: 'mouseMove', ...p });
  for (const type of ['mouseDown', 'mouseUp']) nativePointer(view, { type, ...p, button: 'left', clickCount: 1 });
}
/** Reveal hover-only content through trusted pointer movement. */
async function hover(selector) {
  const p = await point(selector);
  nativePointer(view, { type: 'mouseMove', ...p });
  await pause(80);
}
/** Native clickCount creates two clicks followed by a real DOM double-click. */
async function doubleClick(selector) {
  let p = await point(selector);
  nativePointer(view, { type: 'mouseMove', ...p });
  // Leaving the hover-only fixture can collapse it and move the intended text.
  p = await point(selector);
  nativePointer(view, { type: 'mouseMove', ...p });
  for (const clickCount of [1, 2]) {
    for (const type of ['mouseDown', 'mouseUp'])
      view.webContents.sendInputEvent({ type, x: Math.round(p.x), y: Math.round(p.y), button: 'left', clickCount });
    await pause(30);
  }
}
/** Focus the exact shadow input, matching the old fill operation's absence of a click step. */
async function shadowFill() {
  await evaluate('document.querySelectorAll("x-field")[1].shadowRoot.querySelector("input").focus()');
  await keyboard.type(view, 'hello');
}
/** Verify the recorded host-prefixed path resolves to exactly one shadow field. */
async function shadowCount(selector) {
  return evaluate(`(()=>{const host=document.querySelector('x-field:nth-of-type(2)');
    const prefix='x-field:nth-of-type(2) ';const at=${JSON.stringify(selector)}.indexOf(prefix);
    if(at<0)return 0;return host.shadowRoot.querySelectorAll(${JSON.stringify(selector)}.slice(at+prefix.length)).length;})()`);
}
/** Allow queued native events to arrive before flushing recorder debounce state. */
async function drain() {
  await pause(30);
  return evaluate('window.__acRecordDrain(true).steps');
}
/** Preserve the original action sequence assertions. */
const actions = (steps) => steps.map((step) => step.action);
test('a double-click is recorded after its two clicks', async () => {
  await recordingPage();
  await doubleClick('#dbl');
  assert.deepEqual(actions(await drain()), ['click', 'click', 'double_click']);
});

test('a button a CSS hover reveals is clicked after hovering what reveals it', async () => {
  await recordingPage();
  await hover('.card');
  await click('#reveal');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['hover', 'click']);
  assert.match(steps[0].el.path, /div/);
});

test('an item in a menu that opened on hover is clicked after hovering its trigger', async () => {
  await recordingPage();
  await hover('#menu');
  await click('#menuItem');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['hover', 'click']);
  assert.equal(steps[0].el.domId, 'menu');
});

test('scrolling with the wheel is one step with its direction and distance', async () => {
  await recordingPage();
  await mouse.move(view, 100, 100);
  await mouse.scroll(view, 100, 100, 0, 700);
  await pause(600);
  const [scroll] = await drain();
  assert.deepEqual([scroll.action, scroll.direction, scroll.amount >= 600], ['scroll', 'down', true]);
});

test('double-clicking text nothing marks as interactive is still a step', async () => {
  await recordingPage();
  await doubleClick('#plain');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['double_click']);
  assert.equal(steps[0].el.text, 'Edit me');
});

test('an element with no text is not named after its test id', async () => {
  await recordingPage();
  await click('[data-testid="row-toggle"]');
  const [step] = await drain();
  assert.equal(step.el.text, '');
});

test('clicks inside a popup its trigger opened by click need no hover, however many there are', async () => {
  await recordingPage();
  await click('#guests');
  await click('#plus');
  await click('#plus');
  await click('#minus');
  assert.deepEqual(actions(await drain()), ['click', 'click', 'click', 'click']);
});

test('opening a disclosure by its summary is a click', async () => {
  await recordingPage();
  await click('#more');
  const [step] = await drain();
  assert.deepEqual([step.action, step.el.text], ['click', 'More']);
});

test('a click on a line inside a code editor is not named by the code', async () => {
  await recordingPage();
  await click('#code .line');
  const [step] = await drain();
  assert.deepEqual([step.el.text, step.el.stableText], ['', undefined]);
});

test('Tab with nothing focused is not a step', async () => {
  await recordingPage();
  await keyboard.press(view, 'Tab');
  assert.deepEqual(await drain(), []);
});

test('a rich-text field with no label is not named by what it contains', async () => {
  await recordingPage();
  await click('#note');
  await keyboard.type(view, ' more');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['click', 'type']);
  assert.deepEqual(
    steps.map((s) => s.el.text),
    ['', ''],
  );
});

test('a field inside a shadow root is found through its host', async () => {
  await recordingPage();
  await shadowFill();
  const [step] = await drain();
  assert.match(step.el.host, /x-field:nth-of-type\(2\)$/);
  assert.ok(step.el.path.startsWith(step.el.host + ' '), 'the path starts at the host');
  assert.equal(await shadowCount(step.el.path), 1, 'the path finds exactly the field typed into');
});

test('a test id the page repeats on every row is marked as repeated', async () => {
  await recordingPage();
  await click('[data-testid="row-toggle"]', 1);
  const [step] = await drain();
  assert.equal(step.el.testIdRepeats, 'true');
});

/** Run all behavioral cases without skipping or switching browser providers on failure. */
async function run() {
  await app.whenReady();
  fs.writeFileSync(path.join(profile, 'fixture.html'), FIXTURE);
  for (const item of cases) {
    try {
      await item.run();
      console.log('PASS: ' + item.name);
    } catch (error) {
      throw Error(item.name + ': ' + error.stack, { cause: error });
    }
  }
  console.log('PASS: all ' + cases.length + ' native recorder DOM cases');
}
run().then(
  () => {
    clearTimeout(deadline);
    app.exit(0);
  },
  (error) => {
    console.error(error.stack);
    clearTimeout(deadline);
    app.exit(1);
  },
);
