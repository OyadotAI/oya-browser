/**
 * Unit tests for the Routines pane: the list shows each routine's last run,
 * next run and answer, its history opens to every run's steps and answer, the
 * form saves a new routine or edits one in place, a refusal is shown under
 * the form, and the row's buttons reach the scheduler.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

/** A value from the renderer's realm as a plain one. */
const plain = (value) => JSON.parse(JSON.stringify(value));

const HOUR = 3_600_000;
/** A run that finished well, 42 seconds after it started. */
const DONE_RUN = {
  id: 'run1',
  startedAt: Date.now() - HOUR,
  finishedAt: Date.now() - HOUR + 42_000,
  status: 'done',
  result: 'DONE: **3** new emails',
  steps: ['navigate', 'analyze_page', 'click'],
};
const INBOX = {
  id: 'r1',
  name: 'Inbox',
  prompt: 'Check my inbox',
  schedule: { kind: 'every', n: 2, unit: 'hours' },
  enabled: true,
  nextRunAt: Date.now() + HOUR,
  runs: [DONE_RUN, { ...DONE_RUN, id: 'run0', status: 'failed', result: 'Error: offline', steps: [] }],
};

/** A loaded renderer whose main process holds `routines`, answering each change with `after`. */
async function pane(routines = [], after = { routines, running: null }) {
  const answer = () => after;
  const answers = { listRoutines: { routines, running: null }, saveRoutine: answer, deleteRoutine: answer };
  const app = loadRenderer({ answers: { ...answers, runRoutineNow: answer } });
  await settle();
  return app;
}

/** The rows' text, one string each. */
const rows = (app) =>
  app
    .$('routines-list')
    .querySelectorAll('li')
    .map((li) => li.textContent);

