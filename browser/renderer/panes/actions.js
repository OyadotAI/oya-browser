/**
 * The Actions pane: run a single browser command by hand (analyze, click,
 * type, …) and see its labelled result. Inputs are checked before anything
 * is sent, one action runs at a time, actions that touch the page wait for a
 * person to hold control, Enter runs a row's action, and an analysis lists
 * the page's elements so their numbers can be picked instead of guessed.
 */
/* global oyaBrowser, Dom, RendererConstants */
/* exported DevActions */

/** Whether `value` is a whole element number. */
const isElementNumber = (value) => /^\d+$/.test(String(value));

/** Whether `value` is a real coordinate: an empty field is not 0. */
const isCoordinate = (value) => value !== '' && Number.isFinite(Number(value));

/** Manual commands. */
const DevActions = {
  /** The action in flight, so only one runs at a time. */
  running: null,
  /** Whether a person holds control; page actions wait for it. */
  interactive: true,
  /** The last result's text, for Copy. */
  resultText: '',

  /** Actions that only look, so they run while the agent holds the page. */
  UNGUARDED: ['screenshot', 'list-tabs'],

  /** What each action's name reads as in a result's heading. */
  TITLES: { 'click-coords': 'Click at', 'press-key': 'Press key', 'list-tabs': 'Tabs', 'new-tab': 'New tab' },

  /** How each action reads its parameters from the inputs named in data-from. */
  PARAMS: {
    navigate: ([a]) => ({ url: a.value.trim() }),
    click: ([a]) => ({ element_id: a.value.trim() }),
    type: ([a, b]) => ({ element_id: a.value.trim(), text: b.value }),
    'press-key': ([a]) => ({ key: a.value.trim() }),
    hover: ([a]) => ({ element_id: a.value.trim() }),
    'click-coords': ([a, b]) => ({ x: a.value.trim(), y: b.value.trim() }),
    wait: ([a]) => ({ selector: a.value.trim() }),
  },

  /** Why an action's parameters cannot be sent, by the parameter that is wrong. */
  CHECKS: {
    element_id: (v) => (isElementNumber(v) ? '' : 'Enter an element number, such as 12'),
    x: (v) => (isCoordinate(v) ? '' : 'Enter X and Y as numbers'),
    y: (v) => (isCoordinate(v) ? '' : 'Enter X and Y as numbers'),
  },

  /** The action's parameters, from the inputs its button names. */
  getActionParams(btn) {
    const { from, action } = btn.dataset;
    if (!from || !Object.hasOwn(DevActions.PARAMS, action)) return {};
    return DevActions.PARAMS[action](from.split(',').map((id) => document.getElementById(id)));
  },

  /** The first problem with `params`, or ''. */
  problem(params) {
    for (const [key, value] of Object.entries(params)) {
      const message = Object.hasOwn(DevActions.CHECKS, key) ? DevActions.CHECKS[key](value) : '';
      if (message) return message;
    }
    return '';
  },

  /** Runs a button's action unless another is running or its input is wrong, then shows the result. */
  async run(btn) {
    const action = btn.dataset.action;
    if (!action || DevActions.running || btn.disabled) return;
    const params = DevActions.getActionParams(btn);
    const problem = DevActions.problem(params);
    if (problem) return DevActions.show(action, { ok: false, error: problem });
    DevActions.setRunning(action);
    await DevActions.perform(action, params);
    DevActions.setRunning(null);
  },

  /** Marks one action as running, with every other button waiting. */
  setRunning(action) {
    DevActions.running = action;
    Dom.byId('pane-actions').classList.toggle('busy', !!action);
    DevActions.syncButtons();
  },

  /** Each button is off while an action runs, or while the agent holds the page and it touches the page. */
  syncButtons() {
    document.querySelectorAll('#pane-actions .action-btn').forEach((btn) => {
      const guarded = !DevActions.UNGUARDED.includes(btn.dataset.action);
      btn.disabled = !!DevActions.running || (guarded && !DevActions.interactive);
      btn.title = guarded && !DevActions.interactive ? 'Take control to use this' : '';
    });
  },

  /** Control changed hands. */
  controlChanged(state) {
    DevActions.interactive = !!state?.interactive;
    DevActions.syncButtons();
  },

  /** Asks the main process to run the action and shows what came back. */
  perform(action, params) {
    return oyaBrowser.devAction(action, params).then(
      (result) => DevActions.show(action, result),
      (error) => DevActions.show(action, { ok: false, error: error.message }),
    );
  },

  /** A result's heading: the action, whether it worked, and when. */
  heading(action, result) {
    const title = DevActions.TITLES[action] || action[0].toUpperCase() + action.slice(1);
    return `${title} · ${result?.ok ? 'done' : 'failed'} · ${new Date().toLocaleTimeString()}`;
  },

  /** Shows a result under its heading: a screenshot, an analysis, data, or the error. */
  show(action, result) {
    Dom.byId('action-result').classList.add('visible');
    Dom.byId('action-result').classList.toggle('error', !result?.ok);
    Dom.byId('action-result-title').textContent = DevActions.heading(action, result);
    if (!result?.ok) return DevActions.text(result?.error || 'Unknown error');
    if (action === 'screenshot' && result.data?.screenshot) return DevActions.image(result.data.screenshot);
    if (action === 'analyze') DevActions.elements(result.data?.elements || []);
    const text = action === 'analyze' ? result.data?.page || '' : DevActions.json(result);
    DevActions.text(text, action === 'analyze' ? RendererConstants.ANALYZE_PREVIEW : RendererConstants.RESULT_PREVIEW);
  },

  /** A successful result's data, as JSON. */
  json(result) {
    return JSON.stringify(result.data || result, null, RendererConstants.JSON_INDENT);
  },

  /** Shows text, cut to `limit` characters with a note saying so. */
  text(value, limit = Infinity) {
    DevActions.resultText = value;
    const cut = value.length > limit;
    Dom.byId('action-result-body').textContent = cut ? value.slice(0, limit) : value;
    if (cut)
      Dom.byId('action-result-body').append(
        Dom.node('p', 'Showing the first part. Copy takes all of it.', 'action-cut'),
      );
  },

  /** Shows a screenshot. */
  image(src) {
    DevActions.resultText = src;
    const img = Dom.node('img', null, 'action-shot');
    Object.assign(img, { src, alt: 'Screenshot of the page' });
    Dom.byId('action-result-body').replaceChildren(img);
  },

  /** The analyzed page's visible elements, as buttons that fill in their number. */
  elements(elements) {
    const shown = elements.filter((e) => e.visible !== false).slice(0, RendererConstants.ACTION_ELEMENTS_SHOWN);
    Dom.byId('action-elements').replaceChildren(...shown.map(DevActions.elementButton));
    Dom.byId('action-elements-hint').textContent = shown.length
      ? 'Pick an element, then Click, Hover or Type.'
      : 'Nothing to act on in view.';
  },

  /** One element: "#12 button Sign in", filling the element number when chosen. */
  elementButton(element) {
    const label = `#${element.id} ${element.type} ${element.text || ''}`.trim();
    const button = Dom.node('button', label, 'action-element');
    button.type = 'button';
    button.addEventListener('click', () => DevActions.pickElement(element.id));
    return button;
  },

  /** Puts an element's number in the element field. */
  pickElement(id) {
    Dom.byId('action-click-id').value = String(id);
    Dom.byId('action-click-id').focus();
  },

  /** Copies the whole last result. */
  async copy() {
    try {
      await navigator.clipboard.writeText(DevActions.resultText);
      Dom.byId('action-result-copy').textContent = 'Copied';
    } catch {
      Dom.byId('action-result-copy').textContent = 'Copy failed';
    }
    setTimeout(() => (Dom.byId('action-result-copy').textContent = 'Copy'), RendererConstants.COPIED_MS);
  },

  /** Enter in a field runs the action of its row. */
  fieldKey(event) {
    if (event.key !== 'Enter') return;
    const own = event.target.closest('.action-input-row')?.querySelector('.action-btn');
    const element = event.target.id === 'action-type-text' ? '[data-action="type"]' : '[data-action="click"]';
    const button = own || document.querySelector(element);
    event.preventDefault();
    DevActions.run(button);
  },

  /** Empties the result and the element list. */
  clear() {
    DevActions.resultText = '';
    Dom.byId('action-result-body').replaceChildren();
    Dom.byId('action-result').classList.remove('visible', 'error');
    Dom.byId('action-elements').replaceChildren();
  },
};

document.querySelectorAll('#pane-actions .action-btn').forEach((btn) => {
  btn.addEventListener('click', () => DevActions.run(btn));
});
document.querySelectorAll('#pane-actions .action-field').forEach((field) => {
  field.addEventListener('keydown', DevActions.fieldKey);
});
Dom.byId('action-result-copy').addEventListener('click', DevActions.copy);
oyaBrowser.onControlState(DevActions.controlChanged);
oyaBrowser.getControlState().then(DevActions.controlChanged);
