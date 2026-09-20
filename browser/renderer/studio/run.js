/**
 * The studio's Run tab: the run's status and summary, its controls, the
 * timeline of step events, repair drafts to review, previous runs, and the
 * diagnostics preview.
 */
/* global Dom, RendererConstants, Studio, StudioView */
/* exported RunView */

/** The run view. */
const RunView = {
  /** The heading for each run status. */
  TITLES: {
    succeeded: 'Steps completed.',
    failed: 'Needs your attention.',
    'outcome-unknown': 'Check the website before retrying.',
    paused: 'Paused at a step.',
    running: 'Validating workflow…',
    starting: 'Preparing a fresh tab…',
    interrupted: 'Run interrupted.',
    stopping: 'Stopping…',
    stopped: 'Run stopped.',
  },

  /** Event kinds the timeline shows. */
  SHOWN: ['step', 'attention', 'target'],

  /** Draws the run. */
  render() {
    const run = Studio.state.run;
    RunView.history(run);
    Dom.byId('run-title').textContent = RunView.title(run);
    Dom.byId('run-summary').textContent = RunView.summary(run);
    RunView.controls(run);
    Dom.byId('run-events').replaceChildren(...RunView.shownEvents(run).map(RunView.event));
    Dom.byId('run-repairs').replaceChildren(...(run?.repairs || []).map(RunView.repair));
    RunView.support();
  },

  /** The diagnostics preview: the same centrally redacted object the main process writes. */
  support() {
    const json = JSON.stringify(Studio.state.support || {}, null, RendererConstants.JSON_INDENT);
    Dom.byId('support-preview').textContent = json;
  },

  /** The heading: the status's title, the raw status if it has none, or the idle prompt. */
  title(run) {
    if (!run) return 'Ready when you are.';
    return (Object.hasOwn(RunView.TITLES, run.status) ? RunView.TITLES[run.status] : undefined) || run.status;
  },

  /** Pause, step and stop work only while a run is under way. */
  controls(run) {
    const live = !!run && ['starting', 'running', 'paused'].includes(run.status);
    document.querySelectorAll('[data-run]').forEach((el) => (el.disabled = !live));
  },

  /** The latest events of the kinds the timeline shows. */
  shownEvents(run) {
    return (run?.events || []).filter((e) => RunView.SHOWN.includes(e.kind)).slice(-RendererConstants.RUN_EVENTS_SHOWN);
  },

  /** The previous-runs list, redrawn only when it changed, with this run selected. */
  history(run) {
    const signature = JSON.stringify(Studio.state.runHistory);
    if (signature !== Studio.signatures.runHistory) RunView.historyOptions(signature);
    Dom.byId('run-history').value = run?.id || '';
    Dom.byId('run-history').disabled = Studio.isActive(run);
  },

  /** Rebuilds the previous-runs list under its placeholder. */
  historyOptions(signature) {
    Studio.signatures.runHistory = signature;
    const initial = Dom.node('option', 'Previous runs');
    initial.value = '';
    Dom.byId('run-history').replaceChildren(initial, ...(Studio.state.runHistory || []).map(RunView.historyOption));
  },

  /** One previous run in the list. */
  historyOption(item) {
    const option = Dom.node('option', item.name + ' · ' + new Date(item.updatedAt).toLocaleString());
    option.value = item.id;
    return option;
  },

  /** The line under the title: the error, what passed, or a warning. */
  summary(run) {
    if (run?.error) return run.error;
    if (run?.status !== 'succeeded') return 'Validation uses your current login and can change real data.';
    return run.assertions
      ? `${run.assertions} assertions passed.`
      : 'Actions completed. Add assertions to verify the outcome.';
  },

  /** One timeline event; a step's event links to that step. */
  event(event) {
    const item = Dom.node('div', null, 'run-event ' + (event.status || ''));
    const index = Studio.state.draft.steps.findIndex((step) => step.id === event.stepId);
    const number = index < 0 ? '•' : String(index + 1).padStart(RendererConstants.STEP_NUMBER_DIGITS, '0');
    item.append(Dom.node('span', number, 'step-number'), ...RunView.eventText(event));
    if (index >= 0) RunView.linkToStep(item, event.stepId);
    return item;
  },

  /** An event's message and, when timed, its duration. */
  eventText(event) {
    const duration = event.duration != null ? event.duration + ' ms' : '';
    return [Dom.node('span', event.message || event.status || event.kind), Dom.node('small', duration)];
  },

  /** Makes a timeline event open its step (by click or Enter). */
  linkToStep(item, stepId) {
    item.tabIndex = 0;
    const go = () => {
      Studio.selected = stepId;
      Studio.selectTab('steps');
      StudioView.render(Studio.state);
    };
    item.addEventListener('click', go);
    item.addEventListener('keydown', (e) => e.key === 'Enter' && go());
  },

  /** A repair draft to review. */
  repair(repair) {
    const item = Dom.node('div', null, 'repair-review');
    const review = () => Studio.command({ type: 'open', id: repair.draftId }).then(() => Studio.selectTab('steps'));
    item.append(
      Dom.node('strong', 'Repair draft ready'),
      Dom.node('p', `${repair.original.kind} → ${repair.replacement.kind}. The original draft is unchanged.`),
      Studio.button('Review repair', review),
    );
    return item;
  },
};
