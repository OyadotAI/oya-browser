/**
 * The selected step's editor: a heading, an icon row (on or off, breakpoint,
 * move) with a More menu (duplicate, delete, run to here), the target and
 * value, and the rarely needed fields under Advanced. Every handler reads the
 * step as it is when the event fires, never a copy from the last draw, and a
 * redraw keeps focus on the field the person was in.
 */
/* global Dom, Studio, StudioActions, ShellIcons */
/* exported StepEditor */

/** The step editor. */
const StepEditor = {
  /** Actions that act on an element, so have a target to edit. */
  TARGETED: [
    'click',
    'double_click',
    'hover',
    'type',
    'select_option',
    'upload_file',
    'wait',
    'assert_visible',
    'assert_text',
    'assert_value',
  ],

  /** Locator strategies, in the order the picker lists them, with the names people see. */
  STRATEGIES: { testId: 'Test id', role: 'Role', label: 'Label', text: 'Text', placeholder: 'Placeholder', css: 'CSS' },

  /** The field that holds each action's value, and its label. */
  VALUE_FIELDS: {
    navigate: ['url', 'URL'],
    type: ['text', 'Text or {{variable}}'],
    select_option: ['option', 'Option'],
    upload_file: ['file', 'File'],
    press_key: ['key', 'Key'],
    scroll: ['direction', 'Direction (up or down)'],
    assert_url: ['expected', 'Expected URL'],
    assert_page: ['expected', 'Page (the query is ignored)'],
    assert_text: ['expected', 'Expected text'],
    assert_value: ['expected', 'Expected value'],
  },

  /** Redraws the editor for `step` (hidden when none) when it, its place, the lock or runnability changed. */
  render(step, locked) {
    const index = step ? Studio.state.draft.steps.indexOf(step) : -1;
    const signature = JSON.stringify([step, index, locked, Studio.mode.runnable]);
    if (signature === Studio.signatures.editor) return;
    Studio.signatures.editor = signature;
    const host = Dom.byId('step-editor');
    Studio.keepFocus(host, () => StepEditor.draw(host, step, locked));
  },

  /** The step with `id` as it is now. */
  current(id) {
    return Studio.state.draft.steps.find((s) => s.id === id);
  },

  /** Empties the editor, then fills it for `step` with every control locked or not. */
  draw(host, step, locked) {
    host.hidden = !step;
    host.replaceChildren();
    if (!step) return;
    StepEditor.fill(host, step);
    host.querySelectorAll('button,input,select').forEach((el) => (el.disabled = el.disabled || locked));
  },

  /** Every part of the editor, in order. */
  fill(host, step) {
    const patch = (value) => Studio.command({ type: 'update', id: step.id, patch: value });
    host.append(StepEditor.heading(step));
    if (StepEditor.TARGETED.includes(step.action)) StepEditor.target(host, step);
    StepEditor.valueField(host, step, patch);
    if (step.captureIssue) host.append(Dom.node('p', step.captureIssue, 'studio-issue'));
    StepEditor.issue(host, step);
    host.append(StepEditor.advanced(step, patch));
  },

  /** Why this step blocks a test run, if it does. */
  issue(host, step) {
    const issue = Studio.state.issues.find((item) => item.stepId === step.id);
    if (issue) host.append(Dom.node('p', issue.message, 'studio-issue'));
  },

  /** "Step n · Action" and the icon row. */
  heading(step) {
    const header = Dom.node('div', null, 'editor-heading');
    const title = `Step ${Studio.state.draft.steps.indexOf(step) + 1} · ${Studio.name(step.action)}`;
    header.append(Dom.node('h3', title), StepEditor.iconRow(step));
    return header;
  },

  /** On or off, breakpoint, up, down, and More. */
  iconRow(step) {
    const row = Dom.node('div', null, 'editor-icons');
    const move = (delta) => () => Studio.command({ type: 'move', id: step.id, delta });
    row.append(...StepEditor.toggles(step));
    row.append(StepEditor.icon('up', 'Move up', move(-1)), StepEditor.icon('down', 'Move down', move(1)));
    row.append(StepEditor.more(step));
    return row;
  },

  /** On or off, and the breakpoint; each flips the step as it is now. */
  toggles(step) {
    const toggle = (key) => () =>
      Studio.command(() => ({ type: 'update', id: step.id, patch: { [key]: !StepEditor.current(step.id)?.[key] } }));
    const breakpoint = step.breakpoint ? 'Remove breakpoint' : 'Pause here';
    return [
      StepEditor.icon('power', step.enabled ? 'Turn step off' : 'Turn step on', toggle('enabled'), !step.enabled),
      StepEditor.icon('record', breakpoint, toggle('breakpoint'), step.breakpoint),
    ];
  },

  /** An icon button; `on` marks a toggle that is set. */
  icon(name, label, fn, on = false) {
    const button = Dom.node('button', null, 'icon-button' + (on ? ' on' : ''));
    Object.assign(button, { type: 'button', title: label, innerHTML: ShellIcons.icon(name) });
    button.setAttribute('aria-label', label);
    button.addEventListener('click', fn);
    return button;
  },

  /** Duplicate, delete and run to here, in a menu. */
  more(step) {
    const menu = Dom.node('details', null, 'editor-more');
    const summary = Dom.node('summary', null, 'icon-button');
    Object.assign(summary, { title: 'More', innerHTML: ShellIcons.icon('more') });
    summary.setAttribute('aria-label', 'More');
    menu.append(summary, StepEditor.menuItems(step));
    return menu;
  },

  /** Duplicate, delete, and run to here (only when the workflow can run). */
  menuItems(step) {
    const items = Dom.node('div', null, 'editor-menu');
    const command = (type) => () => Studio.command({ type, id: step.id });
    const run = Studio.button('Run to here', () => StudioActions.validate({ runTo: step.id }));
    run.disabled = !Studio.mode.runnable;
    items.append(Studio.button('Duplicate', command('duplicate')), Studio.button('Delete', command('delete')), run);
    return items;
  },

  /** The target: strategy and value on one row, and the picker. */
  target(host, step) {
    const target = step.candidates?.[0] || { kind: 'css', value: '' };
    const row = Dom.node('div', null, 'editor-target');
    const value = Studio.field('Target', target.value, StepEditor.replacer(step, 'value'));
    row.append(StepEditor.strategy(step, target), value);
    host.append(
      row,
      Studio.button('Pick on page', () => StepEditor.pick(step)),
    );
  },

  /** A change to one key of the step's target, applied to the target as it is now. */
  replacer(step, key) {
    return (value) =>
      Studio.command(() => {
        const candidates = StepEditor.current(step.id)?.candidates || [];
        const target = { ...(candidates[0] || { kind: 'css', value: '' }), [key]: value };
        return { type: 'update', id: step.id, patch: { candidates: [target, ...candidates.slice(1)] } };
      });
  },

  /** Picks the target in the page inspector. */
  async pick(step) {
    if (await Studio.command({ type: 'pick', id: step.id }))
      Studio.say('Click an element in the page. Escape cancels.');
  },

  /** The locator strategy picker. */
  strategy(step, target) {
    const group = Dom.node('label', null, 'studio-field');
    const select = Dom.node('select');
    select.dataset.key = 'strategy';
    select.append(...Object.entries(StepEditor.STRATEGIES).map(StepEditor.strategyOption));
    select.value = target.kind;
    select.addEventListener('change', () => StepEditor.replacer(step, 'kind')(select.value));
    group.append(Dom.node('span', 'Find by'), select);
    return group;
  },

  /** One strategy in the picker. */
  strategyOption([kind, label]) {
    const option = Dom.node('option', label);
    option.value = kind;
    return option;
  },

  /** The action's value field (URL, text, key, expected result), if it has one. */
  valueField(host, step, patch) {
    if (!Object.hasOwn(StepEditor.VALUE_FIELDS, step.action)) return;
    const [key, label] = StepEditor.VALUE_FIELDS[step.action];
    host.append(Studio.field(label, step[key], (value) => patch({ [key]: value })));
  },

  /** The rarely needed fields: ARIA role, frames, timeout and the other recorded targets. */
  advanced(step, patch) {
    const details = Dom.node('details', null, 'studio-details');
    details.append(Dom.node('summary', 'Advanced'));
    if (step.candidates?.[0]?.kind === 'role') details.append(StepEditor.roleField(step));
    details.append(StepEditor.framesField(step, patch));
    details.append(Studio.field('Timeout (ms)', step.timeout, (timeout) => patch({ timeout }), 'number'));
    if (step.candidates?.length > 1) details.append(StepEditor.alternatives(step, patch));
    return details;
  },

  /** The ARIA role a role target matches. */
  roleField(step) {
    return Studio.field('ARIA role', step.candidates[0].role || 'button', StepEditor.replacer(step, 'role'));
  },

  /** The frame selectors, one per nesting level, written joined by arrows. */
  framesField(step, patch) {
    const frames = (step.frames || []).join(' → ');
    return Studio.field('Frames (outer → inner)', frames, (value) => patch({ frames: StepEditor.parseFrames(value) }));
  },

  /** "a → b" as ['a', 'b'], without empty levels. */
  parseFrames(value) {
    return value
      .split('→')
      .map((s) => s.trim())
      .filter(Boolean);
  },

  /** The other recorded targets; choosing one makes it the target. */
  alternatives(step, patch) {
    const group = Dom.node('div', null, 'editor-alternatives');
    group.append(Dom.node('span', 'Other recorded targets', 'eyebrow'));
    step.candidates.slice(1).forEach((c) => {
      const choose = () => patch({ candidates: [c, ...step.candidates.filter((item) => item !== c)] });
      group.append(Studio.button(`${StepEditor.STRATEGIES[c.kind] || c.kind}: ${c.value}`, choose));
    });
    return group;
  },
};
