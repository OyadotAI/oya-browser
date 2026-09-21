/**
 * The studio's main view: the workflow header, the record and test-run
 * controls, what blocks a run, the save card, and the body. What the studio
 * may do is derived once per snapshot (mode) and every control's disabled
 * state comes from one command map, so no control stays enabled for a
 * command the main process would refuse.
 */
/* global Dom, ShellState, RendererConstants, Studio, StudioSteps, StepEditor, StudioVariables, RunView, StudioActions */
/* exported StudioView */

/** The record button's label in each stage. */
const TOGGLE_LABEL = {
  empty: 'Start recording',
  recording: 'Stop recording',
  captured: 'Resume recording',
  running: 'Resume recording',
};

/** Each control's disabled rule, given what the studio may do now. */
const DISABLED = {
  'record-toggle': (m) => Studio.busy || Studio.saving || m.running,
  'record-validate': (m) => !m.runnable,
  'record-clear': (m) => m.locked || !m.stored,
  'draft-library': (m) => m.locked || !m.stored,
  'record-save': (m) => !m.runnable || !ShellState.connected || Studio.state.saved || !Studio.validName(),
  'record-name': (m) => m.locked,
  'record-desc': (m) => m.locked,
  'step-undo': (m) => m.locked || !Studio.state.canUndo,
  'step-redo': (m) => m.locked || !Studio.state.canRedo,
  'step-add': (m) => m.locked,
  'variable-add': (m) => m.locked,
  'run-history': (m) => m.locked,
  'record-copy': () => !Studio.state.code,
  'record-download': () => !Studio.state.code,
};

/** The parts of the view, drawn in this order. */
const VIEW_PARTS = ['header', 'controls', 'issues', 'finish', 'library', 'body'];

