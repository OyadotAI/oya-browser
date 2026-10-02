/**
 * Real-DOM checks for what the analyzer names and what it lets an agent act on,
 * using the markup payer portals actually ship: a date picker whose days carry
 * no role or handler, a search list that wraps the matched part of an option in
 * styling, a results list whose buttons are named by their DOM id, an icon-only
 * button, and a rating whose radios are hidden behind their labels.
 *
 * Run: npm run test:analyzer
 */
/* global analyzePage */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require_ = createRequire(import.meta.url);
const { chromium } = require_('playwright-core');
const ANALYZER = readFileSync(fileURLToPath(new URL('../../scripts/analyzer.js', import.meta.url)), 'utf8')
  .replace('__OYA_ATTR__', 'data-oya-id')
  .replace('__OYA_RECORD__', 'false');

/** The page every check runs against: one fixture holding each widget. */
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

/** Every element the analyzer found, by id. */
async function analyse() {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.setContent(FIXTURE, { waitUntil: 'domcontentloaded' });
    await page.evaluate(ANALYZER);
    return await page.evaluate(() => analyzePage({ highlight: false }).data.elements);
  } finally {
    await browser.close();
  }
}

/** The first element whose name matches, or undefined. */
const named = (elements, re) => elements.find((e) => re.test(e.text || ''));

const elements = await analyse();

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
