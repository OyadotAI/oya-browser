/**
 * The Actions pane: run a single browser command by hand (analyze, click,
 * type, …) and see its result.
 */
/* global oyaBrowser, Dom, RendererConstants */
/* exported DevActions */

/** Manual commands. */
const DevActions = {
  /** How each action reads its parameters from the inputs named in data-from. */
  PARAMS: {
    navigate: ([a]) => ({ url: a.value.trim() }),
    click: ([a]) => ({ element_id: a.value.trim() }),
    type: ([a, b]) => ({ element_id: a.value.trim(), text: b.value }),
    'press-key': ([a]) => ({ key: a.value.trim() }),
    hover: ([a]) => ({ element_id: a.value.trim() }),
    'click-coords': ([a, b]) => ({ x: Number(a.value), y: Number(b.value) }),
    wait: ([a]) => ({ selector: a.value.trim() }),
    select: ([a, b]) => ({ element_id: a.value.trim(), value: b.value }),
  },

  /** The action's parameters, from the inputs its button names. */
  getActionParams(btn) {
    const from = btn.dataset.from;
    if (!from) return {};
    const action = btn.dataset.action;
    if (!Object.hasOwn(DevActions.PARAMS, action)) return {};
    // Map param inputs to action params
    return DevActions.PARAMS[action](from.split(',').map((id) => document.getElementById(id)));
  },

  /** Runs a button's action once at a time and shows the result. */
  async run(btn) {
    const action = btn.dataset.action;
    if (!action || btn.classList.contains('running')) return;
    btn.classList.add('running');
    await DevActions.perform(action, btn);
    btn.classList.remove('running');
  },

  /** Asks the main process to run the action and shows what came back. */
  perform(action, btn) {
    const resultEl = Dom.byId('action-result');
    const show = (result) => DevActions.show(resultEl, action, result);
    return oyaBrowser
      .devAction(action, DevActions.getActionParams(btn))
      .then(show, (e) => DevActions.threw(resultEl, e));
  },

  /** The action itself failed to run. */
  threw(resultEl, e) {
    resultEl.textContent = e.message;
    resultEl.classList.add('error', 'visible');
  },

  /** Shows a result: a screenshot, an analysis, data, or the error. */
  show(resultEl, action, result) {
    resultEl.classList.remove('error');
    resultEl.classList.add('visible');
    if (!result?.ok) return DevActions.failed(resultEl, result);
    if (action === 'screenshot' && result.data?.screenshot) {
      resultEl.innerHTML = '<img style="width:100%;border-radius:4px;" src="' + Dom.esc(result.data.screenshot) + '">';
    } else if (action === 'analyze' && result.data?.markdown) {
      resultEl.textContent = result.data.markdown.slice(0, RendererConstants.ANALYZE_PREVIEW);
    } else DevActions.data(resultEl, result);
  },

  /** Any other successful result, as trimmed JSON. */
  data(resultEl, result) {
    const json = JSON.stringify(result.data || result, null, RendererConstants.JSON_INDENT);
    resultEl.textContent = json.slice(0, RendererConstants.RESULT_PREVIEW);
  },

  /** Shows an action's error. */
  failed(resultEl, result) {
    resultEl.textContent = result?.error || 'Unknown error';
    resultEl.classList.add('error');
  },

  /** Empties the result. */
  clear() {
    const r = Dom.byId('action-result');
    r.textContent = '';
    r.classList.remove('visible', 'error');
  },
};

document.querySelectorAll('.action-btn').forEach((btn) => {
  btn.addEventListener('click', () => DevActions.run(btn));
});
