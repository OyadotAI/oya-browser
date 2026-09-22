/**
 * The workflow studio's shared state and helpers: the last workspace
 * snapshot, what the studio may do now, the selected step and tab, where
 * messages go, and small builders for its controls. The studio's views and
 * actions live beside this file.
 */
/* global oyaBrowser, Dom, StudioView */
/* exported Studio */

/** Studio state and helpers. */
const Studio = {
  /** The last workspace snapshot from the main process. */
  state: undefined,
  /** What the studio may do now (see StudioView.mode), derived once per snapshot. */
  mode: { locked: true, runnable: false, stage: 'empty', stored: true, recording: false, running: false },
  /** The selected step's id. */
  selected: undefined,
  /** A recording start or stop is in flight. */
  busy: false,
  /** A save to Oya is in flight. */
  saving: false,
  /** The recording just finished in this session, so the save card says so and comes into view. */
  justFinished: false,
  /** Bring the save card into view on the next render. */
  revealFinish: false,
  /** The last workspace command in flight: commands run one at a time, in order. */
  queue: Promise.resolve(),
  /** The draft on screen, so a message about another draft is cleared when it changes. */
  shownDraft: undefined,
  /** The studio tab in view: steps, run or code. */
  tab: 'steps',
  /** What each view last drew, so unchanged views are not rebuilt. */
  signatures: {
    steps: '',
    editor: '',
    variables: '',
    inputs: '',
    library: '',
    runHistory: '',
    events: '',
    code: '',
  },

  /** The name people see for each action. */
  NAMES: {
    navigate: 'Navigate',
    go_back: 'Back',
    go_forward: 'Forward',
    type: 'Fill',
    click: 'Click',
    double_click: 'Double-click',
    hover: 'Hover',
    select_option: 'Select',
    press_key: 'Press key',
    scroll: 'Scroll',
    upload_file: 'Upload',
    wait: 'Wait for element',
    assert_visible: 'Assert visible',
    assert_text: 'Assert text',
    assert_value: 'Assert value',
    assert_url: 'Assert URL',
    assert_page: 'Check page',
    unsupported_frame: 'Not recorded: embedded frame',
    unsupported_drop: 'Not recorded: drag and drop',
    unsupported_click: 'Not recorded: canvas click',
    checkpoint: 'Human checkpoint',
  },

  /** The studio tabs, in order. */
  TABS: ['steps', 'run', 'code'],

  /** Where each message shows: under the action that caused it. */
  SLOTS: ['record-result', 'save-result', 'code-result'],

  /** Run statuses during which the draft is locked. */
  ACTIVE: ['starting', 'running', 'paused', 'stopping'],

  /** What a playbook may be called when it is saved to Oya. */
  NAME_RULE: /^[\w-]{1,64}$/,

  /** An action's display name (the raw action when it has none). */
  name: (action) => (Object.hasOwn(Studio.NAMES, action) ? Studio.NAMES[action] : undefined) || action,

  /** Whether a run is still going. */
  isActive: (run) => Studio.ACTIVE.includes(run?.status),

  /** "1 step", "3 steps". */
  plural: (count, word) => `${count} ${word}${count === 1 ? '' : 's'}`,

  /** Whether the name typed for the playbook can be saved to Oya. */
  validName: () => Studio.NAME_RULE.test(Dom.byId('record-name').value.trim()),

  /** An error's message without Electron's IPC wrapper. */
  cleanError: (error) =>
    String(error?.message ?? error).replace(/^Error invoking remote method '[^']+': (?:Error: )?/, ''),

  /** Shows a message in `slot`, under the action it is about ('' clears it). */
  say(message = '', error = false, slot = 'record-result') {
    Dom.byId(slot).textContent = message;
    Dom.byId(slot).classList.toggle('error', error);
  },

  /** Clears every message. */
  clearMessages() {
    for (const slot of Studio.SLOTS) Studio.say('', false, slot);
  },

  /** A small text button. */
  button(text, fn, title = text) {
    const el = Dom.node('button', text, 'text-button');
    el.type = 'button';
    el.title = title;
    el.addEventListener('click', fn);
    return el;
  },

  /** A labelled input that reports changes: a number field reports a number, or nothing when emptied. */
  field(label, value, change, type = 'text', key = label) {
    const group = Dom.node('label', null, 'studio-field');
    group.append(Dom.node('span', label));
    const input = Dom.node('input');
    Object.assign(input, { type, value: value ?? '' });
    input.dataset.key = key;
    input.addEventListener('change', () => change(Studio.fieldValue(input)));
    group.append(input);
    return group;
  },

  /** A field's value: text as typed, a number field as a number or undefined when empty. */
  fieldValue(input) {
    if (input.type !== 'number') return input.value;
    return input.value === '' ? undefined : Number(input.value);
  },

  /** Rebuilds `host` with `draw`, keeping focus on the same field when it was inside. */
  keepFocus(host, draw) {
    const active = document.activeElement;
    const key = active?.closest?.('#' + host.id) ? active.dataset?.key : undefined;
    draw();
    if (key) [...host.querySelectorAll('[data-key]')].find((el) => el.dataset.key === key)?.focus();
  },

  /**
   * Sends a workspace command after the ones before it, showing the new state,
   * or its error in `slot`; resolves to the state or undefined. `cmd` may be a
   * function, built when its turn comes, so an edit made before the last one's
   * answer arrived builds on that answer instead of overwriting it.
   */
  command(cmd, slot = 'record-result') {
    const sent = Studio.queue.then(() => Studio.send(cmd, slot));
    Studio.queue = sent.catch(() => {});
    return sent;
  },

  /** Sends one command now and shows what came back. */
  async send(cmd, slot) {
    Studio.say('', false, slot);
    try {
      const next = await oyaBrowser.workspace(typeof cmd === 'function' ? cmd() : cmd);
      StudioView.render(next);
      return next;
    } catch (error) {
      Studio.say(Studio.cleanError(error), true, slot);
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
