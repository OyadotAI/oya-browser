/**
 * Editor commands: each one changes a copy of the draft. The workspace
 * normalizes the result, bumps its revision and records it for undo.
 */
import { randomUUID } from 'node:crypto';
import { normalizeStep, DRAFT, RESERVED, type Draft, type Variable } from '../../workflow/index.ts';

/** One editor command from the shell page; which fields it carries depends on its type. */
export interface EditCommand {
  /** The command: 'add', 'update', 'undo', 'open', ... */
  type: string;
  /** The step (or, for open and open-run, the draft or run) it acts on. */
  id?: string;
  /** A variable's current name, for rename-variable. */
  name?: string;
  /** A variable's new name, for rename-variable. */
  nextName?: string;
  /** Every variable, for variables. */
  variables?: Record<string, Variable>;
  /** How many places a step moves. */
  delta?: number | string;
  /** The fields an update changes. */
  patch?: Record<string, unknown>;
  /** The step an add inserts. */
  step?: unknown;
  /** The new description, for metadata. */
  description?: string;
  /** A file's workflow, for import. */
  draft?: Partial<Draft>;
}

/** One change to the draft copy; `index` is the command's step, or -1. */
type Edit = (draft: Draft, command: EditCommand, index: number) => unknown;

/** Refuses a new variable name that is not an identifier, is reserved, or is taken. */
function checkNewName(draft: Draft, nextName: string): void {
  if (!/^[A-Za-z_]\w{0,63}$/.test(nextName) || RESERVED.includes(nextName) || draft.variables[nextName]) {
    throw new Error('Choose a unique variable name using letters, digits, and underscores');
  }
}

/** Renames a variable everywhere: its settings, the secret list and every `{{placeholder}}`. */
function renameVariable(draft: Draft, command: EditCommand): void {
  const { name = '', nextName = '' } = command;
  checkNewName(draft, nextName);
  draft.variables[nextName] = draft.variables[name];
  delete draft.variables[name];
  draft.secrets = draft.secrets.map((secret) => (secret === name ? nextName : secret));
  const text = JSON.stringify(draft.steps);
  draft.steps = JSON.parse(text.split('{{' + name + '}}').join('{{' + nextName + '}}'));
}

/** Replaces every variable; the secret list follows their settings. */
function setVariables(draft: Draft, command: EditCommand): void {
  const variables = command.variables ?? {};
  draft.variables = variables;
  draft.secrets = Object.keys(variables).filter((k) => variables[k].secret);
}

/** Moves a step by `delta` places, clamped to the list. */
function moveStep(draft: Draft, command: EditCommand, index: number): void {
  if (index < 0) return;
  const [step] = draft.steps.splice(index, 1);
  draft.steps.splice(Math.max(0, Math.min(draft.steps.length, index + Number(command.delta))), 0, step);
}

/** Replaces a step with its patched version; the step must still exist. */
function updateStep(draft: Draft, command: EditCommand, index: number): void {
  if (index < 0) throw new Error('Step no longer exists');
  draft.steps[index] = normalizeStep({ ...draft.steps[index], ...command.patch, id: command.id });
}

/** The name and description, each capped. */
function setMetadata(draft: Draft, command: EditCommand): void {
  Object.assign(draft, {
    name: String(command.name ?? draft.name).slice(0, DRAFT.MAX_NAME),
    description: String(command.description ?? draft.description).slice(0, DRAFT.MAX_DESCRIPTION),
  });
}

/** Editor command type → change to the draft copy (`index` is the command's step, or -1). */
const EDITS: Record<string, Edit> = {
  metadata: setMetadata,
  'rename-variable': renameVariable,
  variables: setVariables,
  add: (draft, command, index) =>
    draft.steps.splice(index < 0 ? draft.steps.length : index + 1, 0, normalizeStep(command.step)),
  update: updateStep,
  delete: (draft, _command, index) => index >= 0 && draft.steps.splice(index, 1),
  duplicate: (draft, _command, index) =>
    index >= 0 && draft.steps.splice(index + 1, 0, { ...structuredClone(draft.steps[index]), id: randomUUID() }),
  move: moveStep,
};

/** Applies an editor command to `draft` in place; an unknown command is refused. */
export function applyEdit(draft: Draft, command: EditCommand): void {
  if (!Object.hasOwn(EDITS, command.type)) throw new Error('Unknown editor command');
  const index = draft.steps.findIndex((s) => s.id === command.id);
  EDITS[command.type](draft, command, index);
}
