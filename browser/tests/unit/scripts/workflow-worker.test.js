/**
 * Unit tests for scripts/workflow-worker.cjs, the validation worker: driven
 * through its parent port like the utility process drives it, against a fake
 * Playwright browser. The generated module really runs.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { chromium } = require('@playwright/test');
const { freshRequire } = require('../support/fakes.cjs');
const { normalizeDraft } = require('../../../scripts/workflow.cjs');
const { fakeBrowser, FakeParentPort } = require('../support/playwright-fakes.cjs');

const MAIN = 'about:blank#oya-run-main';

describe('workflow worker', () => {
  let port;
  let directory;
  let browser;

  /** Loads a fresh worker on a fresh port, with `connectOverCDP` answering the fake browser. */
  function load(options = {}, urls = [MAIN]) {
    browser = fakeBrowser(urls, options);
    mock.method(chromium, 'connectOverCDP', async () => browser);
    freshRequire('scripts/workflow-worker.cjs');
  }

  /** Starts a run of `steps` (normalized, as the workspace sends them); resolves with the finished message. */
  function run(steps, extra = {}) {
    const draft = normalizeDraft({ id: 'd', steps });
    port.send({
      type: 'start',
      draft,
      endpoint: 'http://x',
      token: 't',
      pageUrls: { main: MAIN },
      directory,
      ...extra,
    });
    return port.waitFor((m) => m.type === 'finished');
  }

  const page = () => browser.pages[0];
  const statuses = () =>
    port
      .events()
      .filter((e) => e.kind === 'step')
      .map((e) => `${e.stepId}:${e.status}`);

  beforeEach(() => {
    port = new FakeParentPort();
    process.parentPort = port;
    directory = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-worker-'));
  });
  afterEach(() => {
    mock.restoreAll();
    delete process.parentPort;
    fs.rmSync(directory, { recursive: true, force: true });
  });

  it('runs every step in order and reports success', async () => {
    load();
    const finished = await run(
      [
        { id: 'a', action: 'navigate', url: 'https://x.test' },
        { id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
        { id: 'c', action: 'type', text: '{{user}}', candidates: [{ kind: 'label', value: 'Name' }] },
      ],
      { vars: { user: 'ann' } },
    );
    assert.deepEqual(finished, { type: 'finished', status: 'succeeded', assertions: 0 });
    assert.deepEqual(page().log, [
      ['goto', 'https://x.test'],
      ['click', 'css:#go'],
      ['fill', 'label:Name', 'ann'],
    ]);
    assert.deepEqual(statuses(), ['a:running', 'a:passed', 'b:running', 'b:passed', 'c:running', 'c:passed']);
    for (let i = 0; i < 50 && !browser.closed; i++) await new Promise((r) => setTimeout(r, 5));
    assert.ok(browser.closed >= 1, 'the browser connection is closed after the run');
  });

  it('lets the page settle before every step, so widgets wired after load are ready', async () => {
    load();
    await run([
      { id: 'a', action: 'navigate', url: 'https://x.test' },
      { id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
    ]);
    assert.deepEqual(page().settles, ['networkidle', 'networkidle']);
  });

  it('gives the page a moment after a click before the next step, for what the click starts', async () => {
    load();
    await run([
      { id: 'a', action: 'click', candidates: [{ kind: 'css', value: '#sort' }] },
      { id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#sort' }] },
      { id: 'c', action: 'assert_url', expected: 'https://x.test' },
    ]);
    assert.equal(page().graces?.length, 2, 'after each of the two clicks, and not before the first step');
  });

  it('counts and acts on visible matches only: a collapsed menu repeats link names', async () => {
    load();
    await run([{ id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] }]);
    assert.ok(page().visibleOnly.every((desc) => desc === 'css:#go') && page().visibleOnly.length >= 2);
  });

  it('does not take one match for the recorded element when it is another link, and heals to one that is', async () => {
    load({ identities: { 'text:Clothing': { within: true, href: 'https://x.test/clothing.html' } } });
    const el = { tag: 'a', href: 'https://x.test/search?cat=5' };
    const candidates = [
      { kind: 'text', value: 'Clothing' },
      { kind: 'css', value: 'a[href="/search?cat=5"]' },
    ];
    await run([{ id: 'b', action: 'click', el, candidates }]);
    assert.deepEqual(page().log, [['click', 'css:a[href="/search?cat=5"]']]);
  });

  it('lets a page that arrived with the target finish loading before acting on it', async () => {
    let looks = 0;
    load({
      counts: {
        get 'css:#sorter'() {
          return looks++ ? 1 : 0;
        },
      },
    });
    await run([{ id: 'b', action: 'select_option', option: 'Name', candidates: [{ kind: 'css', value: '#sorter' }] }]);
    assert.ok(page().settles.includes('load'), 'waited for the new page to load');
  });

  it('says which page it was on when a target is not found', async () => {
    load({ counts: { 'css:#gone': 0 } });
    await run([{ id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#gone' }] }]);
    const missing = port.messages.map((m) => m.event).find((e) => e?.kind === 'target' && e.count === 0);
    assert.ok(missing?.url, 'the not-found event carries the page address');
  });

  it('checks each target and says how many elements match', async () => {
    load();
    await run([{ id: 'b', action: 'click', candidates: [{ kind: 'role', role: 'button', value: 'Save' }] }]);
    const target = port.events().find((e) => e.kind === 'target');
    assert.deepEqual([target.count, target.message], [1, 'One matching target']);
  });

  it('fails a step whose target is missing without claiming the click happened', async () => {
    load({ counts: { 'css:#gone': 0 } });
    const finished = await run([{ id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#gone' }] }], {
      autoHeal: false,
    });
    assert.equal(finished.status, 'failed');
    assert.equal(finished.stepId, 'b');
    assert.equal(port.events().find((e) => e.kind === 'target').message, 'Target not found');
    assert.deepEqual(page().log, []);
  });

  it('refuses an ambiguous target', async () => {
    load({ counts: { 'css:.row': 3 } });
    const finished = await run([{ id: 'b', action: 'click', candidates: [{ kind: 'css', value: '.row' }] }], {
      autoHeal: false,
    });
    assert.equal(finished.status, 'failed');
    assert.equal(port.events().find((e) => e.kind === 'target').message, 'Multiple matching targets');
  });

  it('reports an unknown outcome when a dispatched click fails', async () => {
    load({ failing: ['css:#go'] });
    const finished = await run([{ id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] }]);
    assert.equal(finished.status, 'outcome-unknown');
    assert.ok(statuses().includes('b:outcome-unknown'));
  });

  it('repairs a missing target from a recorded alternative, then reruns it', async () => {
    load({ counts: { 'css:#old': 0 } });
    const steps = [
      {
        id: 'b',
        action: 'click',
        candidates: [
          { kind: 'css', value: '#old' },
          { kind: 'testId', value: 'go' },
        ],
      },
    ];
    const finished = await run(steps);
    const repair = port.messages.find((m) => m.type === 'repair');
    assert.deepEqual(repair.replacement, { kind: 'testId', value: 'go' });
    assert.deepEqual(repair.original, { kind: 'css', value: '#old' });
    assert.equal(finished.status, 'succeeded');
    assert.deepEqual(page().log, [['click', 'testId:go']]);
    assert.ok(statuses().includes('b:repairing'));
  });

  it('never repairs an assertion', async () => {
    load({ counts: { 'css:#old': 0 } });
    const steps = [
      {
        id: 'b',
        action: 'assert_visible',
        candidates: [
          { kind: 'css', value: '#old' },
          { kind: 'testId', value: 't' },
        ],
      },
    ];
    const finished = await run(steps);
    assert.equal(finished.status, 'failed');
    assert.equal(
      port.messages.find((m) => m.type === 'repair'),
      undefined,
    );
  });

  it('pauses at a breakpoint and continues on resume', async () => {
    load();
    const steps = [{ id: 'a', action: 'press_key', key: 'Enter', breakpoint: true }];
    const done = run(steps);
    await port.waitFor((m) => m.type === 'event' && m.event.status === 'paused');
    assert.deepEqual(page().log, []);
    port.send({ type: 'control', command: 'resume' });
    assert.equal((await done).status, 'succeeded');
    assert.deepEqual(page().log, [['press', 'Enter']]);
  });

  it('steps one action at a time', async () => {
    load();
    const steps = [
      { id: 'a', action: 'press_key', key: 'A' },
      { id: 'b', action: 'press_key', key: 'B' },
    ];
    const done = run(steps, { command: 'step' });
    await port.waitFor((m) => m.type === 'event' && m.event.stepId === 'b' && m.event.status === 'paused');
    assert.deepEqual(page().log, [['press', 'A']]);
    port.send({ type: 'control', command: 'resume' });
    assert.equal((await done).status, 'succeeded');
  });

  it('stops a paused run and says so', async () => {
    load();
    const done = run([{ id: 'a', action: 'press_key', key: 'Enter', breakpoint: true }]);
    await port.waitFor((m) => m.type === 'event' && m.event.status === 'paused');
    port.send({ type: 'control', command: 'stop' });
    const finished = await done;
    assert.deepEqual([finished.status, finished.error], ['stopped', 'Run stopped']);
    assert.deepEqual(page().log, []);
  });

  it('waits at a human checkpoint until resumed', async () => {
    load();
    const done = run([{ id: 'a', action: 'checkpoint' }]);
    const attention = await port.waitFor((m) => m.type === 'event' && m.event.kind === 'attention');
    assert.equal(attention.event.status, 'paused');
    port.send({ type: 'control', command: 'resume' });
    assert.equal((await done).status, 'succeeded');
  });

  it('accepts an alert, dismisses a confirm, and redacts secrets from what it reports', async () => {
    load();
    const answers = [];
    const dialog = (type, message) => ({
      type: () => type,
      message: () => message,
      accept: async () => answers.push(`${type}:accept`),
      dismiss: async () => answers.push(`${type}:dismiss`),
    });
    const steps = [{ id: 'a', action: 'press_key', key: 'Enter', breakpoint: true }];
    const done = run(steps, { vars: { pw: 'hunter22' } });
    await port.waitFor((m) => m.type === 'event' && m.event.status === 'paused');
    page().emit('dialog', dialog('alert', 'hi hunter22'));
    page().emit('dialog', dialog('confirm', 'sure?'));
    port.send({ type: 'control', command: 'resume' });
    await done;
    assert.deepEqual(answers, ['alert:accept', 'confirm:dismiss']);
    const notes = port
      .events()
      .filter((e) => e.kind === 'attention')
      .map((e) => e.message);
    assert.ok(notes[0].startsWith('Browser alert: "hi [redacted]" — accepted'));
    assert.ok(notes[1].includes('dismissed (no recorded checkpoint)'));
  });

  it('records network and console evidence with safe URLs only', async () => {
    load();
    const done = run([{ id: 'a', action: 'press_key', key: 'Enter', breakpoint: true }]);
    await port.waitFor((m) => m.type === 'event' && m.event.status === 'paused');
    page().emit('response', { url: () => 'https://u:p@x.test/a?token=1', status: () => 200 });
    page().emit('console', { type: () => 'error' });
    page().emit('console', { type: () => 'log' });
    port.send({ type: 'control', command: 'resume' });
    await done;
    const network = port.events().find((e) => e.kind === 'network');
    assert.deepEqual([network.url, network.status], ['https://x.test/a', 200]);
    assert.equal(port.events().filter((e) => e.kind === 'console').length, 1);
  });

  it('fails when a validation tab is missing', async () => {
    load({}, ['about:blank#other']);
    const finished = await run([{ id: 'a', action: 'press_key', key: 'Enter' }]);
    assert.deepEqual([finished.status, finished.error], ['failed', 'A validation tab is unavailable']);
  });

  it('removes the generated module when the run ends', async () => {
    load();
    await run([{ id: 'a', action: 'press_key', key: 'Enter' }]);
    for (let i = 0; i < 50 && fs.readdirSync(directory).length; i++) await new Promise((r) => setTimeout(r, 5));
    assert.deepEqual(fs.readdirSync(directory), []);
  });
});
