/**
 * Unit tests for the Routines pane: each card says in one line what its
 * routine is doing, the switch and buttons reach the main process, Run now says
 * why it cannot start, Stop is offered only for a run on this browser, Delete
 * asks first, history opens on the latest run, and the editor saves a new
 * routine or an edit and keeps what was typed when the server refuses it.
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
  by: 'b1',
};
const INBOX = {
  id: 'r1',
  name: 'Inbox',
  prompt: 'Check my inbox',
  schedule: { kind: 'every', n: 2, unit: 'hours' },
  enabled: true,
  nextRunAt: Date.now() + HOUR,
  runs: [DONE_RUN, { ...DONE_RUN, id: 'run0', status: 'failed', result: 'Error: offline', steps: [], by: 'b2' }],
};

/** The main process's snapshot of `routines`, online and free. */
const snapshot = (routines, extra = {}) => ({
  routines,
  running: null,
  browserId: 'b1',
  online: true,
  busy: '',
  ...extra,
});

/** A loaded renderer whose main process holds `state`, answering each change with `after` (or the same list). */
async function pane(state = snapshot([]), after = state) {
  const answer = () => after;
  const names = ['saveRoutine', 'deleteRoutine', 'runRoutineNow', 'setRoutineEnabled', 'clearRoutineHistory'];
  const answers = { listRoutines: state, stopRoutine: true, ...Object.fromEntries(names.map((n) => [n, answer])) };
  const app = loadRenderer({ answers });
  await settle();
  return app;
}

/** The first card. */
const card = (app) => app.$('routines-list').querySelector('.routine-card');
/** A button in the first card, by its class. */
const button = (app, cls) => card(app).querySelector(`.${cls}`);
/** The first card's status line. */
const status = (app) => card(app).querySelector('.routine-status').textContent;

describe('the Routines pane', () => {
  it('invites a first routine on an empty project', async () => {
    const app = await pane();
    assert.match(app.$('routines-list').textContent, /No routines yet/);
  });

  it('says what a routine is doing in one line, with its last run as a pill', async () => {
    const app = await pane(snapshot([INBOX]));
    assert.match(status(app), /^Every 2 hours · Next /);
    assert.match(card(app).querySelector('.run-status').textContent, /^Done · /);
    assert.equal(button(app, 'switch').getAttribute('aria-checked'), 'true');
  });

  it('says a routine is off, due and waiting with the reason, or running here or elsewhere', async () => {
    const due = { ...INBOX, nextRunAt: Date.now() - 1 };
    const waiting = await pane(snapshot([due], { busy: 'Finish recording first.' }));
    assert.equal(status(waiting), 'Every 2 hours · Waiting: Finish recording first.');
    assert.equal(button(waiting, 'routine-action').disabled, true);
    const off = await pane(snapshot([{ ...INBOX, enabled: false, nextRunAt: null }]));
    assert.equal(status(off), 'Every 2 hours · Off');
    const running = { ...INBOX, runs: [{ id: 'now', status: 'running', startedAt: Date.now(), by: 'b2' }] };
    const elsewhere = await pane(snapshot([running]));
    assert.equal(status(elsewhere), 'Running on another Oya browser');
    assert.equal(button(elsewhere, 'routine-action'), null, 'no Run now while another browser runs it');
  });

  it('offers Stop only for a run on this browser, and stops that routine', async () => {
    const running = { ...INBOX, runs: [{ id: 'now', status: 'running', startedAt: Date.now(), by: 'b1' }] };
    const app = await pane(snapshot([running], { running: 'r1' }));
    assert.match(status(app), /^Running on this browser · /);
    app.fire(button(app, 'routine-action'), 'click');
    await settle();
    assert.deepEqual(plain(app.bridge.called('stopRoutine')), [['r1']]);
  });

  it('turns a routine off with the switch, and runs it now', async () => {
    const app = await pane(snapshot([INBOX]));
    app.fire(button(app, 'switch'), 'click');
    app.fire(button(app, 'routine-action'), 'click');
    await settle();
    assert.deepEqual(plain(app.bridge.called('setRoutineEnabled')), [['r1', false]]);
    assert.deepEqual(plain(app.bridge.called('runRoutineNow')), [['r1']]);
  });

  it('shows why Run now could not start under the routine', async () => {
    const app = await pane(snapshot([INBOX]), { error: 'Another browser already ran this routine.' });
    app.fire(button(app, 'routine-action'), 'click');
    await settle();
    assert.equal(card(app).querySelector('.routine-note').textContent, 'Another browser already ran this routine.');
  });

  it('opens history on the latest run, and says which runs another browser made', async () => {
    const app = await pane(snapshot([INBOX]));
    app.fire(button(app, 'routine-history-toggle'), 'click');
    const runs = card(app).querySelectorAll('.routine-run details');
    assert.equal(runs.length, 2);
    assert.equal(runs[0].open, true);
    assert.match(runs[0].textContent, /3 steps/);
    assert.match(runs[1].textContent, /another browser/);
  });

  it('asks before deleting, and deletes on Delete', async () => {
    const app = await pane(snapshot([INBOX]), snapshot([]));
    app.fire(button(app, 'routine-more'), 'click');
    const items = card(app).querySelectorAll('[role="menuitem"]');
    assert.deepEqual(
      items.map((i) => i.textContent),
      ['Edit', 'Clear history', 'Delete'],
    );
    app.fire(items[2], 'click');
    assert.equal(app.bridge.called('deleteRoutine').length, 0);
    app.fire(card(app).querySelector('.routine-confirm .routine-action'), 'click');
    await settle();
    assert.deepEqual(plain(app.bridge.called('deleteRoutine')), [['r1']]);
    assert.match(app.$('routines-list').textContent, /No routines yet/);
  });

  it('says so when offline', async () => {
    const app = await pane(snapshot([], { online: false }));
    assert.equal(app.$('routines-banner').hidden, false);
    assert.match(app.$('routines-banner').textContent, /Offline/);
  });
});

