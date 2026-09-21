/**
 * What the studio's buttons do: record, test run, save to Oya, export, and
 * the edits sent to the workspace. Every action clears and reports in the
 * message slot under its own control. Also loads the workspace at start.
 */
/* global oyaBrowser, Dom, ShellState, RendererConstants, Studio, StudioView, DevPanel, CommandPalette */
/* exported StudioActions */

/** Studio actions. */
const StudioActions = {
  /** Starts, resumes or stops recording, then refreshes the workspace. */
  async toggleRecording() {
    if (Studio.busy) return;
    await StudioActions.whileBusy(async () => {
      await StudioActions.switchRecording();
      await Studio.command({ type: 'get' });
    });
  },

  /** Runs `work` with the studio busy, reporting a failure in `slot` and redrawing when done. */
  async whileBusy(work, slot = 'record-result', flag = 'busy') {
    Studio[flag] = true;
    Studio.say('', false, slot);
    if (Studio.state) StudioView.render(Studio.state);
    await Promise.resolve()
      .then(work)
      .catch((error) => Studio.say(Studio.cleanError(error), true, slot));
    Studio[flag] = false;
    if (Studio.state) StudioView.render(Studio.state);
  },

  /** Stops a recording, resumes a draft that has steps, or starts a new one. */
  async switchRecording() {
    const draft = Studio.state?.draft;
    if (draft?.phase === 'recording') return StudioActions.stopRecording();
    if (draft?.steps.length) {
      const resumed = await Studio.command({ type: 'resume-recording' });
      if (!resumed) throw new Error(Dom.byId('record-result').textContent || 'Could not resume recording');
    } else await oyaBrowser.startRecording();
  },

  /** Stops recording; the save card comes into view once it is drawn. */
  async stopRecording() {
    await oyaBrowser.stopRecording();
    Object.assign(Studio, { justFinished: true, revealFinish: true });
    Studio.selectTab('steps');
  },

  /** Brings the save card into view and puts focus on the name. */
  showFinish() {
    Studio.revealFinish = false;
    Dom.byId('record-finish').scrollIntoView?.({ block: 'nearest' });
    if (!Dom.byId('record-name').value) Dom.byId('record-name').focus({ preventScroll: true });
  },

  /** Starts a test run with the run inputs' values; shows the Run tab only if a run started. */
  async validate(extra = {}) {
    const before = Studio.state?.run?.id;
    const next = await Studio.command({ type: 'validate', vars: StudioActions.runInputs(), ...extra });
    Dom.byId('run-inputs')
      .querySelectorAll('input[type=password]')
      .forEach((input) => (input.value = ''));
    if (next?.run && next.run.id !== before) Studio.selectTab('run');
  },

  /** The run inputs' values, by variable. */
  runInputs() {
    const inputs = [...Dom.byId('run-inputs').querySelectorAll('input')];
    return Object.fromEntries(inputs.map((input) => [input.dataset.variable, input.value]));
  },

  /** Adds a step after the selected one, then selects it; a refused add keeps the selection. */
  async addStep(e) {
    if (!e.target.value) return;
    const step = { action: e.target.value, candidates: [], expected: '' };
    const next = await Studio.command({ type: 'add', id: Studio.selected, step });
    e.target.value = '';
    if (!next) return;
    StudioActions.selectAdded();
    StudioView.render(Studio.state);
  },

  /** Selects the step just added: the one after the selection, or the last. */
  selectAdded() {
    const steps = Studio.state?.draft.steps;
    Studio.selected =
      steps?.find((step, i, list) => i > 0 && list[i - 1].id === Studio.selected)?.id || steps?.at(-1)?.id;
  },

  /** Adds the next free input_n variable and says how to use it. */
  async addVariable() {
    if (!Studio.state || Studio.mode.locked) return;
    let name;
    const add = (variables) => ({ ...variables, [(name = StudioActions.freeInput(variables))]: { default: '' } });
    const next = await Studio.command(() => ({ type: 'variables', variables: add(Studio.state.draft.variables) }));
    if (next) Studio.say(`Use {{${name}}} in a step.`);
  },

  /** The first input_n name the variables do not use. */
  freeInput(variables) {
    let n = 1;
    while (variables['input_' + n]) n++;
    return 'input_' + n;
  },

  /** Saves the playbook to the connected Oya workspace. */
  async save() {
    if (Studio.saving || Dom.byId('record-save').disabled) return;
    const name = Dom.byId('record-name').value.trim();
    const description = Dom.byId('record-desc').value || name;
    await StudioActions.whileBusy(() => StudioActions.publish(name, description), 'save-result', 'saving');
  },

  /** The save itself, its confirmation, and the refreshed state. */
  async publish(name, description) {
    const result = await oyaBrowser.saveRecording(name, description);
    if (!result || result.error) throw new Error(result?.error || 'The server did not confirm the save');
    Studio.justFinished = false;
    await Studio.command({ type: 'get' }, 'save-result');
    Studio.say(`Saved “${name}” to Oya.`, false, 'save-result');
  },

  /** Copies the generated module. */
  async copy() {
    try {
      await navigator.clipboard.writeText(Studio.state.code);
      Studio.say('Code copied.', false, 'code-result');
    } catch (error) {
      Studio.say(Studio.cleanError(error), true, 'code-result');
    }
  },

  /** Saves the generated module to a file. */
  async download() {
    try {
      const result = await oyaBrowser.exportPlaywright({ name: Studio.state.draft.name, code: Studio.state.code });
      Studio.say(result.saved ? 'Playwright module exported.' : '', false, 'code-result');
    } catch (error) {
      Studio.say(Studio.cleanError(error), true, 'code-result');
    }
  },

  /** Saves the diagnostics report and says so. */
  async support() {
    const next = await Studio.command({ type: 'support' }, 'code-result');
    if (next?.supportSaved) Studio.say('Diagnostics saved.', false, 'code-result');
  },

  /** ⌘/Ctrl Alt R: open the studio, then start or stop recording, when the record button could. */
  async recordButton() {
    const toggle = Dom.byId('record-toggle');
    if (toggle.disabled || toggle.hasAttribute('data-control-blocked')) return;
    if (!ShellState.devOpen) await oyaBrowser.toggleDevPanel();
    DevPanel.show('record');
    Studio.selectTab('steps');
    // Pressed, not called: the press goes through the control guard, which takes control
    // first while an agent drives. Calling the action skipped it and the shortcut only
    // answered "Take control before interacting with this page".
    toggle.click();
  },

  /** Whether the panel is at (or past) its expanded width. */
  expanded() {
    const width = parseFloat(document.documentElement.style.getPropertyValue('--panel-width'));
    return width >= RendererConstants.PANEL_MAX_WIDTH;
  },

  /** Expands or compacts the panel, from its real width. */
  async expand() {
    const next = StudioActions.expanded() ? RendererConstants.PANEL_COMPACT_WIDTH : RendererConstants.PANEL_MAX_WIDTH;
    await oyaBrowser.resizeDevPanel(next);
  },

  /** Sends the name and description. */
  metadata() {
    Studio.command({
      type: 'metadata',
      name: Dom.byId('record-name').value || 'Untitled workflow',
      description: Dom.byId('record-desc').value,
    });
  },

  /** Enter in the name field saves. */
  nameKey(event) {
    if (event.key !== 'Enter' || Dom.byId('record-save').disabled) return;
    event.preventDefault();
    Dom.byId('record-name').blur();
    StudioActions.save();
  },

  /** As the name is typed: the Save button and hint follow the naming rule. */
  nameTyped() {
    if (Studio.state) StudioView.render(Studio.state);
  },
};

