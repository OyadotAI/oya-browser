/**
 * Real-DOM checks for the page's recorder (scripts/analyzer.js): what a person
 * does with trusted mouse, wheel and keyboard input, and the steps it becomes.
 * A double-click, a hover that reveals a button, a scroll, a Tab with nothing
 * focused, a rich-text field with no label, a field inside a component's
 * shadow root, and a test id every row repeats.
 *
 * Run: npm run test:recorder
 */
/* global window */
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require_ = createRequire(import.meta.url);
const { chromium } = require_('playwright-core');
const ANALYZER = readFileSync(fileURLToPath(new URL('../../scripts/analyzer.js', import.meta.url)), 'utf8')
  .replace('__OYA_ATTR__', 'data-oya-id')
  .replace('__OYA_RECORD__', 'true');

/** One fixture holding each widget, far enough apart that the page scrolls. */
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

let browser;
let page;

before(async () => {
  browser = await chromium.launch();
});

after(async () => {
  await browser.close();
});

/** A fresh page with the recorder running. */
async function recordingPage() {
  page = await browser.newPage();
  await page.setContent(FIXTURE, { waitUntil: 'domcontentloaded' });
  await page.evaluate(ANALYZER);
  return page;
}

/** What the recorder has, the typing and scroll in progress included. */
const drain = () => page.evaluate(() => window.__acRecordDrain(true).steps);

/** The steps' actions. */
const actions = (steps) => steps.map((s) => s.action);

test('a double-click is recorded after its two clicks', async () => {
  await recordingPage();
  await page.dblclick('#dbl');
  assert.deepEqual(actions(await drain()), ['click', 'click', 'double_click']);
});

test('a button a CSS hover reveals is clicked after hovering what reveals it', async () => {
  await recordingPage();
  await page.hover('.card');
  await page.click('#reveal');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['hover', 'click']);
  assert.match(steps[0].el.path, /div/);
});

test('an item in a menu that opened on hover is clicked after hovering its trigger', async () => {
  await recordingPage();
  await page.hover('#menu');
  await page.click('#menuItem');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['hover', 'click']);
  assert.equal(steps[0].el.domId, 'menu');
});

test('scrolling with the wheel is one step with its direction and distance', async () => {
  await recordingPage();
  await page.mouse.move(100, 100);
  await page.mouse.wheel(0, 700);
  await page.waitForTimeout(600);
  const [scroll] = await drain();
  assert.deepEqual([scroll.action, scroll.direction, scroll.amount >= 600], ['scroll', 'down', true]);
});

test('double-clicking text nothing marks as interactive is still a step', async () => {
  await recordingPage();
  await page.dblclick('#plain');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['double_click']);
  assert.equal(steps[0].el.text, 'Edit me');
});

test('an element with no text is not named after its test id', async () => {
  await recordingPage();
  await page.locator('[data-testid="row-toggle"]').first().check();
  const [step] = await drain();
  assert.equal(step.el.text, '');
});

test('clicks inside a popup its trigger opened by click need no hover, however many there are', async () => {
  await recordingPage();
  await page.click('#guests');
  await page.click('#plus');
  await page.click('#plus');
  await page.click('#minus');
  assert.deepEqual(actions(await drain()), ['click', 'click', 'click', 'click']);
});

test('opening a disclosure by its summary is a click', async () => {
  await recordingPage();
  await page.click('#more');
  const [step] = await drain();
  assert.deepEqual([step.action, step.el.text], ['click', 'More']);
});

test('a click on a line inside a code editor is not named by the code', async () => {
  await recordingPage();
  await page.click('#code .line');
  const [step] = await drain();
  assert.deepEqual([step.el.text, step.el.stableText], ['', undefined]);
});

test('Tab with nothing focused is not a step', async () => {
  await recordingPage();
  await page.keyboard.press('Tab');
  assert.deepEqual(await drain(), []);
});

test('a rich-text field with no label is not named by what it contains', async () => {
  await recordingPage();
  await page.click('#note');
  await page.keyboard.type(' more');
  const steps = await drain();
  assert.deepEqual(actions(steps), ['click', 'type']);
  assert.deepEqual(
    steps.map((s) => s.el.text),
    ['', ''],
  );
});

test('a field inside a shadow root is found through its host', async () => {
  await recordingPage();
  await page.locator('x-field').nth(1).locator('input').fill('hello');
  const [step] = await drain();
  assert.match(step.el.host, /x-field:nth-of-type\(2\)$/);
  assert.ok(step.el.path.startsWith(step.el.host + ' '), 'the path starts at the host');
  assert.equal(await page.locator(step.el.path).count(), 1, 'the path finds exactly the field typed into');
});

test('a test id the page repeats on every row is marked as repeated', async () => {
  await recordingPage();
  await page.locator('[data-testid="row-toggle"]').nth(1).check();
  const [step] = await drain();
  assert.equal(step.el.testIdRepeats, 'true');
});
