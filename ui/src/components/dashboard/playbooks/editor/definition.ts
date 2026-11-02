/**
 * The playbook editor's pure helpers: the definition as text and back, the
 * variables its steps use, and changes to a variable's default or secret flag.
 * The definition stays plain data, so what is edited is what the server checks.
 */
import type { Definition } from './editor-api';

/** How the definition is indented in the editor, so it reads and diffs well. */
export const INDENT = 2;

/** A {{name}} or {{name|filter}} placeholder. */
const PLACEHOLDER = /\{\{\s*([A-Za-z_][\w.-]*)\s*(?:\|[^}]*)?\}\}/g;

/** The definition as editor text. */
export const toText = (definition: Definition) => JSON.stringify(definition, null, INDENT);

/** What parsing the text gives: the definition, or why it is not one. */
export interface Parsed {
  /** The definition, when the text is one. */
  definition: Definition | null;
  /** Why it is not, or null. */
  error: string | null;
}

/** Whether a parsed value is a definition: an object with a steps list. */
const isDefinition = (value: unknown): value is Definition =>
  Boolean(value) && typeof value === 'object' && !Array.isArray(value) && Array.isArray((value as Definition).steps);

/** Parses the editor text; anything but a JSON object with a steps list is an error. */
export function parse(text: string): Parsed {
  const value = readJson(text);
  if (value instanceof Error) return { definition: null, error: `Not valid JSON: ${value.message}` };
  return isDefinition(value)
    ? { definition: value, error: null }
    : { definition: null, error: 'The definition needs a steps list' };
}

/** The text as JSON, or the error that says why it is not. */
function readJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch (err) {
    return err as Error;
  }
}

/** The variable names the steps use, in the order they first appear. */
export function variablesUsed(definition: Definition): string[] {
  const names = [...JSON.stringify(definition.steps ?? []).matchAll(PLACEHOLDER)].map((m) => m[1]);
  return [...new Set(names)];
}

/** One variable as the Variables pane shows it. */
export interface VariableRow {
  /** Its name. */
  name: string;
  /** Its default, '' for none. */
  value: string;
  /** Whether it holds a secret. */
  secret: boolean;
}

/** A workflow (schema 2): replay reads its variables, not the classic defaults and secrets. */
const WORKFLOW = 2;

/** Whether the definition is a workflow. */
const isWorkflow = (d: Definition) => d.schemaVersion === WORKFLOW;

/** The variable's default and secret flag, from wherever this kind of playbook keeps them. */
function variableOf(d: Definition, name: string): VariableRow {
  if (isWorkflow(d))
    return { name, value: d.variables?.[name]?.default ?? '', secret: Boolean(d.variables?.[name]?.secret) };
  return { name, value: d.defaults?.[name] ?? '', secret: (d.secrets ?? []).includes(name) };
}

/** Every variable the playbook has, with its default and secret flag: a workflow's declared ones too. */
export function variableRows(d: Definition): VariableRow[] {
  const names = new Set([...variablesUsed(d), ...(isWorkflow(d) ? Object.keys(d.variables ?? {}) : [])]);
  return [...names].map((name) => variableOf(d, name));
}

/** A workflow with one variable changed. */
const withVariable = (d: Definition, name: string, change: Record<string, unknown>): Definition => ({
  ...d,
  variables: { ...(d.variables ?? {}), [name]: { ...(d.variables?.[name] ?? {}), ...change } },
});

/** The definition with one variable's default set ('' removes it). */
export function withDefault(d: Definition, name: string, value: string): Definition {
  if (isWorkflow(d)) return withVariable(d, name, { default: value || undefined });
  const defaults = { ...(d.defaults ?? {}) };
  if (value) defaults[name] = value;
  else delete defaults[name];
  return { ...d, defaults };
}

/** The definition with one variable marked secret or not. */
export function withSecret(d: Definition, name: string, secret: boolean): Definition {
  if (isWorkflow(d)) return withVariable(d, name, { secret });
  const rest = (d.secrets ?? []).filter((n) => n !== name);
  return { ...d, secrets: secret ? [...rest, name] : rest };
}
