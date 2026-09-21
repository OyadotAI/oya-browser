/**
 * The workflow studio's shared state and helpers: the last workspace
 * snapshot, the selected step and tab, and small builders for its controls.
 * The studio's views and actions live beside this file.
 */
/* global oyaBrowser, Dom, StudioView */
/* exported Studio */

/** Studio state and helpers. */
const Studio = {
  /** The last workspace snapshot from the main process. */
  state: undefined,
  /** The selected step's id. */
  selected: undefined,
  /** A recording toggle or save is in flight. */
  busy: false,
  /** The studio tab in view: steps, run or inspect. */
  tab: 'steps',
  /** The panel is at its expanded width. */
  expanded: false,
  /** What each view last drew, so unchanged views are not rebuilt. */
  signatures: { steps: '', editor: '', variables: '', library: '', runHistory: '', code: '' },

  /** The name people see for each action. */
  NAMES: {
    navigate: 'Navigate',
    type: 'Fill',
    click: 'Click',
    select_option: 'Select',
    press_key: 'Press key',
    scroll: 'Scroll',
    upload_file: 'Upload',
    wait: 'Wait for element',
    assert_visible: 'Assert visible',
    assert_text: 'Assert text',
    assert_value: 'Assert value',
    assert_url: 'Assert URL',
    checkpoint: 'Human checkpoint',
  },

  /** The studio tabs, in order. */
  TABS: ['steps', 'run', 'inspect'],

  /** Run statuses during which the draft is locked. */
  ACTIVE: ['starting', 'running', 'paused', 'stopping'],

  /** An action's display name (the raw action when it has none). */
  name: (action) => (Object.hasOwn(Studio.NAMES, action) ? Studio.NAMES[action] : undefined) || action,

  /** Whether a run is still going. */
  isActive: (run) => Studio.ACTIVE.includes(run?.status),

  /** Shows a status message under the studio ('' clears it). */
  say(message = '', error = false) {
    Dom.byId('record-result').textContent = message;
    Dom.byId('record-result').classList.toggle('error', error);
  },

  /** A small text button. */
  button(text, fn, title = text) {
    const el = Dom.node('button', text, 'text-button');
    el.type = 'button';
    el.title = title;
    el.addEventListener('click', fn);
    return el;
  },

  /** A labelled input that reports changes (as a number for number inputs). */
  field(label, value, change, type = 'text') {
    const group = Dom.node('label', null, 'studio-field');
    group.append(Dom.node('span', label));
    const input = Dom.node('input');
    input.type = type;
    input.value = value ?? '';
    input.addEventListener('change', () => change(type === 'number' ? Number(input.value) : input.value));
    group.append(input);
    return group;
  },

  /** Sends a workspace command and shows the new state, or the error without IPC noise. */
  async command(cmd) {
    try {
      const next = await oyaBrowser.workspace(cmd);
      StudioView.render(next);
      return next;
    } catch (error) {
      Studio.say(error.message.replace(/^Error invoking remote method '[^']+': Error: /, ''), true);
    }
  },

  /** Shows one studio tab. */
  selectTab(value) {
    Studio.tab = value;
    document.querySelectorAll('[data-studio]').forEach((el) => {
      const active = el.dataset.studio === Studio.tab;
      el.setAttribute('aria-selected', active);
      el.tabIndex = active ? 0 : -1;
    });
    for (const name of Studio.TABS) Dom.byId('studio-' + name).hidden = name !== Studio.tab;
  },

  /** Left and right arrows move between the studio tabs. */
  tabKey(event) {
    if (!['ArrowLeft', 'ArrowRight'].includes(event.key)) return;
    event.preventDefault();
    const tabs = Studio.TABS;
    Studio.selectTab(
      tabs[(tabs.indexOf(Studio.tab) + (event.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length],
    );
    document.querySelector(`[data-studio="${Studio.tab}"]`).focus();
  },
};

document.querySelectorAll('[data-studio]').forEach((el) => {
  el.addEventListener('click', () => Studio.selectTab(el.dataset.studio));
  el.addEventListener('keydown', Studio.tabKey);
});
