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
    recorder.recordStep(BROWSER, 'type', { element_id: '7', text: '{{email}}' }, { email: 'ada@x.test' });
    assert.deepEqual(run.steps[1], {
      action: 'type',
      text: '{{email}}',
      el: {
        type: 'input',
        tag: 'input',
        text: '{{email}}',
        domId: undefined,
        name: 'email',
        ariaLabel: undefined,
        testId: undefined,
        placeholder: undefined,
        href: undefined,
      },
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

  it('copies replayable arguments and ignores tools that cannot replay', async () => {
    const run = await started();
    recorder.recordStep(BROWSER, 'scroll', { direction: 'down', amount: 300, junk: 1 }, {});
    recorder.recordStep(BROWSER, 'handle_dialog', { accept: false }, {});
    recorder.recordStep(BROWSER, 'click_coordinates', { x: 1, y: 2 }, {});
    recorder.recordStep(BROWSER, 'analyze_page', {}, {});
    assert.deepEqual(run.steps.slice(1), [
      { action: 'scroll', direction: 'down', amount: 300 },
      { action: 'handle_dialog', accept: false },
    ]);
  });

  it('records nothing on a browser with no run', () => {
    recorder.recordStep('b-never', 'click', {}, {});
    recorder.setElements('b-never', [EL]);
    assert.equal(recorder.lastRun('b-never'), null);
  });
});
