/**
 * What the studio's buttons do: record, validate, save to Oya, export, and
 * the edits sent to the workspace. Also loads the workspace at start.
 */
/* global oyaBrowser, Dom, ShellState, RendererConstants, Studio, StudioView, DevPanel */
/* exported StudioActions */

/** Studio actions. */
const StudioActions = {
  /** Starts, resumes or finishes recording, then refreshes the workspace. */
  async toggleRecording() {
    if (Studio.busy) return;
    await StudioActions.whileBusy(async () => {
      await StudioActions.switchRecording();
      Studio.say();
      await Studio.command({ type: 'get' });
    });
  },

  /** Runs `work` with the studio marked busy, reporting a failure and redrawing when done. */
  async whileBusy(work) {
    Studio.busy = true;
    await Promise.resolve()
      .then(work)
      .catch((error) => Studio.say(error.message, true));
    Studio.busy = false;
    if (Studio.state) StudioView.render(Studio.state);
  },

  /** Finishes a recording, resumes a draft that has steps, or starts a new one. */
  async switchRecording() {
    const draft = Studio.state?.draft;
    if (draft?.phase === 'recording') {
      await oyaBrowser.stopRecording();
      Studio.selectTab('steps');
      requestAnimationFrame(StudioActions.showFinish);
    } else if (draft?.steps.length) await Studio.command({ type: 'resume-recording' });
    else await oyaBrowser.startRecording();
  },

  /** Brings the save card into view and puts focus on the name. */
  showFinish() {
    Dom.byId('record-finish').scrollIntoView({ block: 'nearest' });
    if (!Dom.byId('record-name').value) Dom.byId('record-name').focus({ preventScroll: true });
  },

  /** Validates with the run inputs' values, then forgets any secrets typed. */
  async validate(extra = {}) {
    Studio.selectTab('run');
    const inputs = [...Dom.byId('run-inputs').querySelectorAll('input')];
    const vars = Object.fromEntries(inputs.map((input) => [input.dataset.variable, input.value]));
    await Studio.command({ type: 'validate', vars, ...extra });
    Dom.byId('run-inputs')
      .querySelectorAll('input[type=password]')
      .forEach((input) => (input.value = ''));
  },

  /** Adds a step after the selected one, then selects it. */
  async addStep(e) {
    if (!e.target.value) return;
    const step = { action: e.target.value, candidates: [], expected: '' };
    await Studio.command({ type: 'add', id: Studio.selected, step });
    e.target.value = '';
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
    const variables = { ...Studio.state.draft.variables };
    let n = 1;
    while (variables['input_' + n]) n++;
    variables['input_' + n] = { default: '' };
    await Studio.command({ type: 'variables', variables });
    Studio.say(
      `Use {{input_${n}}} in a step. Rename inputs by editing the placeholder and adding its matching variable.`,
    );
  },

  /** Saves the playbook to the connected Oya workspace under a checked name. */
  async save() {
    if (Studio.busy) return;
    const name = Dom.byId('record-name').value.trim();
    const description = Dom.byId('record-desc').value || name;
    await StudioActions.whileBusy(() => {
      StudioView.render(Studio.state);
      return StudioActions.publish(name, description);
    });
  },

  /** The save itself: name check, save, confirmation, refresh. */
  async publish(name, description) {
    if (!/^[\w-]{1,64}$/.test(name))
      throw new Error('Use 1–64 letters, numbers, hyphens or underscores for the published name.');
    const result = await oyaBrowser.saveRecording(name, description);
    if (result.error) throw new Error(result.error);
    Studio.say(`Playbook “${name}” saved to Oya. Your local draft is retained.`);
    await Studio.command({ type: 'get' });
  },

  /** Copies the generated module. */
  async copy() {
    try {
      await navigator.clipboard.writeText(Studio.state.code);
      Studio.say('Playwright module copied.');
    } catch (error) {
      Studio.say(error.message, true);
    }
  },

  /** Saves the generated module to a file. */
  async download() {
    try {
      if (!Studio.state.code) throw new Error('Resolve the capture issues first.');
      const result = await oyaBrowser.exportPlaywright({ name: Studio.state.draft.name, code: Studio.state.code });
      if (result.saved) Studio.say('Playwright module exported.');
    } catch (error) {
      Studio.say(error.message, true);
    }
  },

  /** ⌘/Ctrl Shift R: open the studio, then start or pause recording. */
  async recordButton() {
    if (!ShellState.devOpen) await oyaBrowser.toggleDevPanel();
    DevPanel.show('record');
    Studio.selectTab('steps');
    await StudioActions.toggleRecording();
  },

  /** Expands or compacts the panel. */
  async expand() {
    Studio.expanded = !Studio.expanded;
    await oyaBrowser.resizeDevPanel(
      Studio.expanded ? RendererConstants.PANEL_MAX_WIDTH : RendererConstants.PANEL_COMPACT_WIDTH,
    );
    Dom.byId('studio-expand').textContent = Studio.expanded ? 'Compact' : 'Expand';
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
    Dom.byId('record-save').click();
  },
};

Dom.byId('record-toggle').addEventListener('click', StudioActions.toggleRecording);
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
Dom.byId('record-save').addEventListener('click', StudioActions.save);
Dom.byId('record-copy').addEventListener('click', StudioActions.copy);
Dom.byId('record-download').addEventListener('click', StudioActions.download);
Dom.byId('support-export').addEventListener('click', () => Studio.command({ type: 'support' }));
oyaBrowser.onWorkspace(StudioView.render);
oyaBrowser.onWsStatus(() => Studio.state && StudioView.render(Studio.state));
Studio.command({ type: 'get' });
Studio.selectTab('steps');
