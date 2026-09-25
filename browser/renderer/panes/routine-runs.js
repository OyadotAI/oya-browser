/**
 * A routine's run history in the Routines pane: each run with when it started,
 * how long it took, how it ended, how many steps it took, and whether another
 * Oya browser on the project ran it; opening one shows the steps and the
 * agent's full answer, drawn like an Ask reply. Which runs are open survives
 * the list being redrawn on every change. Also the pill a card shows for its
 * last run.
 */
/* global Dom, Chat */
/* exported RoutineRuns */

/** The words the history shows. */
const ROUTINE_RUNS_TEXT = {
  status: { running: 'Running', done: 'Done', failed: 'Failed', stopped: 'Stopped', interrupted: 'Interrupted' },
  never: 'Never run',
  none: 'No runs yet.',
  /** What an open run says when it has no answer, by how it ended. */
  noAnswer: {
    running: 'Working on it…',
    stopped: 'Stopped before it answered.',
    interrupted: 'The browser running it closed before it finished.',
    done: 'No answer.',
    failed: 'No answer.',
  },
  steps: (n) => `${n} ${n === 1 ? 'step' : 'steps'}`,
  elsewhere: 'another browser',
};

/** Seconds in a minute, for durations. */
const SECONDS_PER_MINUTE = 60;
/** Milliseconds in a second, for durations. */
const MS_PER_SECOND = 1000;

/** The run history. */
const RoutineRuns = {
  /** Ids of the runs whose details are open. */
  open: new Set(),

  /** A status badge; a routine that never ran gets one saying so. */
  badge(status) {
    const label = Object.hasOwn(ROUTINE_RUNS_TEXT.status, status)
      ? ROUTINE_RUNS_TEXT.status[status]
      : ROUTINE_RUNS_TEXT.never;
    return Dom.node('span', label, `run-status ${status || 'never'}`);
  },

  /** A moment, as its time today, or its date and time on another day. */
  when(ts) {
    const date = new Date(ts);
    if (date.toDateString() === new Date().toDateString())
      return date.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
    return date.toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  },

  /** How long a finished run took, as "42s" or "3m 5s"; '' while it runs. */
  duration(run) {
    if (!run.finishedAt) return '';
    const seconds = Math.round((run.finishedAt - run.startedAt) / MS_PER_SECOND);
    const minutes = Math.floor(seconds / SECONDS_PER_MINUTE);
    return minutes ? `${minutes}m ${seconds % SECONDS_PER_MINUTE}s` : `${seconds}s`;
  },

  /** A card's pill for its last run: how it ended and when, "Running", or "Never run". */
  last(routine) {
    const run = routine.runs?.[0];
    const pill = RoutineRuns.badge(run?.status);
    if (run && run.status !== 'running') pill.textContent += ` · ${RoutineRuns.when(run.finishedAt || run.startedAt)}`;
    return pill;
  },

  /** The list of runs, newest first; `browserId` is this browser, so runs by another say so. */
  list(runs = [], browserId) {
    const ul = Dom.node('ul', null, 'routine-runs');
    if (!runs.length) ul.append(Dom.node('li', ROUTINE_RUNS_TEXT.none, 'routine-runs-empty'));
    for (const run of runs) ul.append(RoutineRuns.item(run, browserId));
    return ul;
  },

  /** One run: its summary line, opening to its steps and answer. */
  item(run, browserId) {
    const li = Dom.node('li', null, 'routine-run');
    const details = Dom.node('details');
    details.open = RoutineRuns.open.has(run.id);
    details.addEventListener('toggle', () => RoutineRuns.remember(run.id, details.open));
    details.append(RoutineRuns.summary(run, run.by && run.by !== browserId), RoutineRuns.body(run));
    li.append(details);
    return li;
  },

  /** Keeps whether a run is open across redraws. */
  remember(id, open) {
    if (open) RoutineRuns.open.add(id);
    else RoutineRuns.open.delete(id);
  },

  /** "9:00 AM · 42s · 5 steps", with the status badge, and "another browser" when one ran it. */
  summary(run, elsewhere) {
    const summary = Dom.node('summary');
    const parts = [RoutineRuns.when(run.startedAt), RoutineRuns.duration(run)];
    if (run.steps?.length) parts.push(ROUTINE_RUNS_TEXT.steps(run.steps.length));
    if (elsewhere) parts.push(ROUTINE_RUNS_TEXT.elsewhere);
    const when = Dom.node('span', parts.filter(Boolean).join(' · '), 'routine-run-when');
    summary.append(RoutineRuns.badge(run.status), when);
    return summary;
  },

  /** The steps the agent took, then its answer. */
  body(run) {
    const body = Dom.node('div', null, 'routine-run-body');
    if (run.steps?.length) body.append(RoutineRuns.steps(run.steps));
    const answer = Dom.node('div', null, 'routine-run-answer chat-msg assistant');
    answer.innerHTML = run.result ? Chat.mdToHtml(run.result) : Dom.esc(ROUTINE_RUNS_TEXT.noAnswer[run.status] || '');
    body.append(answer);
    return body;
  },

  /** The steps as badges, in order. */
  steps(steps) {
    const box = Dom.node('div', null, 'chat-tools');
    box.append(...steps.map((name) => Dom.node('span', name, 'chat-tool-badge')));
    return box;
  },
};
