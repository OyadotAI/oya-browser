/**
 * The selected step's editor: enable, breakpoint, move, duplicate, delete;
 * its target, value, frames and timeout; and running up to it.
 */
/* global Dom, Studio, StudioActions */
/* exported StepEditor */

/** The step editor. */
const StepEditor = {
  /** Actions that act on an element, so have a target to edit. */
  TARGETED: ['click', 'type', 'select_option', 'upload_file', 'wait', 'assert_visible', 'assert_text', 'assert_value'],

  /** Locator strategies, in the order the picker lists them. */
  STRATEGIES: ['testId', 'role', 'label', 'text', 'placeholder', 'css'],

  /** The field that holds each action's value. */
  VALUE_KEYS: {
    navigate: 'url',
    type: 'text',
    select_option: 'option',
    upload_file: 'file',
    press_key: 'key',
    assert_url: 'expected',
    assert_text: 'expected',
    assert_value: 'expected',
  },

  /** Redraws the editor for `step` (hidden when none) when it or the lock changed. */
  render(step, locked) {
    const signature = JSON.stringify([step, locked]);
    if (signature === Studio.signatures.editor) return;
    Studio.signatures.editor = signature;
    StepEditor.draw(Dom.byId('step-editor'), step, locked);
  },

  /** Empties the editor, then fills it for `step` with every control locked or not. */
  draw(host, step, locked) {
    host.hidden = !step;
    host.replaceChildren();
    if (!step) return;
    StepEditor.fill(host, step);
    host.querySelectorAll('button,input,select').forEach((el) => (el.disabled = locked));
  },

  /** Every part of the editor, in order. */
  fill(host, step) {
    const patch = (value) => Studio.command({ type: 'update', id: step.id, patch: value });
    host.append(StepEditor.heading(step), StepEditor.toolbar(step, patch));
    if (StepEditor.TARGETED.includes(step.action)) StepEditor.target(host, step, patch);
    StepEditor.valueField(host, step, patch);
    host.append(StepEditor.framesField(step, patch));
    host.append(Studio.field('Timeout (milliseconds)', step.timeout, (timeout) => patch({ timeout }), 'number'));
    if (step.captureIssue) host.append(Dom.node('p', step.captureIssue, 'studio-issue'));
    host.append(Studio.button('Run to this step', () => StudioActions.validate({ runTo: step.id })));
  },

  /** The frame selectors, one per nesting level, written joined by arrows. */
  framesField(step, patch) {
    const frames = (step.frames || []).join(' → ');
    const label = 'Frame selectors (one per level, separated by →)';
    return Studio.field(label, frames, (value) => patch({ frames: StepEditor.parseFrames(value) }));
  },

  /** "a → b" as ['a', 'b'], without empty levels. */
  parseFrames(value) {
    return value
      .split('→')
      .map((s) => s.trim())
      .filter(Boolean);
  },

  /** "Step n" and the action's name. */
  heading(step) {
    const header = Dom.node('div', null, 'editor-heading');
    header.append(
      Dom.node('h3', 'Step ' + (Studio.state.draft.steps.indexOf(step) + 1)),
      Dom.node('span', Studio.name(step.action), 'eyebrow'),
    );
    return header;
  },

  /** Enable, breakpoint, move, duplicate and delete. */
  toolbar(step, patch) {
    const controls = Dom.node('div', null, 'studio-toolbar');
    controls.append(...StepEditor.toggles(step, patch), ...StepEditor.stepCommands(step));
    return controls;
  },

  /** Enable or disable the step, and set or clear its breakpoint. */
  toggles(step, patch) {
    const breakpoint = step.breakpoint ? 'Remove breakpoint' : 'Breakpoint';
    return [
      Studio.button(step.enabled ? 'Disable' : 'Enable', () => patch({ enabled: !step.enabled })),
      Studio.button(breakpoint, () => patch({ breakpoint: !step.breakpoint })),
    ];
  },

  /** Move up, move down, duplicate and delete. */
  stepCommands(step) {
    const run = (command) => () => Studio.command({ ...command, id: step.id });
    return [
      Studio.button('↑', run({ type: 'move', delta: -1 }), 'Move step up'),
      Studio.button('↓', run({ type: 'move', delta: 1 }), 'Move step down'),
      Studio.button('Duplicate', run({ type: 'duplicate' })),
      Studio.button('Delete', run({ type: 'delete' })),
    ];
  },

  /** The target: its strategy, value, role, the picker, and recorded alternatives. */
  target(host, step, patch) {
    const target = step.candidates?.[0] || { kind: 'css', value: '' };
    const replace = (changes) => patch({ candidates: [{ ...target, ...changes }, ...step.candidates.slice(1)] });
    host.append(StepEditor.strategy(target, replace));
    host.append(Studio.field('Target', target.value, (value) => replace({ value })));
    if (target.kind === 'role')
      host.append(Studio.field('ARIA role', target.role || 'button', (role) => replace({ role })));
    host.append(Studio.button('Pick target on page', () => StepEditor.pick(step)));
    if (step.candidates.length > 1) host.append(StepEditor.alternatives(step, patch));
  },

  /** Picks the target in the page inspector. */
  async pick(step) {
    Studio.say('Click an element in the page inspector. Escape cancels.');
    await Studio.command({ type: 'pick', id: step.id });
  },

  /** The locator strategy picker. */
  strategy(target, replace) {
    const group = Dom.node('label', null, 'studio-field');
    group.append(Dom.node('span', 'Target strategy'));
    const select = Dom.node('select');
    select.append(...StepEditor.STRATEGIES.map(StepEditor.strategyOption));
    select.value = target.kind;
    select.addEventListener('change', () => replace({ kind: select.value }));
    group.append(select);
    return group;
  },

  /** One strategy in the picker. */
  strategyOption(kind) {
    const option = Dom.node('option', kind);
    option.value = kind;
    return option;
  },

  /** The other recorded locators; choosing one makes it the target. */
  alternatives(step, patch) {
    const alternatives = Dom.node('details', null, 'studio-details');
    alternatives.append(Dom.node('summary', `${step.candidates.length - 1} recorded alternatives`));
    step.candidates.slice(1).forEach((c) => {
      const choose = () => patch({ candidates: [c, ...step.candidates.filter((item) => item !== c)] });
      alternatives.append(Studio.button(`${c.kind}: ${c.value}`, choose));
    });
    return alternatives;
  },

  /** The action's value field (URL, text, key, expected result…), if it has one. */
  valueField(host, step, patch) {
    const valueKey = Object.hasOwn(StepEditor.VALUE_KEYS, step.action) ? StepEditor.VALUE_KEYS[step.action] : undefined;
    if (!valueKey) return;
    const label =
      valueKey === 'expected' ? 'Expected result' : valueKey === 'text' ? 'Value or {{variable}}' : valueKey;
    host.append(Studio.field(label, step[valueKey], (value) => patch({ [valueKey]: value })));
  },
};
