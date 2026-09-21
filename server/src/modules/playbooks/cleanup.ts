/**
 * Cleaning legacy recording noise out of stored playbooks as they are read.
 */

/** A recorded step on a hidden input: never something a person did. */
const hidden = (step) => step.el?.tag === 'input' && (step.el.type === 'hidden' || step.el.inputType === 'hidden');
/** Names of the placeholders anywhere in `value`. */
const placeholders = (value) =>
  new Set([...JSON.stringify(value).matchAll(/\{\{(\w+)(?:\|[^}]*)?\}\}/g)].map((match) => match[1]));

/** Remove legacy recording noise only when the stored data proves its origin.
 * DOM IDs and offscreen/visible flags are not evidence that an input was hidden.
 * Returns a copy; callers retain the original sealed record for recovery.
 */
export function cleanPlaybook(original) {
  if (!Array.isArray(original?.steps)) return original;
  const pb = structuredClone(original);
  const labels = new Set(pb.labels || []);
  const secrets = new Set(pb.secrets || []);
  dropHiddenInputs(pb);
  restoreClickLabels(pb, labels, secrets);
  pruneVariables(pb);
  return pb;
}

/** Drops steps on hidden inputs, keeping `healedFrom` on the same step. */
function dropHiddenInputs(pb) {
  if (Number.isInteger(pb.healedFrom)) {
    pb.healedFrom -= pb.steps.slice(0, pb.healedFrom).filter(hidden).length;
  }
  pb.steps = pb.steps.filter((step) => !hidden(step));
}

/** Clicks templated from a button label get their recorded label back. */
function restoreClickLabels(pb, labels, secrets) {
  for (const step of pb.steps) {
    if (step.action !== 'click') continue;
    const name = /^\{\{(\w+)\}\}$/.exec(step.el?.text || '')?.[1];
    if (!labels.has(name) || secrets.has(name) || typeof pb.defaults?.[name] !== 'string') continue;
    step.el.text = pb.defaults[name];
  }
}

/** Drops defaults, labels and secrets no step or prompt uses any more. */
function pruneVariables(pb) {
  // Keep inputs used by other steps or the healing prompt, even if a click shared a name.
  const used = placeholders([pb.steps, pb.prompt]);
  if (pb.defaults) {
    for (const name of Object.keys(pb.defaults)) if (!used.has(name)) delete pb.defaults[name];
  }
  const clickInputs = placeholders(pb.steps.filter((step) => step.action === 'click'));
  if (pb.labels) pb.labels = pb.labels.filter((name) => clickInputs.has(name));
  if (pb.secrets) pb.secrets = pb.secrets.filter((name) => used.has(name));
}
