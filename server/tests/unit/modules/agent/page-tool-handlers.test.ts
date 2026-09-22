/**
 * Unit tests for the page tools beyond clicking and typing (run_script, wait_for,
 * find, hover, go_back, go_forward, reload) against a scripted browser, and for
 * the tool list each kind of browser is offered.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { TOOL_HANDLERS } = await import('../../../../src/modules/agent/tool-handlers.ts');
const { toolsOn, BROWSER_TOOLS } = await import('../../../../src/modules/agent/tools.ts');
const recorder = await import('../../../../src/modules/agent/recorder.ts');
const { RUN_SCRIPT_OUTPUT_CHARS, WAIT_FOR_MAX_MS, WAIT_FOR_TRIES } =
  await import('../../../../src/modules/agent/constants.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');

const BROWSER = 'b-page-tools';
let answer: (action, params) => any;
let browser;

/** Runs one tool. */
const run = (name, args = {}) => TOOL_HANDLERS[name](BROWSER, args, {});
/** The commands of one kind the browser received. */
const callsOf = (action) => browser.calls.filter((c) => c.action === action);

describe('page tools', () => {
  beforeEach(async () => {
    answer = () => ({ ok: true, data: {} });
    browser = scriptedBrowser(BROWSER, 'key-a', (a, p) => answer(a, p), 'oya');
    await recorder.startRun(BROWSER, { steps: [], elements: [] });
  });
  afterEach(() => browser.disconnect());

  describe('run_script', () => {
    it('returns what the script returned, as JSON', async () => {
      answer = () => ({ ok: true, data: { value: { rows: 3 } } });
      const out = await run('run_script', { script: 'return { rows: 3 }' });
      assert.deepEqual(JSON.parse(out), { rows: 3 });
      assert.equal(callsOf('run_script')[0].params.script, 'return { rows: 3 }');
    });

    it('refuses a script that acts on the page, without sending it', async () => {
      for (const script of [
        "document.querySelector('a').click()",
        "fetch('/api')",
        "document.querySelector('input').value = 'x'",
        "location.href = 'https://evil.test'",
        'el.innerHTML += "<b>"',
        'new XMLHttpRequest()',
      ]) {
        assert.match(await run('run_script', { script }), /only reads the page/, script);
      }
      assert.equal(callsOf('run_script').length, 0);
    });

    it('lets reading scripts through, including variables named like DOM properties', async () => {
      answer = () => ({ ok: true, data: { value: 1 } });
      const script = 'const value = document.title; const href = location.href; return value === href ? 0 : 1;';
      assert.equal(await run('run_script', { script }), '1');
    });

    it('says when the script threw', async () => {
      answer = () => ({ ok: true, data: { error: 'x is not defined' } });
      assert.equal(await run('run_script', { script: 'return x' }), 'Error: the script threw: x is not defined');
    });

    it('cuts a long result to fit', async () => {
      answer = () => ({ ok: true, data: { value: 'a'.repeat(RUN_SCRIPT_OUTPUT_CHARS * 2) } });
      const out = await run('run_script', { script: 'return 1' });
      assert.ok(out.length < RUN_SCRIPT_OUTPUT_CHARS + 20);
      assert.match(out, /… \(cut\)$/);
    });
  });

  describe('wait_for', () => {
    it('reports the page ready when the condition was met', async () => {
      answer = () => ({ ok: true, data: { value: { met: true, url: 'https://a.test/done' } } });
      const out = await run('wait_for', { text: 'Thanks', url: '/done' });
      assert.match(out, /ready \(shows "Thanks", at https:\/\/a.test\/done\)/);
      const script = callsOf('run_script')[0].params.script;
      assert.match(script, /"text":"Thanks"/);
      assert.match(script, /"idle":false/);
    });

    it('waits for a quiet network when given nothing else, and caps the wait', async () => {
      answer = () => ({ ok: true, data: { value: { met: false, url: 'https://a.test' } } });
      const out = await run('wait_for', { timeout: WAIT_FOR_MAX_MS * 10 });
      assert.match(out, new RegExp(`Waited ${WAIT_FOR_MAX_MS} ms`));
      assert.match(callsOf('run_script')[0].params.script, /"idle":true/);
      assert.equal(callsOf('run_script').length, 1, 'a plain timeout is not retried');
    });

    it('starts again when the page navigated underneath it', async () => {
      let n = 0;
      answer = () =>
        n++
          ? { ok: true, data: { value: { met: true, url: 'https://a.test/next' } } }
          : { ok: false, error: 'context destroyed' };
      assert.match(await run('wait_for', { url: 'next' }), /ready/);
      assert.equal(callsOf('run_script').length, 2);
    });

    it('gives up with the error after its tries', async () => {
      answer = () => ({ ok: false, error: 'gone' });
      assert.equal(await run('wait_for', { url: 'x' }), 'Error: gone');
      assert.equal(callsOf('run_script').length, WAIT_FOR_TRIES);
    });
  });

  describe('find', () => {
    const elements = [
      { id: 1, type: 'button', text: 'Sign in', visible: true },
      { id: 2, type: 'input', placeholder: 'Search products', visible: true },
      { id: 3, type: 'button', text: 'Search', visible: true },
    ];

    it('lists only the matching elements, and keeps them for the next action', async () => {
      answer = () => ({ ok: true, data: { url: 'https://a.test', title: 'Shop', elements } });
      const out = await run('find', { query: 'search products' });
      assert.match(out, /\(2 total/);
      assert.match(out, /Search products/);
      assert.doesNotMatch(out, /Sign in/);
      assert.equal(recorder.elementOf(BROWSER, 1), elements[0]);
    });

    it('says when nothing matches', async () => {
      answer = () => ({ ok: true, data: { url: 'https://a.test', title: 'Shop', elements } });
      assert.match(await run('find', { query: 'checkout' }), /^Nothing on the page matches "checkout"/);
    });
  });

  it('hovers over the element by id and shows what the page has now', async () => {
    answer = (a) =>
      a === 'analyze'
        ? { ok: true, data: { elements: [{ id: 9, type: 'link', text: 'Menu item' }] } }
        : { ok: true, data: {} };
    const out = await run('hover', { element_id: 4 });
    assert.equal(callsOf('hover')[0].params.selector, '[data-ac-id="4"]');
    assert.match(out, /^Hovered over element 4/);
    assert.match(out, /Menu item/);
  });

  it('moves through history with the browser’s back, forward and reload', async () => {
    assert.match(await run('go_back'), /^Went back/);
    assert.match(await run('go_forward'), /^Went forward/);
    assert.match(await run('reload'), /^Reloaded the page/);
    const history = new Set(['back', 'forward', 'reload']);
    assert.deepEqual(
      browser.actions().filter((a) => history.has(a)),
      ['back', 'forward', 'reload'],
    );
  });
});

describe('toolsOn', () => {
  /** The tool names offered to a browser that does `actions`. */
  const names = (actions) => toolsOn(actions).map((t) => t.function.name);

  it('offers every tool when the browser’s actions are not known', () => {
    assert.equal(toolsOn(null), BROWSER_TOOLS);
  });

  it('leaves out the tools a browser cannot run', () => {
    // An app new enough to reload is new enough to run a script; run_script itself is
    // server-internal, so no browser announces it.
    const offered = names(['analyze', 'click', 'reload']);
    assert.ok(['click', 'run_script', 'wait_for', 'reload'].every((n) => offered.includes(n)));
    const older = names(['analyze', 'click', 'hover']);
    for (const missing of ['read_console', 'read_network', 'go_back', 'reload', 'run_script', 'wait_for']) {
      assert.ok(!older.includes(missing), missing);
    }
    assert.ok(older.includes('hover'));
  });
});