/** The studio view. */
const StudioView = {
  /** Draws a workspace snapshot (ignored when it has no draft). */
  render(next) {
    if (!next?.draft) return;
    Studio.state = next;
    Studio.mode = StudioView.mode(next);
    StudioView.forgetOtherDraft(next.draft.id);
    Dom.byId('pane-record').dataset.stage = Studio.mode.stage;
    for (const part of VIEW_PARTS) StudioView[part](next.draft, Studio.mode);
  },

  /** What the studio may do now. */
  mode(s) {
    const d = s.draft;
    const recording = d.phase === 'recording';
    const running = Studio.isActive(s.run);
    const locked = recording || running || Studio.busy || Studio.saving;
    const runnable = !locked && d.steps.some((step) => step.enabled) && !s.issues.length;
    const stage = recording ? 'recording' : running ? 'running' : d.steps.length ? 'captured' : 'empty';
    return { recording, running, locked, runnable, stage, stored: !s.storageError };
  },

  /** Another draft came on screen: its messages and the "just recorded" note belong to the old one. */
  forgetOtherDraft(id) {
    if (Studio.shownDraft === id) return;
    if (Studio.shownDraft !== undefined) Studio.clearMessages();
    Studio.shownDraft = id;
    Studio.justFinished = false;
  },

  /** Step count, the storage banner and the record button. */
  header(d, m) {
    Dom.byId('record-count').textContent = Studio.plural(d.steps.length, 'step');
    Dom.byId('record-count').title = `Up to ${RendererConstants.MAX_STEPS} steps`;
    Dom.byId('draft-status').hidden = !Studio.state.storageError;
    Dom.byId('draft-status').textContent = Studio.state.storageError || '';
    Dom.byId('record-toggle-label').textContent = TOGGLE_LABEL[m.stage];
    Dom.byId('record-toggle').classList.toggle('recording', m.recording);
    StudioView.toolbarRecord(m.recording);
    StudioView.expandLabel();
  },

  /** The Oya Agent button shows a recording dot while a workflow is recorded. */
  toolbarRecord(recording) {
    ShellState.recording = recording;
    const button = Dom.byId('btn-dev');
    button.classList.toggle('recording', recording);
    button.title = recording ? 'Oya Agent · recording' : 'Oya Agent (⌘/Ctrl Shift D)';
  },

  /** Expand or Compact, from the panel's real width (a drag or keyboard resize changes it too). */
  expandLabel() {
    const expanded = StudioActions.expanded();
    const label = expanded ? 'Compact workspace' : 'Expand workspace';
    Dom.byId('studio-expand').setAttribute('aria-label', label);
    Dom.byId('studio-expand').title = label;
  },

  /** Every control's disabled state, from the command map; variable fields follow the lock. */
  controls(d, m) {
    for (const [id, disabled] of Object.entries(DISABLED)) Dom.byId(id).disabled = disabled(m);
    Dom.byId('workflow-variables')
      .querySelectorAll('input,button')
      .forEach((el) => (el.disabled = m.locked));
    Dom.byId('record-save').title = StudioView.saveTitle();
  },

  /** What blocks a test run and has no step to point at: listed under the button it blocks. */
  issues() {
    const loose = Studio.state.issues.filter((issue) => !issue.stepId);
    Dom.byId('studio-issues').replaceChildren(...loose.map((issue) => Dom.node('li', issue.message)));
  },

  /** The save button's tooltip: publishing needs a connection, local work does not. */
  saveTitle() {
    return ShellState.connected ? 'Save this playbook to Oya' : 'Connect to a server to save to Oya';
  },

  /** The save card: shown once there are steps and no recording, with its copy, button and hint. */
  finish(d, m) {
    const card = Dom.byId('record-finish');
    card.hidden = m.recording || !d.steps.length;
    Dom.byId('record-finish-title').textContent = StudioView.finishTitle();
    Dom.byId('record-finish-copy').textContent = StudioView.finishCopy(d);
    Dom.byId('record-save').textContent = StudioView.saveLabel(d);
    Dom.byId('record-save-hint').textContent = StudioView.saveHint();
    if (!card.hidden && Studio.revealFinish) StudioActions.showFinish();
  },

  /** The card's title: saved, just recorded, or ready to save. */
  finishTitle() {
    if (Studio.state.saved) return 'Saved to Oya';
    return Studio.justFinished ? 'Recording captured' : 'Save to Oya';
  },

  /** What the card says about where the workflow is. */
  finishCopy(d) {
    if (Studio.state.storageError) return 'This workflow is only in memory. Save or export it before closing Oya.';
    if (Studio.state.saved) return 'Your local draft is kept too. Edits need saving again.';
    const steps = Studio.plural(d.steps.length, 'step');
    return Studio.justFinished
      ? `${steps} saved on this device. Name it to reuse it in Oya.`
      : `${steps} on this device.`;
  },

  /** The save button's label. */
  saveLabel(d) {
    if (Studio.saving) return 'Saving…';
    if (Studio.state.saved) return 'Saved';
    return d.publishedAt ? 'Save changes' : 'Save playbook';
  },

  /** What stands between this draft and saving it. */
  saveHint() {
    if (!ShellState.connected) return 'Connect this browser to save to Oya. The draft and the code work offline.';
    if (Studio.state.issues.length) return 'Fix the steps marked ! before saving.';
    const name = Dom.byId('record-name').value.trim();
    return name && !Studio.validName() ? 'Use letters, numbers, hyphens or underscores, up to 64.' : '';
  },

  /** The saved-drafts list; the name and description. */
  library(d) {
    StudioView.drafts(Studio.state.library);
    Dom.byId('draft-library').value = d.id;
    StudioView.fields(d);
  },

  /** The name and description, left alone while the person is typing in them. */
  fields(d) {
    const [name, desc] = [Dom.byId('record-name'), Dom.byId('record-desc')];
    if (document.activeElement !== name) name.value = d.name === 'Untitled workflow' ? '' : d.name;
    if (document.activeElement !== desc) desc.value = d.description;
  },

  /** The saved-drafts list; unreadable drafts are shown but cannot be opened. */
  drafts(library) {
    const libraryKey = JSON.stringify(library.map((item) => [item.id, item.name, item.error]));
    if (libraryKey === Studio.signatures.library) return;
    Studio.signatures.library = libraryKey;
    Dom.byId('draft-library').replaceChildren(...library.map(StudioView.draftOption));
  },

  /** One saved draft in the list. */
  draftOption(item) {
    const option = Dom.node('option', item.name + (item.error ? ' · recovery unavailable' : ''));
    option.value = item.id;
    option.disabled = !!item.error;
    return option;
  },

  /** Steps, the selected step's editor, variables, code and the run. */
  body(d, m) {
    if (!d.steps.some((s) => s.id === Studio.selected)) Studio.selected = d.steps[0]?.id;
    StudioSteps.render(d, m.recording, m.running);
    const selected = d.steps.find((s) => s.id === Studio.selected);
    StepEditor.render(m.recording ? undefined : selected, m.locked);
    StudioVariables.render(d.variables);
    StudioView.code();
    RunView.render();
  },

  /** The generated code, numbered, with the selected step's line highlighted. */
  code() {
    const { state } = Studio;
    const nextCodeSignature = state.code + ':' + Studio.selected;
    if (nextCodeSignature === Studio.signatures.code) return;
    Studio.signatures.code = nextCodeSignature;
    const lines = (state.code || 'Fix the steps marked ! to see the code.').split('\n');
    Dom.byId('record-code').replaceChildren(...lines.map(StudioView.codeLine));
  },

  /** One numbered line of code. */
  codeLine(line, index) {
    const selected = Studio.state.mapping?.[Studio.selected] === index + 1;
    const el = Dom.node('span', null, 'code-line' + (selected ? ' selected' : ''));
    const number = Dom.node('span', String(index + 1), 'code-line-number');
    number.setAttribute('aria-hidden', 'true');
    el.append(number, document.createTextNode(line + '\n'));
    return el;
  },
};
