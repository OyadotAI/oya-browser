/**
 * Unit tests for the Routines pane's ViewModel and its wording: each routine
 * says in one line what it is doing, the switch and buttons reach the main
 * process, Run now says why it cannot start (and the note goes), Delete asks
 * first and stops an edit of it, history opens on the latest run, the menu
 * offers Clear history only when there is some, and the clock redraws times
 * only while the pane shows.
 */
import { describe, it, mock, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { RoutinesViewModel } from '../../../../../../src/renderer/features/routines/view-models/routines-view-model.ts';
import {
  phaseOf,
  statusLine,
  lastRunLabel,
  runSummary,
  answerHtml,
  hasFinishedRuns,
  duration,
  scheduleOf,
} from '../../../../../../src/renderer/features/routines/model/routine-format.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { ViewModel } from '../../../../../../src/renderer/core/view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

const HOUR = 3_600_000;
const NOW = Date.UTC(2026, 9, 4, 12);
/** A run that finished well, 42 seconds after it started. */
const DONE_RUN = {
  id: 'run1',
  startedAt: NOW - HOUR,
  finishedAt: NOW - HOUR + 42_000,
  status: 'done',
  result: 'DONE: **3** new emails',
  steps: ['navigate', 'analyze_page', 'click'],
  by: 'b1',
};
const INBOX = {
  id: 'r1',
  name: 'Inbox',
  prompt: 'Check my inbox',
  schedule: { kind: 'every', n: 2, unit: 'hours' },
  enabled: true,
  nextRunAt: NOW + HOUR,
  runs: [DONE_RUN, { ...DONE_RUN, id: 'run0', status: 'failed', result: 'Error: offline', steps: [], by: 'b2' }],
};

/** The main process's snapshot of `routines`, online and free. */
const snapshot = (routines: unknown[], extra = {}) => ({
  routines,
  running: null,
  browserId: 'b1',
  online: true,
  busy: '',
  error: '',
  ...extra,
});

/** What the fake panel holds. */
interface PanelState {
  /** The pane in view. */
  pane: string;
}

/** A stand-in for the workspace panel. */
class FakePanel extends ViewModel<PanelState> {
  /** Shows `pane`. */
  show(pane: string) {
    this.set({ pane });
  }
}

/** Lets pending promises settle. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

/** A pane whose main process holds `state`, answering each change with `after`. */
async function pane(state: unknown = snapshot([]), after: unknown = state, shown = 'chat') {
  const answer = () => after;
  const names = ['saveRoutine', 'deleteRoutine', 'runRoutineNow', 'setRoutineEnabled', 'clearRoutineHistory'];
  const fake = fakeBridge({
    listRoutines: state,
    stopRoutine: true,
    ...Object.fromEntries(names.map((n) => [n, answer])),
  });
  const panel = new FakePanel({ pane: shown });
  const vm = new RoutinesViewModel({ bridge: fake.bridge, panel });
  await settle();
  return { fake, panel, vm };
}

afterEach(() => mock.timers.reset());

describe('routine wording', () => {
  it('says what a routine is doing in one line, with its last run as a pill', () => {
    const snap = snapshot([INBOX]);
    assert.equal(phaseOf(INBOX, null, NOW), 'scheduled');
    assert.match(statusLine(INBOX, 'scheduled', snap, NOW), /^Every 2 hours · Next /);
    assert.match(lastRunLabel(INBOX, NOW), /^Done · /);
    assert.equal(lastRunLabel({ ...INBOX, runs: [] }, NOW), 'Never run');
  });

  it('says a routine is due, or waiting with the reason this app cannot run it', () => {
    const due = { ...INBOX, nextRunAt: NOW - 1 };
    assert.equal(phaseOf(due, null, NOW), 'due');
    assert.equal(statusLine(due, 'due', snapshot([due]), NOW), 'Every 2 hours · Due now');
    const busy = snapshot([due], { busy: 'Finish recording first.' });
    assert.equal(statusLine(due, 'due', busy, NOW), 'Every 2 hours · Waiting: Finish recording first.');
  });

  it('says a routine is off when its schedule is off or unknown', () => {
    const off = { ...INBOX, enabled: false, nextRunAt: null };
    assert.equal(phaseOf(off, null, NOW), 'off');
    assert.equal(statusLine(off, 'off', snapshot([off]), NOW), 'Every 2 hours · Off');
    assert.equal(scheduleOf({ ...INBOX, schedule: { kind: 'weekly' } }), '');
  });

  it('names an hourly and a daily schedule', () => {
    assert.equal(scheduleOf({ ...INBOX, schedule: { kind: 'every', n: 1, unit: 'hours' } }), 'Every hour');
    assert.equal(scheduleOf({ ...INBOX, schedule: { kind: 'daily', at: '08:30' } }), 'Daily at 08:30');
  });

  it('tells a run here, with its elapsed time, from a run on another browser', () => {
    const running = { ...INBOX, runs: [{ id: 'now', status: 'running', startedAt: NOW - 65_000, by: 'b1' }] };
    assert.equal(phaseOf(running, 'r1', NOW), 'here');
    assert.equal(statusLine(running, 'here', snapshot([running]), NOW), 'Running on this browser · 1m 5s');
    assert.equal(phaseOf(running, null, NOW), 'elsewhere');
    assert.equal(statusLine(running, 'elsewhere', snapshot([running]), NOW), 'Running on another Oya browser');
  });

  it('sums a run up with its steps, and says when another browser ran it', () => {
    assert.match(runSummary(DONE_RUN, 'b1', NOW), / · 42s · 3 steps$/);
    assert.match(runSummary(INBOX.runs[1], 'b1', NOW), /another browser$/);
    assert.equal(duration({ startedAt: NOW }), '');
  });

  it('draws an answer as escaped Markdown, and says why a run has none', () => {
    assert.match(answerHtml(DONE_RUN), /<strong>3<\/strong>/);
    assert.match(answerHtml({ ...DONE_RUN, result: '<img src=x>' }), /&lt;img/);
    assert.equal(answerHtml({ ...DONE_RUN, result: '', status: 'stopped' }), 'Stopped before it answered.');
  });

  it('offers Clear history only when some run has finished', () => {
    assert.equal(hasFinishedRuns(INBOX), true);
    assert.equal(hasFinishedRuns({ ...INBOX, runs: [{ id: 'x', status: 'running', startedAt: NOW }] }), false);
  });
});

describe('RoutinesViewModel', () => {
  it('loads the list from the main process, and follows its changes', async () => {
    const { fake, vm } = await pane(snapshot([]));
    assert.deepEqual(vm.state.snapshot.routines, []);
    fake.emit('onRoutinesChanged', snapshot([INBOX]));
    assert.equal(vm.state.snapshot.routines[0].id, 'r1');
  });

  it('says why the list could not be read in the banner', async () => {
    const { vm } = await pane({ error: 'server down' });
    assert.equal(vm.state.snapshot.error, 'server down');
  });

  it('keeps the offline snapshot, so the banner says so', async () => {
    const { vm } = await pane(snapshot([], { online: false }));
    assert.equal(vm.state.snapshot.online, false);
  });

  it('stops this browser’s run of a routine', async () => {
    const { fake, vm } = await pane(snapshot([INBOX], { running: 'r1' }));
    await vm.stop('r1');
    assert.deepEqual(fake.called('stopRoutine'), [['r1']]);
  });

  it('turns a routine off with the switch, and runs it now', async () => {
    const { fake, vm } = await pane(snapshot([INBOX]));
    await vm.setEnabled(INBOX);
    await vm.runNow('r1');
    assert.deepEqual(fake.called('setRoutineEnabled'), [['r1', false]]);
    assert.deepEqual(fake.called('runRoutineNow'), [['r1']]);
  });

  it('shows why Run now could not start under the routine, for a few seconds', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const { vm } = await pane(snapshot([INBOX]), { error: 'Another browser already ran this routine.' });
    await vm.runNow('r1');
    assert.equal(vm.state.notes.r1, 'Another browser already ran this routine.');
    mock.timers.tick(C.ROUTINE_NOTE_MS);
    assert.equal(Object.hasOwn(vm.state.notes, 'r1'), false);
  });

  it('opens history on the latest run, and closes it again', async () => {
    const { vm } = await pane(snapshot([INBOX]));
    vm.toggleHistory(INBOX);
    assert.deepEqual([vm.state.expanded, vm.state.openRuns], [['r1'], ['run1']]);
    vm.setRunOpen('run0', true);
    vm.setRunOpen('run1', false);
    assert.deepEqual(vm.state.openRuns, ['run0']);
    vm.toggleHistory(INBOX);
    assert.deepEqual(vm.state.expanded, []);
  });

  it('opens and closes a routine’s menu, and closes it on a press outside', async () => {
    const { vm } = await pane(snapshot([INBOX]));
    vm.toggleMenu('r1');
    assert.equal(vm.state.menu, 'r1');
    vm.closeMenu();
    assert.equal(vm.state.menu, null);
  });

  it('asks before deleting, deletes on Delete, and stops editing it', async () => {
    const { fake, vm } = await pane(snapshot([INBOX]), snapshot([]));
    vm.toggleMenu('r1');
    vm.edit(INBOX);
    vm.askDelete('r1');
    assert.deepEqual([vm.state.confirming, vm.state.menu, fake.called('deleteRoutine').length], ['r1', null, 0]);
    await vm.remove('r1');
    assert.deepEqual(fake.called('deleteRoutine'), [['r1']]);
    assert.deepEqual([vm.state.confirming, vm.state.snapshot.routines, vm.editor.state.open], [null, [], false]);
  });

  it('cancels a delete', async () => {
    const { vm } = await pane(snapshot([INBOX]));
    vm.askDelete('r1');
    vm.askDelete(null);
    assert.equal(vm.state.confirming, null);
  });

  it('clears a routine’s history', async () => {
    const { fake, vm } = await pane(snapshot([INBOX]));
    await vm.clearHistory('r1');
    assert.deepEqual(fake.called('clearRoutineHistory'), [['r1']]);
  });

  it('redraws times on a clock only while the pane shows', async () => {
    mock.timers.enable({ apis: ['setInterval', 'Date'], now: NOW });
    const { panel, vm } = await pane(snapshot([INBOX]));
    panel.show('routines');
    assert.equal(vm.state.now, NOW);
    mock.timers.tick(C.ROUTINES_CLOCK_MS);
    assert.equal(vm.state.now, NOW + C.ROUTINES_CLOCK_MS);
    panel.show('chat');
    mock.timers.tick(C.ROUTINES_CLOCK_MS);
    assert.equal(vm.state.now, NOW + C.ROUTINES_CLOCK_MS);
  });

  it('stops listening once disposed', async () => {
    const { fake, vm } = await pane();
    vm.dispose();
    assert.equal(fake.listeners('onRoutinesChanged'), 0);
  });
});

