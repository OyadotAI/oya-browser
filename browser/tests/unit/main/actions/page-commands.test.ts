/**
 * Unit tests for the server's page commands: navigation with retries,
 * screenshots, element clicks and typing, key presses, hovers, selects and
 * the page-world evaluation. Timers fire at once and record their delays.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PageDriver } from '../../../../src/main/actions/driver.ts';
import { Keyboard } from '../../../../src/main/input/keyboard.ts';
import { Mouse } from '../../../../src/main/input/mouse.ts';
import * as c from '../../../../src/main/actions/constants.ts';
import { instantTimers, fixedRandom, pageView, pageCtx, results, mouseEvents, keyEvents } from '../../support/page.cjs';

/** A driver over the composition root's (src/main/main.ts) helpers in `ctx`, with its own keyboard and mouse. */
const driverFor = (ctx: any) => new PageDriver({ ...ctx, keyboard: new Keyboard('linux'), mouse: new Mouse() });

/** An element the analyzer found in the main document. */
const FOUND = { ok: true, data: { x: 10, y: 20, inIframe: false } };
/** The same element inside an iframe. */
const IN_IFRAME = { ok: true, data: { x: 10, y: 20, inIframe: true } };

/** Runs one server command against `view` and returns the ctx that recorded it. */
async function run(view, action, params, options) {
  const ctx = pageCtx(view, options);
  await driverFor(ctx).runPageAction('c1', action, params, view);
  return ctx;
}

