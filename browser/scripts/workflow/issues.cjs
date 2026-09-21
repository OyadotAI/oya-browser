/**
 * What stops a draft from running: problems a person has to fix before
 * validation or export, and the variables the steps refer to.
 */
const { ACTIONS, TARGETED, PLACEHOLDER } = require('./rules.cjs');

/** Each rule a step must pass: when it fails, and what to tell the person. */
const STEP_RULES = [
  [
    (step) => !ACTIONS.has(step.action),
    (step) => `Unsupported interaction: ${step.action}. Replace or disable this step.`,
  ],
  [(step) => TARGETED.includes(step.action) && !step.candidates.length, () => 'Pick a target before validation.'],
  [
    (step) => step.action === 'navigate' && !/^https?:\/\//i.test(step.url || ''),
    () => 'Navigation requires an HTTP or HTTPS URL.',
  ],
  [(step) => step.candidates.some((c) => !c.value.trim()), () => 'Target cannot be empty.'],
  [(step) => step.captureIssue, (step) => step.captureIssue],
];

/** The problems with one enabled step, as messages, in rule order. */
function stepProblems(step) {
  return STEP_RULES.filter(([fails]) => fails(step)).map(([, message]) => message(step));
}

/** Every problem in the draft's enabled steps, as `{ stepId, message }`. */
function issues(draft) {
  return draft.steps
    .filter((step) => step.enabled)
    .flatMap((step) => stepProblems(step).map((message) => ({ stepId: step.id, message })));
}

/** The distinct `{{variable}}` names the steps use, in order of first use. */
function variableNames(draft) {
  return [...new Set([...JSON.stringify(draft.steps).matchAll(PLACEHOLDER)].map((match) => match[1]))];
}

module.exports = { issues, variableNames };