Dom.byId('record-toggle').addEventListener('click', StudioActions.toggleRecording);
// It moved off ⌘⇧R (Chrome's hard reload), so it is no longer the one people guess: say it on the button.
Dom.byId('record-toggle').title = `Start or stop recording (${CommandPalette.recordShortcut})`;
Dom.byId('run-history').addEventListener(
  'change',
  (e) => e.target.value && Studio.command({ type: 'open-run', id: e.target.value }),
);
Dom.byId('record-validate').addEventListener('click', () => StudioActions.validate());
Dom.byId('record-clear').addEventListener('click', () => Studio.command({ type: 'new' }));
Dom.byId('draft-library').addEventListener('change', (e) => Studio.command({ type: 'open', id: e.target.value }));
Dom.byId('studio-expand').addEventListener('click', StudioActions.expand);
Dom.byId('step-undo').addEventListener('click', () => Studio.command({ type: 'undo' }));
Dom.byId('step-redo').addEventListener('click', () => Studio.command({ type: 'redo' }));
Dom.byId('step-add').addEventListener('change', StudioActions.addStep);
Dom.byId('variable-add').addEventListener('click', StudioActions.addVariable);
for (const id of ['record-name', 'record-desc']) Dom.byId(id).addEventListener('change', StudioActions.metadata);
document
  .querySelectorAll('[data-run]')
  .forEach((el) => el.addEventListener('click', () => Studio.command({ type: 'control', command: el.dataset.run })));
Dom.byId('record-name').addEventListener('keydown', StudioActions.nameKey);
Dom.byId('record-name').addEventListener('input', StudioActions.nameTyped);
Dom.byId('record-save').addEventListener('click', StudioActions.save);
Dom.byId('record-copy').addEventListener('click', StudioActions.copy);
Dom.byId('record-download').addEventListener('click', StudioActions.download);
Dom.byId('support-export').addEventListener('click', StudioActions.support);
oyaBrowser.onWorkspace(StudioView.render);
oyaBrowser.onWsStatus(() => Studio.state && StudioView.render(Studio.state));
oyaBrowser.onShellLayout(() => Studio.state && StudioView.expandLabel());
Studio.command({ type: 'get' });
Studio.selectTab('steps');
