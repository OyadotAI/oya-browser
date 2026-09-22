/**
 * The studio's step list: one row per step with its number (or breakpoint),
 * action, target and any issue; a click selects it.
 */
/* global Dom, RendererConstants, Studio, StudioView */
/* exported StudioSteps */

/** Locator kinds that read as words on the page, in the order a row prefers them. */
const READABLE_KINDS = ['label', 'role', 'text', 'placeholder'];

/** The step list. */
const StudioSteps = {
  /** Redraws the list when the steps, selection or lock changed, keeping the scroll position. */
  render(d, recording, active) {
    const signature = JSON.stringify([d.steps, d.variables, Studio.selected, recording, active, Studio.state.issues]);
    if (signature === Studio.signatures.steps) return;
    Studio.signatures.steps = signature;
    const list = Dom.byId('record-steps');
    const scroll = list.scrollTop;
    list.replaceChildren(...(d.steps.length ? d.steps.map(StudioSteps.row) : [StudioSteps.empty()]));
    list.scrollTop = StudioSteps.grew(d, recording) ? list.scrollHeight : scroll;
  },

  /** Whether a recording just added a step: the list then follows it, as a person acts, instead of staying put. */
  grew(d, recording) {
    const grew = recording && d.steps.length > (StudioSteps.shown ?? d.steps.length);
    StudioSteps.shown = d.steps.length;
    return grew;
  },

  /** What an empty draft shows. */
  empty() {
    const empty = Dom.node('div', null, 'record-empty');
    const hint = 'Record what you do in the page, or add steps by hand.';
    empty.append(Dom.node('h3', 'No steps yet'), Dom.node('p', hint));
    return empty;
  },

  /** One step's row. */
  row(step, index) {
    const selected = Studio.selected === step.id;
    const row = Dom.node('button', null, StudioSteps.rowClass(step, selected));
    row.type = 'button';
    row.setAttribute('aria-pressed', selected);
    row.append(Dom.node('span', StudioSteps.number(step, index), 'step-number'), StudioSteps.copy(step));
    StudioSteps.markIssue(row, step);
    row.addEventListener('click', () => StudioSteps.select(step.id));
    return row;
  },

  /** A step that blocks a test run says why, on hover and to a screen reader. */
  markIssue(row, step) {
    const issue = Studio.state.issues.find((item) => item.stepId === step.id);
    if (!issue) return;
    const mark = Dom.node('span', '!', 'step-issue');
    mark.title = issue.message;
    mark.setAttribute('aria-label', 'Blocks a test run: ' + issue.message);
    row.append(mark);
  },

  /** A row's classes: selected, and disabled when the step is switched off. */
  rowClass(step, selected) {
    return 'studio-step' + (selected ? ' selected' : '') + (!step.enabled ? ' disabled' : '');
  },

  /** A step's number, or a dot when it has a breakpoint. */
  number(step, index) {
    return step.breakpoint ? '●' : String(index + 1).padStart(RendererConstants.STEP_NUMBER_DIGITS, '0');
  },

  /** A row's action, target, and what it enters or expects. */
  copy(step) {
    const copy = Dom.node('span', null, 'step-copy');
    const parts = [StudioSteps.target(step), StudioSteps.detail(step)].filter(Boolean);
    const value = parts.join(' · ') || 'Select to configure';
    copy.append(Dom.node('strong', Studio.name(step.action)), Dom.node('span', value, 'step-value'));
    return copy;
  },

  /** What the step enters, picks, presses, goes to or expects; a secret shows only that it is one. */
  detail(step) {
    if (Object.hasOwn(StudioSteps.DETAILS, step.action)) return StudioSteps.DETAILS[step.action](step);
    return step.url || step.key || step.expected || step.captureIssue;
  },

  /** Action → what its row says after the target. */
  DETAILS: {
    type: (step) => StudioSteps.typed(step.text),
    select_option: (step) => step.option && `“${step.option}”`,
    scroll: (step) => `${step.direction || 'down'} ${step.amount || ''}`.trim(),
    go_back: () => 'to the previous page',
    go_forward: () => 'to the next page',
    assert_page: (step) => StudioSteps.page(step.expected, step.params),
  },

  /** A checked page as a row shows it: host and path, and the parameters it holds to, not a page of tokens. */
  page(url, params) {
    try {
      const u = new URL(url);
      const held = StudioSteps.held(u, params);
      return StudioSteps.clip(u.host + u.pathname + (held ? ` (${held})` : ''));
    } catch {
      return StudioSteps.clip(String(url || ''));
    }
  },

  /** The named parameters of `u` as `k=v, …`. */
  held(u, params) {
    const names = String(params || '')
      .split(',')
      .filter(Boolean);
    return names.map((k) => `${k}=${u.searchParams.get(k) ?? ''}`).join(', ');
  },

  /** Text cut to what a row has room for. */
  clip(text) {
    const max = RendererConstants.ROW_TEXT_CHARS;
    return text.length > max ? text.slice(0, max - 1) + '…' : text;
  },

  /** A Fill's text as a row shows it: quoted, or marked secret when it is one. */
  typed(text) {
    if (!text) return '(empty)';
    const variable = /^\{\{(\w+)\}\}$/.exec(text)?.[1];
    if (variable && Studio.state?.draft.variables?.[variable]?.secret) return `secret {{${variable}}}`;
    return `“${StudioSteps.clip(text.replace(/\s+/g, ' '))}”`;
  },

  /**
   * What the step aims at, in the words a person sees on the page. Replay tries
   * the most robust locator first, often a CSS id; the row names the element by
   * its label, role, text or placeholder when it has one, then by what was
   * recorded about it, and only then by a shortened selector.
   */
  target(step) {
    const candidates = step.candidates || [];
    const readable = candidates.find((c) => READABLE_KINDS.includes(c.kind));
    if (readable) return readable.value;
    const short = StudioSteps.shortSelector(candidates[0]?.value);
    const kind = ['checkbox', 'radio'].includes(step.el?.type) ? step.el.type + ' ' : '';
    return StudioSteps.recordedName(step.el) || (short && kind + short);
  },

  /** A name from the recorded element: its test id, name or id, with what kind of element it is. */
  recordedName(el) {
    const name = el?.testId || el?.name || el?.domId;
    if (!name) return '';
    return el.type && el.type !== 'button' ? `${name} ${el.type}` : name;
  },

  /** A CSS selector made readable: `[id="x"]` reads as `#x`, and only the last two levels of a path show. */
  shortSelector(selector) {
    if (!selector) return '';
    const plain = selector.replace(/\[id="([^"]+)"\]/g, '#$1');
    const levels = plain.split(' > ');
    const shown = RendererConstants.SELECTOR_LEVELS_SHOWN;
    return levels.length > shown ? '… > ' + levels.slice(-shown).join(' > ') : plain;
  },

  /** Selects a step and redraws. */
  select(id) {
    Studio.selected = id;
    StudioView.render(Studio.state);
  },
};
