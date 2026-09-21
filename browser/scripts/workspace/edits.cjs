/**
 * Editor commands: each one changes a copy of the draft. The workspace
 * normalizes the result, bumps its revision and records it for undo.
 */
const { randomUUID } = require('node:crypto');
const { normalizeStep } = require('../workflow.cjs');
const { DRAFT } = require('../constants.cjs');
const { RESERVED } = require('../workflow/rules.cjs');

/** Refuses a new variable name that is not an identifier, is reserved, or is taken. */
function checkNewName(draft, nextName) {
  if (!/^[A-Za-z_]\w{0,63}$/.test(nextName) || RESERVED.includes(nextName) || draft.variables[nextName]) {
    throw new Error('Choose a unique variable name using letters, digits, and underscores');
  }
}

/** Renames a variable everywhere: its settings, the secret list and every `{{placeholder}}`. */
function renameVariable(draft, command) {
  const { name, nextName } = command;
  checkNewName(draft, nextName);
  draft.variables[nextName] = draft.variables[name];
  delete draft.variables[name];
  draft.secrets = draft.secrets.map((secret) => (secret === name ? nextName : secret));
  const text = JSON.stringify(draft.steps);
  draft.steps = JSON.parse(text.split('{{' + name + '}}').join('{{' + nextName + '}}'));
}

/** Replaces every variable; the secret list follows their settings. */
function setVariables(draft, command) {
  draft.variables = command.variables;
  draft.secrets = Object.keys(command.variables).filter((k) => command.variables[k].secret);
}

/** Moves a step by `delta` places, clamped to the list. */
function moveStep(draft, command, index) {
  if (index < 0) return;
  const [step] = draft.steps.splice(index, 1);
  draft.steps.splice(Math.max(0, Math.min(draft.steps.length, index + Number(command.delta))), 0, step);
}

/** Replaces a step with its patched version; the step must still exist. */
function updateStep(draft, command, index) {
  if (index < 0) throw new Error('Step no longer exists');
  draft.steps[index] = normalizeStep({ ...draft.steps[index], ...command.patch, id: command.id });
}

/** The name and description, each capped. */
function setMetadata(draft, command) {
  Object.assign(draft, {
    name: String(command.name ?? draft.name).slice(0, DRAFT.MAX_NAME),
    description: String(command.description ?? draft.description).slice(0, DRAFT.MAX_DESCRIPTION),
  });
}

/** Editor command type → change to the draft copy (`index` is the command's step, or -1). */
const EDITS = {
  metadata: setMetadata,
  'rename-variable': renameVariable,
  variables: setVariables,
  add: (draft, command, index) =>
    draft.steps.splice(index < 0 ? draft.steps.length : index + 1, 0, normalizeStep(command.step)),
  update: updateStep,
  delete: (draft, command, index) => index >= 0 && draft.steps.splice(index, 1),
  duplicate: (draft, command, index) =>
    index >= 0 && draft.steps.splice(index + 1, 0, { ...structuredClone(draft.steps[index]), id: randomUUID() }),
  move: moveStep,
};

/** Applies an editor command to `draft` in place; an unknown command is refused. */
function applyEdit(draft, command) {
  if (!Object.hasOwn(EDITS, command.type)) throw new Error('Unknown editor command');
  const index = draft.steps.findIndex((s) => s.id === command.id);
  EDITS[command.type](draft, command, index);
}

module.exports = { applyEdit };