describe('page commands', () => {
  let delays;
  beforeEach(() => {
    delays = instantTimers();
    fixedRandom(0);
  });
  afterEach(() => mock.restoreAll());

  it('navigate pulls cookies, loads, reloads the analyzer and reports the page', async () => {
    const view = pageView();
    const ctx = await run(view, 'navigate', { url: 'https://x.test' });
    assert.deepEqual(ctx.calls.slice(0, 2), [['pull', 'https://x.test'], ['inject']]);
    assert.deepEqual(view.webContents.calls, [['loadURL', 'https://x.test']]);
    assert.deepEqual(results(ctx), [['c1', true, { url: 'https://a.test/', title: 'Title' }]]);
  });

  it('navigate opens only http(s) and about:blank, so a command cannot read this machine or run as code', async () => {
    for (const url of [
      'file:///etc/passwd',
      ' FILE:///etc/passwd',
      'javascript:alert(1)',
      'view-source:https://x.test',
      'data:text/html,x',
      'chrome://settings',
    ]) {
      const view = pageView();
      const ctx = await run(view, 'navigate', { url });
      assert.deepEqual(view.webContents.calls, [], url);
      assert.deepEqual(
        results(ctx),
        [['c1', false, null, 'Only http and https addresses, or about:blank, can be opened.']],
        url,
      );
    }
  });

  it('navigate in a tab that could not be protected answers tab_unprotected, loads nothing and does not retry', async () => {
    const view = pageView();
    const tabs = [{ id: 1, view, protection: 'failed', setup: Promise.resolve() }];
    const failed = await driverFor(pageCtx(view, { tabs }))
      .runPageAction('c1', 'navigate', { url: 'https://x.test' }, view)
      .then(
        () => null,
        (e) => e,
      );
    assert.equal(failed?.code, 'tab_unprotected');
    assert.ok(view.webContents.calls.every((call) => call[0] === 'input'));
    assert.equal(delays.filter((ms) => ms === c.NAVIGATE_RETRY_MS).length, 0);
  });

  it('navigate retries a failed load, then reports the error', async () => {
    let attempts = 0;
    const view = pageView({ load: async () => Promise.reject(new Error(`fail ${++attempts}`)) });
    const ctx = await run(view, 'navigate', { url: 'https://x.test' });
    assert.equal(attempts, c.NAVIGATE_RETRIES + 1);
    assert.equal(delays.filter((ms) => ms === c.NAVIGATE_RETRY_MS).length, c.NAVIGATE_RETRIES);
    assert.deepEqual(results(ctx), [['c1', false, null, 'fail 3']]);
  });

  it('navigate succeeds once a retry loads', async () => {
    let attempts = 0;
    const view = pageView({ load: async () => (++attempts === 1 ? Promise.reject(new Error('x')) : undefined) });
    const ctx = await run(view, 'navigate', { url: 'https://x.test' });
    assert.equal(results(ctx)[0][1], true);
  });

  it('an aborted navigation is not an error', async () => {
    let attempts = 0;
    const view = pageView({ load: async () => (attempts++, Promise.reject(new Error('net::ERR_ABORTED'))) });
    const ctx = await run(view, 'navigate', { url: 'https://x.test' });
    assert.equal(attempts, 1);
    assert.equal(results(ctx)[0][1], true);
  });

  it('screenshot answers a PNG data URL', async () => {
    const ctx = await run(pageView(), 'screenshot', {});
    assert.deepEqual(results(ctx), [['c1', true, { screenshot: 'data:image/png;base64,UE5H' }]]);
  });

  it('screenshot answers a JPEG when asked for one', async () => {
    const view = pageView();
    const ctx = await run(view, 'screenshot', { format: 'jpeg' });
    assert.deepEqual(results(ctx), [['c1', true, { screenshot: 'data:image/jpeg;base64,SlBFRw==' }]]);
    assert.deepEqual(view.webContents.calls.at(-1), ['jpeg', c.SCREENSHOT_JPEG_QUALITY]);
  });

  it('click answers a missing element with its error', async () => {
    const view = pageView();
    const ctx = await run(view, 'click', { selector: 'a' }, { world: { ok: false, error: 'Element not found: a' } });
    assert.deepEqual(results(ctx), [['c1', false, null, 'Element not found: a']]);
    assert.deepEqual(mouseEvents(view), []);
  });

  it('click presses at the element and reports where the page is', async () => {
    const view = pageView();
    const ctx = await run(view, 'click', { selector: 'a' }, { world: FOUND });
    assert.deepEqual(mouseEvents(view).at(-2), {
      type: 'mouseDown',
      x: 10,
      y: 20,
      button: 'left',
      clickCount: 1,
    });
    assert.deepEqual(results(ctx), [['c1', true, { clicked: true, url: 'https://a.test/', title: 'Title' }]]);
  });

  for (const action of ['click', 'type']) {
    it(`${action} refuses covered targets without input or synthetic page events`, async () => {
      const view = pageView();
      const ctx = await run(
        view,
        action,
        { selector: 'a', text: 'secret' },
        { world: { ...FOUND, data: { ...FOUND.data, covered: true } } },
      );
      assert.deepEqual(results(ctx), [
        ['c1', false, null, 'Element is covered. Dismiss the overlay and analyze again.', 'element_covered'],
      ]);
      assert.deepEqual(mouseEvents(view), []);
      assert.deepEqual(keyEvents(view), []);
      assert.equal(ctx.calls.filter((call) => call[0] === 'world').length, 1);
    });
  }

  it('click inside an iframe sends native input once without synthetic DOM replay', async () => {
    const ctx = await run(pageView(), 'click', { selector: 'a' }, { world: IN_IFRAME });
    assert.ok(!ctx.calls.some((call) => call[0] === 'world' && call[1].includes("new PointerEvent('pointerdown'")));
  });

  it('click waits for a navigation it started', async () => {
    const view = pageView({ loading: true });
    navigateOnInput(view, 'mouseUp');
    const waiting = run(view, 'click', { selector: 'a' }, { world: FOUND });
    await new Promise((resolve) => setImmediate(resolve));
    await waiting;
    assert.ok(!delays.includes(c.LOAD_TIMEOUT_MS));
  });

  it('type with no text only focuses the field', async () => {
    const view = pageView();
    const ctx = await run(view, 'type', { selector: 'i' }, { world: FOUND });
    assert.deepEqual(results(ctx), [['c1', true, { typed: true }]]);
  });

  it('type selects the field, deletes, types and reports suggestions', async () => {
    const view = pageView();
    const world = (expr) => (expr.includes('el.select()') ? 'select' : expr.includes('listbox') ? true : FOUND);
    const ctx = await run(view, 'type', { selector: 'i', text: 'hi' }, { world });
    const keys = view.webContents.calls
      .filter((call) => call[0] === 'input' && ['keyDown', 'keyUp'].includes(call[1].type))
      .map((call) => call[1]);
    assert.deepEqual(
      keys.filter((k) => k.type !== 'keyUp').map((k) => k.keyCode),
      ['Backspace', 'h', 'i'],
    );
    assert.deepEqual(results(ctx), [['c1', true, { typed: true, suggestions_visible: true }]]);
  });

  it('type skips the delete when the field could not be selected', async () => {
    const view = pageView();
    await run(
      view,
      'type',
      { selector: 'i', text: 'h' },
      { world: (expr) => (expr.includes('el.select()') ? false : FOUND) },
    );
    const keys = view.webContents.calls
      .filter((call) => call[0] === 'input' && ['keyDown', 'keyUp'].includes(call[1].type))
      .map((call) => call[1]);
    assert.ok(!keys.some((k) => k.keyCode === 'Backspace'));
  });

  it('type sets a native date input to its value instead of typing into its segments', async () => {
    const view = pageView();
    const world = (expr) =>
      expr.includes("el.tagName === 'INPUT'") ? 'date' : expr.includes('el.value =') ? '2024-01-15' : FOUND;
    const ctx = await run(view, 'type', { selector: 'd', text: '01/15/2024' }, { world });
    const set = ctx.calls.find((call) => call[0] === 'world' && call[1].includes('el.value ='));
    assert.ok(set[1].includes('"2024-01-15"'), "the value is set in the input's own format");
    const keys = view.webContents.calls
      .filter((call) => call[0] === 'input' && ['keyDown', 'keyUp'].includes(call[1].type))
      .map((call) => call[1]);
    assert.equal(keys.length, 0, 'nothing is typed');
    assert.deepEqual(results(ctx), [['c1', true, { typed: true, value: '2024-01-15' }]]);
  });

  it('type tells the agent the format when its text is not a date', async () => {
    const world = (expr) => (expr.includes("el.tagName === 'INPUT'") ? 'date' : FOUND);
    const ctx = await run(pageView(), 'type', { selector: 'd', text: 'next Friday' }, { world });
    assert.deepEqual(results(ctx), [
      ['c1', false, null, 'Could not read "next Friday" as a date value. Type it as YYYY-MM-DD.'],
    ]);
  });

  it('type reports a date the field refused', async () => {
    const world = (expr) =>
      expr.includes("el.tagName === 'INPUT'") ? 'date' : expr.includes('el.value =') ? '' : FOUND;
    const ctx = await run(pageView(), 'type', { selector: 'd', text: '2024-01-15' }, { world });
    assert.deepEqual(results(ctx), [['c1', false, null, 'The date field did not accept 2024-01-15']]);
  });

  it('type into an iframe uses native input after focusing the field', async () => {
    const view = pageView();
    // Native input targets the focused frame rather than synthesizing DOM keystrokes.
    const ctx = await run(
      view,
      'type',
      { selector: 'i', text: 'a/' },
      { world: (e) => (e.includes('listbox') ? false : IN_IFRAME) },
    );
    assert.ok(view.webContents.calls.every((call) => call[0] === 'input'));
    assert.deepEqual(
      keyEvents(view).map((e) => [e.type, e.keyCode]),
      [
        ['keyDown', 'a'],
        ['keyUp', 'a'],
        ['keyDown', '/'],
        ['keyUp', '/'],
      ],
    );
    assert.deepEqual(results(ctx), [['c1', true, { typed: true, suggestions_visible: false }]]);
  });

  it('a filled field in an iframe is cleared with a real Backspace first', async () => {
    const view = pageView();
    const world = (e) => (e.includes('el.select()') ? 'select' : e.includes('listbox') ? false : IN_IFRAME);
    await run(view, 'type', { selector: 'i', text: '1' }, { world });
    assert.deepEqual(
      keyEvents(view).map((e) => [e.type, e.keyCode]),
      [
        ['keyDown', 'Backspace'],
        ['keyUp', 'Backspace'],
        ['keyDown', '1'],
        ['keyUp', '1'],
      ],
    );
  });

  it('a line break in an iframe is a real Enter', async () => {
    const view = pageView();
    await run(
      view,
      'type',
      { selector: 'i', text: '\n' },
      { world: (e) => (e.includes('listbox') ? false : IN_IFRAME) },
    );
    assert.deepEqual(
      keyEvents(view).map((e) => e.keyCode),
      ['Enter', 'Enter'],
    );
  });

  it('type into an unfilled mask moves the caret to its start instead of clearing it', async () => {
    const view = pageView();
    const world = (e) =>
      e.includes('el.isContentEditable ? el.innerText') ? '__/__/____' : e.includes('listbox') ? false : IN_IFRAME;
    const ctx = await run(view, 'type', { selector: 'd', text: '1' }, { world });
    assert.ok(ctx.calls.some((call) => call[0] === 'world' && call[1].includes('setSelectionRange(0, 0)')));
    assert.ok(!ctx.calls.some((call) => call[0] === 'world' && call[1].includes('el.select()')), 'nothing is selected');
    const keys = view.webContents.calls.map(([, event]) => event.keyCode);
    assert.ok(!keys.includes('Backspace'));
  });

  it('type tells the agent what the field shows when it is not what was typed', async () => {
    const world = (e) =>
      e.includes('el.isContentEditable ? el.innerText') ? '02/32/026_' : e.includes('listbox') ? false : IN_IFRAME;
    const ctx = await run(pageView(), 'type', { selector: 'd', text: '10/23/2026' }, { world });
    assert.deepEqual(results(ctx), [['c1', true, { typed: true, suggestions_visible: false, shown: '02/32/026_' }]]);
  });

  it('type says nothing more when the field shows exactly what was typed', async () => {
    const world = (e) =>
      e.includes('el.isContentEditable ? el.innerText') ? 'ada' : e.includes('listbox') ? false : IN_IFRAME;
    const ctx = await run(pageView(), 'type', { selector: 'n', text: 'ada' }, { world });
    assert.deepEqual(results(ctx), [['c1', true, { typed: true, suggestions_visible: false }]]);
  });

  it('a failed suggestions check reports none', async () => {
    const world = async (expr) => (expr.includes('listbox') ? Promise.reject(new Error('gone')) : FOUND);
    const ctx = await run(pageView(), 'type', { selector: 'i', text: 'h' }, { world });
    assert.equal(results(ctx)[0][2].suggestions_visible, false);
  });

  it('press_key refuses keys that change browser state', async () => {
    const view = pageView();
    const ctx = await run(view, 'press_key', { key: 'F5' });
    assert.deepEqual(results(ctx), [['c1', false, null, 'Key "F5" is blocked, it can change browser state']]);
    assert.ok(view.webContents.calls.every((call) => call[0] === 'input'));
  });

  it('press_key reaches the page through CDP, Enter by default', async () => {
    const view = pageView();
    // Electron's own key events reached nothing on an ordinary page: a page that
    // echoes the key it was given stayed unchanged through Enter, Tab and the arrows.
    const ctx = await run(view, 'press_key', {}, { world: false });
    assert.deepEqual(
      keyEvents(view).map((e) => [e.type, e.keyCode]),
      [
        ['keyDown', 'Enter'],
        ['keyUp', 'Enter'],
      ],
    );
    assert.ok(view.webContents.calls.every((call) => call[0] === 'input'));
    assert.deepEqual(results(ctx), [['c1', true, { key: 'Enter' }]]);
  });

  it('press_key sends the same CDP events wherever the focus is', async () => {
    const view = pageView();
    await run(view, 'press_key', { key: 'ArrowDown' }, { world: IN_IFRAME });
    assert.deepEqual(
      keyEvents(view).map((e) => [e.type, e.keyCode]),
      [
        ['keyDown', 'Down'],
        ['keyUp', 'Down'],
      ],
    );
    assert.ok(view.webContents.calls.every((call) => call[0] === 'input'));
  });

  it('Enter that starts a navigation reloads the analyzer', async () => {
    const view = pageView({ loading: true });
    navigateOnInput(view, 'keyDown');
    const ctx = await run(view, 'press_key', { key: 'Enter' }, { world: false });
    assert.ok(ctx.calls.some(([name]) => name === 'inject'));
  });

  it('another key never waits for a navigation', async () => {
    await run(pageView({ loading: true }), 'press_key', { key: 'a' });
    assert.ok(!delays.includes(c.NAVIGATION_START_MS));
  });

  it('hover moves to the rounded element centre', async () => {
    const view = pageView();
    const ctx = await run(view, 'hover', { selector: 'h' }, { world: { ok: true, data: { x: 4.6, y: 5.4 } } });
    assert.deepEqual(mouseEvents(view).at(-1), { type: 'mouseMove', x: 5, y: 5 });
    assert.deepEqual(results(ctx), [['c1', true, { hovered: true }]]);
  });

  it('hover answers a missing element', async () => {
    const ctx = await run(pageView(), 'hover', { selector: 'h' }, { world: null });
    assert.deepEqual(results(ctx), [['c1', false, null, 'Element not found']]);
  });

  it('select answers the script result', async () => {
    const ctx = await run(pageView(), 'select', { selector: 's', value: 'v' }, { world: { ok: false, error: 'nope' } });
    assert.deepEqual(results(ctx), [['c1', false, undefined, 'nope']]);
  });

  it('evaluate_raw runs in the page world of the given view', async () => {
    const view = pageView({ evalValue: 42 });
    const ctx = pageCtx(null);
    await driverFor(ctx).runPageAction('c1', 'evaluate_raw', { expression: '6*7' }, view);
    assert.deepEqual(view.webContents.calls.at(-1), ['evaluate', '6*7']);
    assert.deepEqual(results(ctx), [['c1', true, { result: 42 }]]);
  });

  it('a failing CDP command rejects to the caller', async () => {
    const view = pageView();
    view.webContents.destroyed = true;
    await assert.rejects(run(view, 'screenshot', {}), /destroyed/);
  });
});

/** Reproduce native navigation events during dispatch rather than unrelated resource loading. */
function navigateOnInput(view, type) {
  const original = view.webContents.sendInputEvent;
  view.webContents.sendInputEvent = (event) => {
    original(event);
    if (event.type !== type) return;
    view.webContents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false });
    view.webContents.emit('dom-ready');
  };
}