describe('the routine editor', () => {
  it('saves a new daily routine, on', async () => {
    const app = await pane(snapshot([]), snapshot([INBOX]));
    app.fire(app.$('routine-new'), 'click');
    app.$('routine-name').value = 'Standup';
    app.$('routine-prompt').value = 'Summarize Slack';
    app.fire(app.$('routine-kind').querySelector('[data-kind="daily"]'), 'click', { bubbles: true });
    app.$('routine-at').value = '08:30';
    app.fire(app.$('routine-form'), 'submit');
    await settle();
    const [[saved]] = plain(app.bridge.called('saveRoutine'));
    assert.deepEqual(saved, {
      name: 'Standup',
      prompt: 'Summarize Slack',
      enabled: true,
      schedule: { kind: 'daily', at: '08:30' },
    });
    assert.equal(app.$('routine-form').hidden, true);
  });

  it('edits a routine in place, keeping its id and whether it is on', async () => {
    const app = await pane(snapshot([{ ...INBOX, enabled: false }]));
    app.fire(button(app, 'routine-more'), 'click');
    app.fire(card(app).querySelector('[role="menuitem"]'), 'click');
    assert.equal(app.$('routine-name').value, 'Inbox');
    assert.equal(app.$('routine-form-title').textContent, 'Edit routine');
    app.fire(app.$('routine-form'), 'submit');
    await settle();
    const [[saved]] = plain(app.bridge.called('saveRoutine'));
    assert.deepEqual([saved.id, saved.enabled, saved.schedule], ['r1', false, INBOX.schedule]);
  });

  it('keeps what was typed and says why when the server refuses it', async () => {
    const app = await pane(snapshot([]), { error: '"Daily" runs at a time written HH:MM.' });
    app.fire(app.$('routine-new'), 'click');
    app.$('routine-name').value = 'Standup';
    app.fire(app.$('routine-form'), 'submit');
    await settle();
    assert.equal(app.$('routine-error').textContent, '"Daily" runs at a time written HH:MM.');
    assert.equal(app.$('routine-form').hidden, false);
    assert.equal(app.$('routine-name').value, 'Standup');
  });

  it('shows a routine name as text, never as markup', async () => {
    const app = await pane(snapshot([{ ...INBOX, name: '<img src=x onerror=alert(1)>' }]));
    assert.equal(card(app).querySelectorAll('img').length, 0);
    assert.match(card(app).textContent, /<img src=x/);
  });
});