describe('the routine editor', () => {
  it('saves a new daily routine, on, and closes', async () => {
    const { fake, vm } = await pane(snapshot([]), snapshot([INBOX]));
    vm.edit(null);
    vm.editor.setField('name', 'Standup');
    vm.editor.setField('prompt', 'Summarize Slack');
    vm.editor.setKind('daily');
    vm.editor.setField('at', '08:30');
    await vm.editor.submit();
    assert.deepEqual(fake.called('saveRoutine'), [
      [{ name: 'Standup', prompt: 'Summarize Slack', enabled: true, schedule: { kind: 'daily', at: '08:30' } }],
    ]);
    assert.equal(vm.editor.state.open, false);
    assert.equal(vm.state.snapshot.routines[0].id, 'r1');
  });

  it('saves an every-N schedule with its number', async () => {
    const { fake, vm } = await pane();
    vm.edit(null);
    vm.editor.setField('n', '15');
    vm.editor.setField('unit', 'minutes');
    await vm.editor.submit();
    assert.deepEqual((fake.called('saveRoutine')[0][0] as any).schedule, {
      kind: 'every',
      n: 15,
      unit: 'minutes',
    });
  });

  it('edits a routine in place, keeping its id and whether it is on', async () => {
    const { fake, vm } = await pane(snapshot([{ ...INBOX, enabled: false }]));
    vm.edit({ ...INBOX, enabled: false });
    assert.deepEqual([vm.editor.state.name, !!vm.editor.state.editing], ['Inbox', true]);
    await vm.editor.submit();
    const [[saved]] = fake.called('saveRoutine') as any[][];
    assert.deepEqual([saved.id, saved.enabled, saved.schedule], ['r1', false, INBOX.schedule]);
  });

  it('keeps what was typed and says why when the server refuses it', async () => {
    const { vm } = await pane(snapshot([]), { error: '"Daily" runs at a time written HH:MM.' });
    vm.edit(null);
    vm.editor.setField('name', 'Standup');
    await vm.editor.submit();
    assert.deepEqual(
      [vm.editor.state.error, vm.editor.state.open, vm.editor.state.name],
      ['"Daily" runs at a time written HH:MM.', true, 'Standup'],
    );
  });

  it('says why a failed call was refused, without Electron’s prefix', async () => {
    const { fake, vm } = await pane();
    fake.bridge.saveRoutine = async () => {
      throw new Error("Error invoking remote method 'save-routine': Error: Name too long");
    };
    vm.edit(null);
    await vm.editor.submit();
    assert.equal(vm.editor.state.error, 'Name too long');
  });

  it('closes on Cancel', async () => {
    const { vm } = await pane();
    vm.edit(null);
    vm.editor.close();
    assert.equal(vm.editor.state.open, false);
  });
});
