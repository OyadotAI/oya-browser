/**
 * The studio's step list: one row per step with its number (or breakpoint),
 * action, target and any issue; a click selects it.
 */
/* global Dom, RendererConstants, Studio, StudioView */
/* exported StudioSteps */

/** The step list. */
const StudioSteps = {
  /** Redraws the list when the steps, selection or lock changed, keeping the scroll position. */
  render(d, recording, active) {
    const signature = JSON.stringify([d.steps, Studio.selected, recording, active, Studio.state.issues]);
    if (signature === Studio.signatures.steps) return;
    Studio.signatures.steps = signature;
    const list = Dom.byId('record-steps');
    const scroll = list.scrollTop;
    list.replaceChildren(...(d.steps.length ? d.steps.map(StudioSteps.row) : [StudioSteps.empty()]));
    list.scrollTop = scroll;
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

  /** A row's action and target. */
  copy(step) {
    const copy = Dom.node('span', null, 'step-copy');
    const value = step.candidates?.[0]?.value || step.url || step.key || 'Select to configure';
    copy.append(Dom.node('strong', Studio.name(step.action)), Dom.node('span', value, 'step-value'));
    return copy;
  },

  /** Selects a step and redraws. */
  select(id) {
    Studio.selected = id;
    StudioView.render(Studio.state);
  },
};
