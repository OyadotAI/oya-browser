/**
 * Unit tests for the last run per browser: where it starts, which tool calls
 * become steps, and that steps hold stable handles and placeholders, never values.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const recorder = await import('../../../../src/modules/agent/recorder.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');

const BROWSER = 'b-recorder';
const EL = { id: 7, type: 'input', tag: 'input', name: 'email', text: 'ada@x.test', visible: true };
let browser;

/** A fresh run, started on a browser whose active tab is `url`. */
async function started(url = 'https://a.test/start') {
  browser = scriptedBrowser(BROWSER, 'key-a', () => ({ ok: true, data: { tabs: [{ active: true, url }] } }));
  const run = { prompt: 'p', steps: [], elements: [], secrets: [] };
  await recorder.startRun(BROWSER, run);
  return run;
}

describe('hasReplayableSteps', () => {
  it('is true only for a run that did more than navigate', () => {
    assert.equal(recorder.hasReplayableSteps(null), false);
    assert.equal(recorder.hasReplayableSteps({ steps: [{ action: 'navigate' }] }), false);
    assert.equal(recorder.hasReplayableSteps({ steps: [{ action: 'navigate' }, { action: 'click' }] }), true);
  });
});

describe('recorder', () => {
  afterEach(() => browser?.disconnect());

  it('starts a run from the page the browser is on', async () => {
    const run = await started();
    assert.deepEqual(run.steps, [{ action: 'navigate', url: 'https://a.test/start', start: true }]);
    assert.equal(recorder.lastRun(BROWSER), run);
  });

  it('adds no start step when the page is not http(s)', async () => {
    const run = await started('about:blank');
    assert.deepEqual(run.steps, []);
  });

  it('starts empty when the browser cannot list its tabs', async () => {
    browser = scriptedBrowser(BROWSER, 'key-a', () => {
      throw new Error('gone');
    });
    const run = { steps: [], elements: [] };
    await recorder.startRun(BROWSER, run);
    assert.deepEqual(run.steps, []);
  });

  it('drops a tab opened only to be read and closed again', async () => {
    const run = await started();
    await recorder.recordStep(BROWSER, 'open_tab', { url: 'https://a.test/help' }, {});
    await recorder.recordStep(BROWSER, 'close_tab', {}, {});
    assert.deepEqual(
      run.steps.map((s) => s.action),
      ['navigate'],
    );
  });

  it('keeps a tab the run acted in before closing it', async () => {
    const run = await started();
    await recorder.recordStep(BROWSER, 'open_tab', { url: 'https://a.test/sso' }, {});
    await recorder.recordStep(BROWSER, 'press_key', { key: 'Enter' }, {});
    await recorder.recordStep(BROWSER, 'close_tab', {}, {});
    assert.deepEqual(
      run.steps.map((s) => s.action),
      ['navigate', 'open_tab', 'press_key', 'close_tab'],
    );
  });

  it('keeps only the attempt that worked when the agent starts over', async () => {
    const run = await started();
    await recorder.recordStep(BROWSER, 'press_key', { key: 'Tab' }, {});
    await recorder.restartRun(BROWSER);
    await recorder.recordStep(BROWSER, 'press_key', { key: 'Enter' }, {});
    assert.deepEqual(
      run.steps.map((s) => s.key ?? s.url),
      ['https://a.test/start', 'Enter'],
    );
  });

  it('starts the new attempt from the page the agent is on, unless it navigates first', async () => {
    const run = await started('https://a.test/form');
    await recorder.recordStep(BROWSER, 'press_key', { key: 'Tab' }, {});
    await recorder.restartRun(BROWSER);
    await recorder.recordStep(BROWSER, 'navigate', { url: 'https://a.test/other' }, {});
    assert.deepEqual(
      run.steps.map((s) => s.url),
      ['https://a.test/other'],
    );
  });

  it('keeps what an earlier message of the chat did when a later one starts over', async () => {
    const run = await started();
    await recorder.recordStep(BROWSER, 'press_key', { key: 'Tab' }, {});
    recorder.markMessage(BROWSER);
    await recorder.recordStep(BROWSER, 'press_key', { key: 'Escape' }, {});
    await recorder.restartRun(BROWSER);
    assert.deepEqual(
      run.steps.map((s) => s.key ?? s.action),
      ['navigate', 'Tab', 'navigate'],
    );
  });

  it('marks a click that closed a popup as optional, and a click inside one that did something as not', async () => {
    const run = await started();
    const close = { id: 5, tag: 'button', text: 'Close', visible: true };
    const save = { id: 6, tag: 'button', text: 'Save', visible: true };
    recorder.setElements(BROWSER, [close, save], 'Important update');
    await recorder.recordStep(BROWSER, 'click', { element_id: 5 }, {}, recorder.elementOf(BROWSER, 5));
    await recorder.recordStep(BROWSER, 'click', { element_id: 6 }, {}, recorder.elementOf(BROWSER, 6));
    recorder.setElements(BROWSER, [close]);
    await recorder.recordStep(BROWSER, 'click', { element_id: 5 }, {}, recorder.elementOf(BROWSER, 5));
    assert.deepEqual(
      run.steps.slice(1).map((s) => !!s.optional),
      [true, false, false],
    );
  });

  it('has no last run for a browser that never ran', () => {
    assert.equal(recorder.lastRun('b-never'), null);
  });

  it('records an element step by its stable handles with values redacted', async () => {
    const run = await started();
    recorder.setElements(BROWSER, [EL]);
    await recorder.recordStep(BROWSER, 'type', { element_id: '7', text: '{{email}}' }, { email: 'ada@x.test' });
    assert.deepEqual(run.steps[1], {
      action: 'type',
      text: '{{email}}',
      // Only what is known: a handle carries no field the page never gave it.
      el: { type: 'input', tag: 'input', text: '{{email}}', name: 'email' },
    });
  });

  it('finds an element from the latest analysis by the id the model gave', async () => {
    await started();
    recorder.setElements(BROWSER, [EL]);
    assert.equal(recorder.elementOf(BROWSER, '7'), EL);
    assert.equal(recorder.elementOf(BROWSER, 8), undefined);
    assert.equal(recorder.elementOf('b-never', 7), undefined);
  });

  it('records an upload by its variable name, never its bytes', async () => {
    const run = await started();
    recorder.recordStep(BROWSER, 'upload_file', { name: '{{cv}}' }, {});
    assert.deepEqual(run.steps[1], { action: 'upload_file', file: '{{cv}}' });
  });

  it('copies replayable arguments and ignores tools that only read the page', async () => {
    const run = await started();
    await recorder.recordStep(BROWSER, 'scroll', { direction: 'down', amount: 300, junk: 1 }, {});
    await recorder.recordStep(BROWSER, 'handle_dialog', { accept: false }, {});
    await recorder.recordStep(BROWSER, 'analyze_page', {}, {});
    await recorder.recordStep(BROWSER, 'read_elements', {}, {});
    assert.deepEqual(run.steps.slice(1), [
      { action: 'scroll', direction: 'down', amount: 300 },
      { action: 'handle_dialog', accept: false },
    ]);
  });

  it('records a click the page only accepted at a point, rather than dropping it', async () => {
    const run = await started();
    await recorder.recordStep(BROWSER, 'click_coordinates', { x: 1, y: 2 }, {});
    await recorder.recordStep(BROWSER, 'keyboard_type', { text: '{{member_id}}' }, {});
    assert.deepEqual(run.steps.slice(1), [
      { action: 'click_coordinates', x: 1, y: 2 },
      { action: 'keyboard_type', text: '{{member_id}}' },
    ]);
  });

  it('records a tab switch by where it landed, since a tab id dies with the session', async () => {
    const run = await started('https://portal.example.com/web/auth');
    browser.disconnect();
    // After the switch the handoff tab is the active one, so that is what is recorded.
    browser = scriptedBrowser(BROWSER, 'key-a', () => ({
      ok: true,
      data: { tabs: [{ active: true, url: 'https://vendor.example.com/order/new?ssoToken=abc' }] },
    }));
    await recorder.recordStep(BROWSER, 'switch_tab', { tab_id: 4 }, {});
    assert.deepEqual(run.steps.at(-1), {
      action: 'switch_tab',
      tabUrl: 'https://vendor.example.com/order/new?ssoToken=abc',
    });
  });

  it('prefers the handle the browser read as it clicked over a stale analysis', async () => {
    const run = await started();
    recorder.setElements(BROWSER, [{ ...EL, id: 3, text: 'stale' }]);
    recorder.rememberHandle(BROWSER, 3, { tag: 'button', text: 'Next', ariaLabel: 'Next page' });
    await recorder.recordStep(BROWSER, 'click', { element_id: 3 }, {});
    assert.equal(run.steps.at(-1).el.text, 'Next');
    assert.equal(run.steps.at(-1).el.ariaLabel, 'Next page');
  });

  it('keeps a radio’s label when the browser reads no text off the input', async () => {
    const run = await started();
    const radio = { id: 3, tag: 'input', type: 'radio', name: 'duration', text: 'More than 12 weeks', visible: true };
    recorder.setElements(BROWSER, [radio]);
    recorder.rememberHandle(BROWSER, 3, {
      tag: 'input',
      name: 'duration',
      text: '',
      path: 'form > label:nth-of-type(3) > input',
    });
    await recorder.recordStep(BROWSER, 'click', { element_id: 3 }, {}, radio);
    assert.equal(run.steps.at(-1).el.text, 'More than 12 weeks');
  });

  it('does not name a submit input after the form around it', async () => {
    const run = await started();
    const next = {
      id: 4,
      tag: 'input',
      type: 'button',
      text: 'Has this procedure been performed? No Yes',
      visible: true,
    };
    recorder.setElements(BROWSER, [next]);
    recorder.rememberHandle(BROWSER, 4, { tag: 'input', text: '', path: 'form > p > input' });
    await recorder.recordStep(BROWSER, 'click', { element_id: 4 }, {}, next);
    assert.equal(run.steps.at(-1).el.text, '');
  });

  it('still lets the browser say a button has no text', async () => {
    const run = await started();
    recorder.setElements(BROWSER, [{ id: 3, tag: 'button', text: 'stale', visible: true }]);
    recorder.rememberHandle(BROWSER, 3, { tag: 'button', text: '', ariaLabel: 'Close' });
    await recorder.recordStep(BROWSER, 'click', { element_id: 3 }, {});
    assert.equal(run.steps.at(-1).el.text, '');
  });

  it('records a step without a handle once the handle an earlier action left under its id is forgotten', async () => {
    const run = await started();
    recorder.rememberHandle(BROWSER, 3, { tag: 'a', href: '/request', text: 'Authorization Request' });
    recorder.forgetHandle(BROWSER, 3);
    recorder.setElements(BROWSER, [{ id: 3, type: 'select', tag: 'select', domId: 'requestType', visible: true }]);
    await recorder.recordStep(BROWSER, 'select_option', { element_id: 3, option: 'Outpatient' }, {});
    assert.equal(run.steps.at(-1).el.domId, 'requestType');
    assert.equal(run.steps.at(-1).el.href, undefined);
  });

  it('marks a step whose element nothing can find again, rather than aiming it at the body', async () => {
    const run = await started();
    recorder.setElements(BROWSER, []);
    await recorder.recordStep(BROWSER, 'click', { element_id: 99 }, {});
    const step = run.steps.at(-1);
    assert.equal(step.el, undefined);
    assert.equal(step.unaimable, true);
  });

  it('drops the page it started on when the run navigates somewhere itself', async () => {
    const run = await started('https://leftover.test/previous-run');
    await recorder.recordStep(BROWSER, 'navigate', { url: 'https://a.test/real-start' }, {});
    // Not two navigations: replaying through whatever the last run left on screen is
    // how a challenge wall from an unrelated site came to stop every later replay.
    assert.deepEqual(run.steps, [{ action: 'navigate', url: 'https://a.test/real-start' }]);
  });

  it('keeps the page it started on when the run acts on that page first', async () => {
    const run = await started('https://a.test/dashboard');
    recorder.setElements(BROWSER, [EL]);
    await recorder.recordStep(BROWSER, 'click', { element_id: '7' }, {});
    await recorder.recordStep(BROWSER, 'navigate', { url: 'https://a.test/next' }, {});
    assert.equal(run.steps[0].start, true);
    assert.equal(run.steps.length, 3);
  });

  it('marks a step whose element has a tag and nothing else as unaimable', async () => {
    const run = await started();
    // A tag is not a handle. This used to count as aimable and fail on replay with
    // nothing even to name: no element matching "".
    recorder.setElements(BROWSER, [{ id: 5, tag: 'input', type: 'input', text: '', visible: true }]);
    await recorder.recordStep(BROWSER, 'click', { element_id: 5 }, {});
    assert.equal(run.steps.at(-1).unaimable, true);
  });

  it('keeps a step whose element has only where it sits', async () => {
    const run = await started();
    recorder.setElements(BROWSER, [{ id: 6, tag: 'input', type: 'input', path: 'form > input', visible: true }]);
    await recorder.recordStep(BROWSER, 'click', { element_id: 6 }, {});
    assert.equal(run.steps.at(-1).unaimable, undefined);
    assert.equal(run.steps.at(-1).el.path, 'form > input');
  });

  it('records nothing on a browser with no run', () => {
    recorder.recordStep('b-never', 'click', {}, {});
    recorder.setElements('b-never', [EL]);
    assert.equal(recorder.lastRun('b-never'), null);
  });
});
