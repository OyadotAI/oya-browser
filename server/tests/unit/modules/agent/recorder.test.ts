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

  it('marks a step whose element nothing can find again, rather than aiming it at the body', async () => {
    const run = await started();
    recorder.setElements(BROWSER, []);
    await recorder.recordStep(BROWSER, 'click', { element_id: 99 }, {});
    const step = run.steps.at(-1);
    assert.equal(step.el, undefined);
    assert.equal(step.unaimable, true);
  });

  it('records nothing on a browser with no run', () => {
    recorder.recordStep('b-never', 'click', {}, {});
    recorder.setElements('b-never', [EL]);
    assert.equal(recorder.lastRun('b-never'), null);
  });
});