describe('the Routines pane', () => {
  it('says what routines are for when there are none', async () => {
    assert.match(rows(await pane())[0], /No routines yet/);
  });

  it('shows each routine with how its last run went, when it runs next, its last answer and buttons', async () => {
    const paused = { ...INBOX, id: 'r2', name: 'Prices', schedule: { kind: 'daily', at: '07:30' }, enabled: false };
    const app = await pane([INBOX, { ...paused, runs: [], nextRunAt: null }]);
    const [inbox, prices] = rows(app);
    assert.match(
      inbox,
      /Inbox.*Done.*Every 2 hours · Next .*DONE: \*\*3\*\* new emails.*Run now.*History \(2\).*Pause/,
    );
    assert.match(prices, /Prices.*Never run.*Daily at 07:30 · Paused.*History.*Resume/);
  });

  it('says a routine that is due is due now', async () => {
    const app = await pane([{ ...INBOX, nextRunAt: Date.now() - 1 }]);
    assert.match(rows(app)[0], /Due now/);
  });

  it("opens a routine's history: each run with its status, duration and steps, and its answer inside", async () => {
    const app = await pane([INBOX]);
    const button = (label) =>
      app
        .$('routines-list')
        .querySelectorAll('.text-button')
        .find((b) => b.textContent === label);
    button('History (2)').click();
    const runs = app.$('routines-list').querySelectorAll('.routine-run');
    assert.equal(runs.length, 2);
    assert.match(runs[0].querySelector('summary').textContent, /Done.*42s · 3 steps/);
    assert.match(runs[1].querySelector('summary').textContent, /Failed/);
    assert.equal(
      runs[0].querySelector('.routine-run-answer strong').textContent,
      '3',
      'the answer is drawn as Markdown',
    );
    assert.deepEqual(
      runs[0].querySelectorAll('.chat-tool-badge').map((b) => b.textContent),
      ['navigate', 'analyze_page', 'click'],
    );
    button('Hide history').click();
    assert.equal(app.$('routines-list').querySelectorAll('.routine-run').length, 0);
  });

  it('keeps a history and an opened run open when the list is redrawn', async () => {
    const app = await pane([INBOX]);
    app
      .$('routines-list')
      .querySelectorAll('.text-button')
      .find((b) => b.textContent === 'History (2)')
      .click();
    const details = app.$('routines-list').querySelector('.routine-run details');
    details.open = true;
    app.fire(details, 'toggle');
    app.bridge.emit('RoutinesChanged', { routines: [INBOX], running: null });
    assert.equal(app.$('routines-list').querySelector('.routine-run details').open, true);
  });

  it('says why a run has no answer: stopped, or cut short by quitting', async () => {
    const runs = [
      { ...DONE_RUN, id: 'a', status: 'stopped', result: '' },
      { ...DONE_RUN, id: 'b', status: 'interrupted', result: '', finishedAt: undefined },
    ];
    const app = await pane([{ ...INBOX, runs }]);
    app
      .$('routines-list')
      .querySelectorAll('.text-button')
      .find((b) => /History/.test(b.textContent))
      .click();
    const answers = app
      .$('routines-list')
      .querySelectorAll('.routine-run-answer')
      .map((a) => a.textContent);
    assert.deepEqual(answers, ['Stopped before it answered.', 'Oya closed before this run finished.']);
  });

  it('escapes what a run answered', async () => {
    const app = await pane([{ ...INBOX, runs: [{ ...DONE_RUN, result: '<img src=x onerror=alert(1)>' }] }]);
    app
      .$('routines-list')
      .querySelectorAll('.text-button')
      .find((b) => /History/.test(b.textContent))
      .click();
    assert.equal(app.$('routines-list').querySelector('.routine-run-answer img'), null);
  });

  it('says a routine is running, and offers Stop for it', async () => {
    const app = await pane();
    const running = { ...INBOX, runs: [{ id: 'run2', startedAt: Date.now(), status: 'running' }, ...INBOX.runs] };
    app.bridge.emit('RoutinesChanged', { routines: [running], running: 'r1' });
    assert.match(rows(app)[0], /Inbox.*Running.*Stop/);
    app.$('routines-list').querySelector('.text-button').click();
    assert.equal(app.bridge.called('stopChat').length, 1);
  });

  it('saves a new routine from the form, then empties it', async () => {
    const app = await pane([], { routines: [INBOX], running: null });
    app.$('routine-name').value = 'Inbox';
    app.$('routine-prompt').value = 'Check my inbox';
    app.$('routine-n').value = '2';
    app.fire(app.$('routine-form'), 'submit');
    await settle();
    assert.deepEqual(plain(app.bridge.called('saveRoutine')[0][0]), {
      name: 'Inbox',
      prompt: 'Check my inbox',
      enabled: true,
      schedule: { kind: 'every', n: 2, unit: 'hours' },
    });
    assert.equal(app.$('routine-name').value, '');
    assert.match(rows(app)[0], /Inbox/);
  });

  it('switches the form to a time of day for a daily routine', async () => {
    const app = await pane();
    app.$('routine-kind').value = 'daily';
    app.fire(app.$('routine-kind'), 'change');
    assert.deepEqual([app.$('routine-n').hidden, app.$('routine-at').hidden], [true, false]);
    app.$('routine-at').value = '06:45';
    assert.deepEqual(plain(app.run('Routines.schedule()')), { kind: 'daily', at: '06:45' });
  });

  it('edits a routine in place, keeping its id and whether it is paused', async () => {
    const app = await pane([{ ...INBOX, enabled: false }]);
    const edit = app
      .$('routines-list')
      .querySelectorAll('.text-button')
      .find((b) => b.textContent === 'Edit');
    edit.click();
    assert.equal(app.$('routine-name').value, 'Inbox');
    assert.equal(app.$('routine-form-title').textContent, 'Edit routine');
    app.$('routine-name').value = 'Inbox twice';
    app.fire(app.$('routine-form'), 'submit');
    await settle();
    const saved = plain(app.bridge.called('saveRoutine')[0][0]);
    assert.deepEqual([saved.id, saved.name, saved.enabled], ['r1', 'Inbox twice', false]);
  });

  it('shows why a routine was refused, keeping what was typed', async () => {
    const app = loadRenderer({
      answers: {
        saveRoutine: () => {
          throw new Error(
            "Error invoking remote method 'save-routine': Error: A routine needs a name, a prompt and a valid schedule.",
          );
        },
      },
    });
    await settle();
    app.$('routine-name').value = 'Inbox';
    app.fire(app.$('routine-form'), 'submit');
    await settle();
    assert.equal(app.$('routine-error').hidden, false);
    assert.equal(app.$('routine-error').textContent, 'A routine needs a name, a prompt and a valid schedule.');
    assert.equal(app.$('routine-name').value, 'Inbox');
  });

  it('runs, pauses and deletes a routine from its row', async () => {
    const app = await pane([INBOX]);
    const button = (label) =>
      app
        .$('routines-list')
        .querySelectorAll('.text-button')
        .find((b) => b.textContent === label);
    button('Run now').click();
    button('Pause').click();
    button('Delete').click();
    await settle();
    assert.deepEqual(app.bridge.called('runRoutineNow'), [['r1']]);
    assert.equal(plain(app.bridge.called('saveRoutine')[0][0]).enabled, false);
    assert.deepEqual(app.bridge.called('deleteRoutine'), [['r1']]);
  });
});
