/**
 * The studio's Run tab: the run's status and summary, the controls that make
 * sense for its status, the timeline (each step's latest event), repair
 * drafts to review, previous runs, and the diagnostics preview.
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
    running: 'Running…',
    starting: 'Preparing a fresh tab…',
    interrupted: 'Run interrupted.',
    stopping: 'Stopping…',
    stopped: 'Run stopped.',
  },

  /** Event kinds the timeline shows. */
  SHOWN: ['step', 'attention', 'target'],

  /** The run statuses each control works in. */
  CONTROLS: {
    pause: ['starting', 'running'],
    resume: ['paused'],
    step: ['paused'],
    stop: ['starting', 'running', 'paused'],
  },

  /** Draws the run. */
  render() {
    const run = Studio.state.run;
    RunView.history(run);
    Dom.byId('run-title').textContent = RunView.title(run);
    Dom.byId('run-summary').textContent = RunView.summary(run);
    RunView.controls(run);
    RunView.events(run);
    Dom.byId('run-repairs').replaceChildren(...(run?.repairs || []).map(RunView.repair));
    RunView.support();
  },

  /** The timeline, rebuilt only when what it shows changed, so focus in it survives a push. */
  events(run) {
    const shown = RunView.shownEvents(run).map((event) => RunView.settled(event, run));
    const signature = JSON.stringify([shown, Studio.state.draft.steps.map((s) => s.id)]);
    if (signature === Studio.signatures.events) return;
    Studio.signatures.events = signature;
    Dom.byId('run-events').replaceChildren(...shown.map(RunView.event));
  },

  /** A step still "running" when the run is over ended with the run. */
  settled(event, run) {
    if (Studio.isActive(run) || event.status !== 'running') return event;
    return { ...event, status: run.status, message: RunView.title(run) };
  },

  /** The diagnostics preview: the same centrally redacted object the main process writes. */
  support() {
    const json = JSON.stringify(Studio.state.support || {}, null, RendererConstants.JSON_INDENT);
    Dom.byId('support-preview').textContent = json;
  },

  /** The heading: the status's title, the raw status if it has none, or the idle prompt. */
  title(run) {
    if (!run) return 'Test this workflow';
    return (Object.hasOwn(RunView.TITLES, run.status) ? RunView.TITLES[run.status] : undefined) || run.status;
  },

  /** The controls show only while a run is live, each enabled only in the statuses it works in. */
  controls(run) {
    Dom.byId('run-controls').hidden = !Studio.isActive(run);
    document.querySelectorAll('[data-run]').forEach((el) => {
      el.disabled = !RunView.CONTROLS[el.dataset.run]?.includes(run?.status);
    });
  },

  /** Each step's latest event (and every event without a step), most recent last. */
  shownEvents(run) {
    const events = (run?.events || []).filter((e) => RunView.SHOWN.includes(e.kind));
    const seen = new Set();
    const latest = events.reverse().filter((e) => !e.stepId || (!seen.has(e.stepId) && seen.add(e.stepId)));
    return latest.reverse().slice(-RendererConstants.RUN_EVENTS_SHOWN);
  },

  /** The previous-runs list, redrawn only when it changed, with this run selected. */
  history(run) {
    const signature = JSON.stringify(Studio.state.runHistory);
    if (signature !== Studio.signatures.runHistory) RunView.historyOptions(signature);
    Dom.byId('run-history').value = run?.id || '';
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
    if (run?.status !== 'succeeded') return 'A test run uses your current login and can change real data.';
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

  /** An event's message (or its step's action and status) and, when timed, its duration. */
  eventText(event) {
    const duration = event.duration != null ? event.duration + ' ms' : '';
    const step = Studio.state.draft.steps.find((s) => s.id === event.stepId);
    const said = step ? `${Studio.name(step.action)} · ${event.status || event.kind}` : event.status || event.kind;
    return [Dom.node('span', event.message || said), Dom.node('small', duration)];
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
    const note = `${repair.original.kind} → ${repair.replacement.kind}. The target that worked in this run.`;
    item.append(Dom.node('strong', 'Repair found'), Dom.node('p', note));
    item.append(RunView.applyButton(repair), Studio.button('Review as a copy', review));
    return item;
  },

  /** "Apply to this workflow", enabled only when it can be. */
  applyButton(repair) {
    const apply = Studio.button('Apply to this workflow', () => RunView.applyRepair(repair));
    apply.disabled = !RunView.canApply(repair);
    return apply;
  },

  /** Whether the repair's step is in the workflow shown, with the run over and the target not yet first. */
  canApply(repair) {
    const run = Studio.state.run;
    const step = Studio.state.draft.steps.find((s) => s.id === repair.stepId);
    if (!step || Studio.isActive(run) || run?.draftId !== Studio.state.draft.id) return false;
    return !RunView.sameTarget(step.candidates?.[0], repair.replacement);
  },

  /** Whether two targets are the same locator. */
  sameTarget(a, b) {
    return !!a && !!b && a.kind === b.kind && a.value === b.value && a.role === b.role;
  },

  /** Makes the target that worked the step's first, keeping the others behind it; Undo takes it back. */
  applyRepair(repair) {
    const step = Studio.state.draft.steps.find((s) => s.id === repair.stepId);
    if (!step) return;
    const rest = (step.candidates || []).filter((c) => !RunView.sameTarget(c, repair.replacement));
    Studio.command({ type: 'update', id: step.id, patch: { candidates: [repair.replacement, ...rest] } });
  },
};
