/** Remove legacy recording noise only when the stored data proves its origin.
 * DOM IDs and offscreen/visible flags are not evidence that an input was hidden.
 * Returns a copy; callers retain the original sealed record for recovery.
 */
export function cleanPlaybook(original) {
  if (!Array.isArray(original?.steps)) return original;
  const pb = structuredClone(original);
  const labels = new Set(pb.labels || []);
  const secrets = new Set(pb.secrets || []);
  const hidden = step => step.el?.tag === 'input'
    && (step.el.type === 'hidden' || step.el.inputType === 'hidden');
  if (Number.isInteger(pb.healedFrom)) {
    pb.healedFrom -= pb.steps.slice(0, pb.healedFrom).filter(hidden).length;
  }
  pb.steps = pb.steps.filter(step => !hidden(step));
  for (const step of pb.steps) {
    if (step.action !== 'click') continue;
    const name = /^\{\{(\w+)\}\}$/.exec(step.el?.text || '')?.[1];
    if (!labels.has(name) || secrets.has(name) || typeof pb.defaults?.[name] !== 'string') continue;
    step.el.text = pb.defaults[name];
  }
  // Keep inputs used by other steps or the healing prompt, even if a click shared a name.
  const used = new Set([...JSON.stringify([pb.steps, pb.prompt]).matchAll(/\{\{(\w+)(?:\|[^}]*)?\}\}/g)].map(match => match[1]));
  if (pb.defaults) {
    for (const name of Object.keys(pb.defaults)) if (!used.has(name)) delete pb.defaults[name];
  }
  const clickInputs = new Set([...JSON.stringify(pb.steps.filter(step => step.action === 'click'))
    .matchAll(/\{\{(\w+)(?:\|[^}]*)?\}\}/g)].map(match => match[1]));
  if (pb.labels) pb.labels = pb.labels.filter(name => clickInputs.has(name));
  if (pb.secrets) pb.secrets = pb.secrets.filter(name => used.has(name));
  return pb;
}
