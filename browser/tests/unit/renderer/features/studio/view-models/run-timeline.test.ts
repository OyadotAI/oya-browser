/**
 * Unit tests for the Run tab (run-timeline.ts, run-format.ts): only the
 * controls that work in the run's status, nothing left "running" after it
 * ends, each step once, repairs, previous runs, and a timeline event opening
 * its step.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { studioApp } from '../studio-app.ts';
import { isActive, isDisabled } from '../../../../../../src/renderer/features/studio/model/studio-model.ts';
import {
  canApply,
  controlEnabled,
  eventText,
  runSummary,
  runTitle,
  shownEvents,
} from '../../../../../../src/renderer/features/studio/model/run-format.ts';
import { RUN_CONTROLS } from '../../../../../../src/renderer/features/studio/model/constants.ts';
import type { Run } from '../../../../../../src/renderer/features/studio/model/types.ts';

/** Whether each run control is enabled, by name. */
const controls = (run: Run | null) =>
  Object.fromEntries(RUN_CONTROLS.map(({ command }) => [command, controlEnabled(run, command)]));

/** A finished run with a repair found for step b. */
async function repaired(finish = true) {
  const app = await studioApp();
  await app.ws.start({});
  const replacement = { kind: 'text', value: 'Go' };
  const original = { kind: 'css', value: '#go' };
  app.ws.receiveFromWorker({ type: 'repair', stepId: 'b', original, replacement, draft: app.ws.draft });
  if (finish) app.ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
  await app.push();
  return { ...app, replacement, original };
}

describe('RunTimeline', () => {
  it('offers Pause and Stop while running, and Continue, Step and Stop while paused', async () => {
    const { vm, ws, push } = await studioApp();
    await ws.start({});
    await push();
    assert.equal(isActive(vm.state.snapshot!.run), true, 'the controls show');
    assert.deepEqual(controls(vm.state.snapshot!.run), { pause: true, resume: false, step: false, stop: true });
    ws.run.status = 'paused';
    await push();
    assert.deepEqual(controls(vm.state.snapshot!.run), { pause: false, resume: true, step: true, stop: true });
  });

  it('hides the controls when the run ends and shows no step still running', async () => {
    const { vm, ws, push } = await studioApp();
    await ws.start({});
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'a', status: 'running' } });
    ws.receiveFromWorker({ type: 'finished', status: 'stopped', assertions: 0 });
    await push();
    const run = vm.state.snapshot!.run;
    assert.equal(isActive(run), false);
    const texts = shownEvents(run).map((e) => eventText(e, vm.state.snapshot!.draft.steps));
    assert.deepEqual(texts, ['Run stopped.']);
  });

  it('shows each step once, at its latest event', async () => {
    const { vm, ws, push } = await studioApp();
    await ws.start({});
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'a', status: 'running' } });
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'a', status: 'passed', message: 'Opened' } });
    ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
    await push();
    const shown = shownEvents(vm.state.snapshot!.run);
    assert.equal(shown.length, 1);
    assert.equal(shown[0].message, 'Opened');
  });

  it('names each step in the timeline by its action', async () => {
    const { vm, ws, push } = await studioApp();
    await ws.start({});
    ws.receiveFromWorker({ type: 'event', event: { kind: 'step', stepId: 'b', status: 'passed' } });
    ws.receiveFromWorker({ type: 'finished', status: 'succeeded', assertions: 0 });
    await push();
    const [event] = shownEvents(vm.state.snapshot!.run);
    assert.equal(eventText(event, vm.state.snapshot!.draft.steps), 'Click · passed');
  });

  it('titles and sums up the run: idle, a raw status, an error, assertions passed', () => {
    const run = (fields: Partial<Run>): Run => ({
      id: 'r',
      draftId: 'd',
      status: 'running',
      events: [],
      repairs: [],
      ...fields,
    });
    assert.equal(runTitle(null), 'Test this workflow');
    assert.equal(runTitle(run({ status: 'odd' })), 'odd');
    assert.equal(runSummary(run({ status: 'failed', error: 'Boom' })), 'Boom');
    assert.equal(runSummary(run({ status: 'succeeded', assertions: 2 })), '2 assertions passed.');
    assert.match(runSummary(run({ status: 'succeeded' })), /Add assertions/);
  });

  it('turns off previous runs while recording', async () => {
    const { vm, ws, push } = await studioApp();
    ws.capture(ws.draft.steps, [], true);
    await push();
    assert.equal(isDisabled(vm.state, 'run-history'), true);
  });

  it('opens a previous run, and does nothing for the list’s placeholder', async () => {
    const { vm, fake } = await studioApp({ commands: { 'open-run': () => undefined } });
    await vm.run.openRun('');
    await vm.run.openRun('r1');
    const opened = fake.called('workspace').filter(([cmd]) => (cmd as any).type === 'open-run');
    assert.deepEqual(opened, [[{ type: 'open-run', id: 'r1' }]]);
  });

  it('pauses the run through its control', async () => {
    const { vm, ws, push } = await studioApp();
    await ws.start({});
    await push();
    await vm.run.control('stop');
    assert.equal(vm.state.snapshot!.run!.status, 'stopping');
  });

  it('opens a timeline event’s step on the Steps tab', async () => {
    const { vm } = await studioApp();
    vm.selectTab('run');
    vm.run.openStep('b');
    assert.deepEqual([vm.state.selected, vm.state.tab], ['b', 'steps']);
  });

  it('applies a repair the run found to the workflow, making the target that worked the first', async () => {
    const { vm, ws, replacement, original } = await repaired();
    const repair = vm.state.snapshot!.run!.repairs[0];
    assert.equal(canApply(vm.state.snapshot!, repair), true);
    await vm.run.applyRepair(repair);
    const step = ws.draft.steps.find((s: any) => s.id === 'b');
    assert.deepEqual(step.candidates, [replacement, original]);
    assert.equal(canApply(vm.state.snapshot!, repair), false, 'already first');
  });

  it('offers no repair to apply while the run is still going', async () => {
    const { vm, fake } = await repaired(false);
    const repair = vm.state.snapshot!.run!.repairs[0];
    assert.equal(canApply(vm.state.snapshot!, repair), false);
    await vm.run.applyRepair(repair);
    assert.equal(fake.called('workspace').filter(([cmd]) => (cmd as any).type === 'update').length, 0);
  });

  it('reviews a repair as a copy, on the Steps tab', async () => {
    const { vm } = await repaired();
    const repair = vm.state.snapshot!.run!.repairs[0];
    vm.selectTab('run');
    await vm.run.reviewRepair(repair);
    assert.equal(vm.state.snapshot!.draft.id, repair.draftId);
    assert.equal(vm.state.tab, 'steps');
  });
});
