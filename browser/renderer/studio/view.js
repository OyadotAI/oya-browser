/**
 * The studio's main view: drafts, recording state, save and export controls,
 * the generated code and its issues. Each part redraws only when its input
 * changed.
 */
/* global Dom, ShellState, RendererConstants, Studio, StudioSteps, StepEditor, StudioVariables, RunView */
/* exported StudioView */

/** Text for the three stages a draft can be in: recording, has steps, empty. */
const STAGE_TEXT = {
  /** The panel heading. */
  title: ['Recording your workflow.', 'Shape your workflow.', 'Make it repeatable.'],
  /** The record button in the panel. */
  toggle: ['Finish recording', 'Resume recording', 'Start recording'],
};

/** What the draft status line says when storage is healthy. */
const DRAFT_STATUS = {
  /** While recording. */
  recording: 'Recording · pauses safely on restart · pages and identifiable frames',
  /** Otherwise. */
  saved: 'Saved on this device · encrypted with your OS keychain',
};

/** The stage's text from a STAGE_TEXT row. */
const byStage = (row, recording, hasSteps) => (recording ? row[0] : hasSteps ? row[1] : row[2]);

/** The parts of the view, drawn in this order. */
const VIEW_PARTS = ['header', 'controls', 'finish', 'library', 'body'];

/** The studio view. */
const StudioView = {
  /** Draws a workspace snapshot (ignored when it has no draft). */
  render(next) {
    if (!next?.draft) return;
    Studio.state = next;
    const d = next.draft;
    const flags = { recording: d.phase === 'recording', active: Studio.isActive(next.run) };
    for (const part of VIEW_PARTS) StudioView[part](d, flags);
  },

  /** Title, step count, save state and the record buttons. */
  header(d, { recording, active }) {
    const hasSteps = d.steps.length > 0;
    Dom.byId('record-title').textContent = byStage(STAGE_TEXT.title, recording, hasSteps);
    Dom.byId('record-count').textContent = `${d.steps.length} / ${RendererConstants.MAX_STEPS}`;
    StudioView.draftStatus(recording);
    Dom.byId('record-toggle').textContent = byStage(STAGE_TEXT.toggle, recording, hasSteps);
    Dom.byId('record-toggle').disabled = Studio.busy || active;
    StudioView.toolbarRecord(recording);
  },

  /** Where the draft is kept, or the storage error that stops it being kept. */
  draftStatus(recording) {
    const { storageError } = Studio.state;
    Dom.byId('draft-status').textContent = storageError || (recording ? DRAFT_STATUS.recording : DRAFT_STATUS.saved);
    Dom.byId('draft-status').classList.toggle('error', !!storageError);
  },

  /** The Oya Agent button shows a recording dot while a workflow is recorded. */
  toolbarRecord(recording) {
    ShellState.recording = recording;
    const button = Dom.byId('btn-dev');
    button.classList.toggle('recording', recording);
    button.title = recording ? 'Oya Agent · recording' : 'Oya Agent (⌘/Ctrl Shift D)';
  },

  /** Which controls are available while recording, validating or offline. */
  controls(d, { recording, active }) {
    const { state, busy } = Studio;
    const blocked = recording || active || !d.steps.length || state.issues.length > 0;
    Dom.byId('record-validate').disabled = blocked;
    Dom.byId('record-clear').disabled = busy || recording || active;
    Dom.byId('draft-library').disabled = busy || recording || active;
    Dom.byId('record-save').disabled = busy || blocked || !ShellState.connected;
    Dom.byId('record-save').title = StudioView.saveTitle();
    Dom.byId('record-finish').hidden = recording || !d.steps.length;
  },

  /** The save button's tooltip: publishing needs a connection, local work does not. */
  saveTitle() {
    return ShellState.connected
      ? 'Save this playbook to Oya'
      : 'Connect to a server to publish. Local drafts and export work offline.';
  },

  /** The "recording captured" card: its copy, the save button and its hint. */
  finish(d) {
    const recording = d.phase === 'recording';
    Dom.byId('record-finish-title').textContent = d.publishedAt ? 'Playbook saved to Oya' : 'Recording captured';
    Dom.byId('record-finish-copy').textContent = StudioView.finishCopy(d);
    Dom.byId('record-save').textContent = StudioView.saveLabel(d, recording);
    Dom.byId('record-save-hint').textContent = StudioView.saveHint();
  },

  /** What the card says about where the recording is. */
  finishCopy(d) {
    if (d.publishedAt) return 'Your local draft is kept too. Save again after making changes.';
    if (Studio.state.storageError) return 'Your recording is in memory. Save or export it before closing Oya.';
    return `${d.steps.length} steps saved on this device. Name your workflow and save a playbook to reuse it in Oya.`;
  },

  /** The save button's label. */
  saveLabel(d, recording) {
    if (Studio.busy && !recording) return 'Saving…';
    return d.publishedAt ? 'Save changes' : 'Save playbook';
  },

  /** What stands between this draft and saving it. */
  saveHint() {
    if (!ShellState.connected) {
      return 'Connect this browser to save to Oya. Your local draft and Playwright export are available now.';
    }
    return Studio.state.issues.length
      ? 'Resolve the highlighted steps before saving.'
      : 'Saves to your connected Oya workspace.';
  },

  /** Undo, redo and add; the saved-drafts list; the name and description. */
  library(d, { recording, active }) {
    const { state } = Studio;
    Dom.byId('step-undo').disabled = recording || active || !state.canUndo;
    Dom.byId('step-redo').disabled = recording || active || !state.canRedo;
    Dom.byId('step-add').disabled = recording || active;
    StudioView.drafts(state.library);
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

  /** Steps, the selected step's editor, variables, code, issues and the run. */
  body(d, { recording, active }) {
    if (!d.steps.some((s) => s.id === Studio.selected)) Studio.selected = d.steps[0]?.id;
    StudioSteps.render(d, recording, active);
    const selected = d.steps.find((s) => s.id === Studio.selected);
    StepEditor.render(selected, Studio.busy || recording || active);
    StudioVariables.render(d.variables);
    StudioView.code();
    StudioView.issues();
    RunView.render();
  },

  /** The capture issues that block validating and saving. */
  issues() {
    const nodes = Studio.state.issues.map((issue) => Dom.node('p', issue.message, 'studio-issue'));
    Dom.byId('inspect-issues').replaceChildren(...nodes);
  },

  /** The generated code, numbered, with the selected step's line highlighted. */
  code() {
    const { state } = Studio;
    const nextCodeSignature = state.code + ':' + Studio.selected;
    if (nextCodeSignature === Studio.signatures.code) return;
    Studio.signatures.code = nextCodeSignature;
    const lines = (state.code || 'Resolve the capture issues before exporting this workflow.').split('\n');
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
