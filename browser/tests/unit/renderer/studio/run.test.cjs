/**
 * Unit tests for the Run tab (renderer/studio/run.js): only the controls that
 * work in the run's status, nothing left "running" after it ends, and a
 * timeline that is not rebuilt for nothing.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { studioApp } = require('../../support/studio.cjs');

/** Whether each run control is enabled, by name. */
const controls = ($) =>
  Object.fromEntries(
    ['pause', 'resume', 'step', 'stop'].map((name) => [
      name,
      !$('run-controls').querySelector(`[data-run="${name}"]`).disabled,
    ]),
  );

describe('the Run tab', () => {
  it('offers Pause and Stop while running, and Continue, Step and Stop while paused', async () => {
    const { ws, push, $ } = await studioApp();
    await ws.start({});
    await push();
    assert.equal($('run-controls').hidden, false);
    assert.deepEqual(controls($), { pause: true, resume: false, step: false, stop: true });
    ws.run.status = 'paused';
    await push();
    assert.deepEqual(controls($), { pause: false, resume: true, step: true, stop: true });
  });

  it('hides the controls when the run ends and shows no step still running', async () => {
    const { ws, push, $ } = await studioApp();
    await ws.start({});
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'a', status: 'running' } });
    ws.receiveFromWorker({ type: 'finished', status: 'stopped', assertions: 0 });
    await push();
    assert.equal($('run-controls').hidden, true);
    assert.doesNotMatch($('run-events').textContent, /running/);
    assert.match($('run-events').textContent, /Run stopped/);
  });

  it('shows each step once, at its latest event', async () => {
    const { ws, push, $ } = await studioApp();
    await ws.start({});
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'a', status: 'running' } });
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'a', status: 'passed', message: 'Opened' } });
    ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
    await push();
    assert.equal($('run-events').children.length, 1);
    assert.match($('run-events').textContent, /Opened/);
  });

  it('names each step in the timeline by its action', async () => {
    const { ws, push, $ } = await studioApp();
    await ws.start({});
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'b', status: 'passed' } });
    ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
    await push();
    assert.match($('run-events').textContent, /Click · passed/);
  });

  it('leaves the timeline alone when nothing in it changed', async () => {
    const { ws, push, $ } = await studioApp();
    await ws.start({});
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'a', status: 'passed' } });
    ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
    await push();
    const first = $('run-events').children[0];
    await push();
    assert.equal($('run-events').children[0], first);
  });

  it('turns off previous runs while recording', async () => {
    const { ws, push, $ } = await studioApp();
    ws.capture(ws.draft.steps, [], true);
    await push();
    assert.equal($('run-history').disabled, true);
  });

  it('applies a repair the run found to the workflow, making the target that worked the first', async () => {
    const { ws, push, $, settle } = await studioApp();
    await ws.start({});
    const replacement = { kind: 'text', value: 'Go' };
    ws.receiveFromWorker({
      type: 'repair',
      stepId: 'b',
      original: { kind: 'css', value: '#go' },
      replacement,
      draft: ws.draft,
    });
    ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
    await push();
    const apply = [...$('run-repairs').querySelectorAll('button')].find(
      (b) => b.textContent === 'Apply to this workflow',
    );
    assert.equal(apply.disabled, false);
    apply.click();
    await settle();
    const step = ws.draft.steps.find((s) => s.id === 'b');
    assert.equal(JSON.stringify(step.candidates), JSON.stringify([replacement, { kind: 'css', value: '#go' }]));
  });

  it('offers no repair to apply while the run is still going', async () => {
    const { ws, push, $ } = await studioApp();
    await ws.start({});
    const replacement = { kind: 'text', value: 'Go' };
    ws.receiveFromWorker({
      type: 'repair',
      stepId: 'b',
      original: { kind: 'css', value: '#go' },
      replacement,
      draft: ws.draft,
    });
    await push();
    const apply = [...$('run-repairs').querySelectorAll('button')].find(
      (b) => b.textContent === 'Apply to this workflow',
    );
    assert.equal(apply.disabled, true);
  });
});
