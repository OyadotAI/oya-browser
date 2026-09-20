/**
 * The toolbar's control status: who is driving this browser (the agent, you,
 * another operator), the Take control / Release button, and the watch-only
 * guard that blocks local actions until you hold control.
 */
/* global oyaBrowser */

/** What the status says in each control mode. `human` depends on whether it is you. */
const CONTROL_LABELS = {
  agent: 'Agent control',
  paused: 'Automation paused',
  offline: 'Offline · your control',
  disconnected: 'Disconnected',
  unavailable: 'Control unavailable',
  taking: 'Taking control…',
};

/** Toolbar controls that act on the page, blocked while it is watch-only. */
const PAGE_ACTIONS = ['btn-reload', 'btn-new-tab', 'record-toggle'];

/** Clicks refused while watch-only (matched before the controls' own listeners run). */
const GUARDED = '#btn-back, #btn-forward, #btn-reload, #btn-new-tab, .tab-close, #record-toggle';

/** The control status and its buttons. */
const ControlBar = {
  /** The last control state drawn. */
  current: undefined,
  /** The last control change's error, shown until the state moves on. */
  error: '',

  /** An element by id. */
  el: (id) => document.getElementById(id),

  /** Draws a control state. */
  render(state) {
    ControlBar.current = state;
    const mine = state.mode === 'human' && state.mine;
    const mode = state.busy || state.taking ? 'taking' : state.mode;
    ControlBar.status(state, mode, mine);
    ControlBar.buttons(state, mine);
    ControlBar.gate(state.interactive);
  },

  /** The status chip: its mode, label and tooltip. */
  status(state, mode, mine) {
    const status = ControlBar.el('control-status');
    status.dataset.mode = mode;
    ControlBar.el('control-label').textContent = ControlBar.error || ControlBar.label(state, mode, mine);
    const hint = state.interactive
      ? 'You can interact with this page.'
      : 'Page is watch-only. Take control to interact.';
    status.title = ControlBar.error || hint;
  },

  /** The label for a mode; "you" versus "another operator" when a person holds control. */
  label(state, mode, mine) {
    if (state.busyAction === 'return') return 'Returning control…';
    if (mode === 'human') return mine ? 'You’re in control' : 'Another operator';
    return (Object.hasOwn(CONTROL_LABELS, mode) ? CONTROL_LABELS[mode] : undefined) || 'Checking control…';
  },

  /** Take control / Release, and Resume after a pause. */
  buttons(state, mine) {
    const [button, resume] = [ControlBar.el('control-action'), ControlBar.el('control-resume')];
    const available = state.supported || state.localClients > 0 || state.local;
    button.hidden = !available || (state.mode === 'human' && !state.mine);
    button.disabled = state.busy || (state.taking && !state.mine);
    button.textContent = mine ? 'Release to agent' : 'Take control';
    resume.hidden = state.mode !== 'paused' || state.taking || state.busy;
    resume.disabled = state.busy;
  },

  /** Marks the page actions blocked (or not) and makes the address bar read-only while watch-only. */
  gate(interactive) {
    for (const id of PAGE_ACTIONS) {
      const element = ControlBar.el(id);
      if (!element) continue;
      element.toggleAttribute('data-control-blocked', !interactive);
      element.setAttribute('aria-disabled', String(!interactive));
    }
    ControlBar.el('url-bar').readOnly = !interactive;
  },

  /** Asks for a control change and draws the answer. */
  async change(action, trigger) {
    ControlBar.error = '';
    trigger.disabled = true;
    const result = await oyaBrowser.changeControl(action);
    ControlBar.error = result.error || '';
    ControlBar.render(result.state);
  },

  /** Take control, or release it if it is already yours. */
  toggle() {
    const { current } = ControlBar;
    ControlBar.change(current.mode === 'human' && current.mine ? 'return' : 'acquire', ControlBar.el('control-action'));
  },

  /** Refuses a click on a page action while watch-only, pointing at Take control instead. */
  guard(event) {
    if (ControlBar.current?.interactive || !event.target.closest(GUARDED)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    ControlBar.el('control-action').focus();
  },

  /** A new state from the main process; a real change clears the last error. */
  received(state) {
    const { current } = ControlBar;
    const moved = state.mode !== current?.mode || state.revision !== current?.revision;
    if (moved || state.connected !== current?.connected) ControlBar.error = '';
    ControlBar.render(state);
  },

  /** Wires the buttons, the guard and the state feed, then draws the current state. */
  start() {
    ControlBar.el('control-action').addEventListener('click', ControlBar.toggle);
    ControlBar.el('control-resume').addEventListener('click', () =>
      ControlBar.change('return', ControlBar.el('control-resume')),
    );
    // Capture prevents local actions before existing navigation/recording listeners.
    document.addEventListener('click', ControlBar.guard, true);
    oyaBrowser.onControlState(ControlBar.received);
    oyaBrowser.getControlState().then(ControlBar.render);
  },
};

ControlBar.start();
