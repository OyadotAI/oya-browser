/**
 * A playbook's `{{name}}` placeholders: which it uses, which a run must supply,
 * and turning recorded values into placeholders with the value as their default.
 */
import workflow from '../../../../browser/scripts/workflow.cjs';

import { FIRST_NAME_SUFFIX, MAX_VARIABLE_NAME, MIN_PROMPT_VALUE_LEN, WORKFLOW_SCHEMA } from './constants.ts';

/** A `{{name}}` or `{{name|filter}}` placeholder anywhere in a string. */
export const HAS_PLACEHOLDER = /\{\{\w+(?:\|[^}]*)?\}\}/;
/** A usable JavaScript identifier, as a variable name. */
const IDENT = /^[A-Za-z_]\w{0,39}$/;
/** Handles a click's value is named from, best first. */
const CLICK_HANDLES = ['text', 'ariaLabel', 'testId', 'domId', 'name'];
/** Handles a field's value is named from, best first. */
const FIELD_HANDLES = ['name', 'domId', 'ariaLabel', 'placeholder', 'text', 'testId'];

/** Every placeholder the steps use, in order of appearance. */
export const variablesOf = (steps) => [
  ...new Set([...JSON.stringify(steps).matchAll(/\{\{(\w+)(?:\|[^}]*)?\}\}/g)].map((m) => m[1])),
];

/** A playbook's variable names, whichever schema it was saved in. */
export const namesOf = (pb) =>
  pb.schemaVersion === WORKFLOW_SCHEMA ? workflow.variableNames(workflow.normalizeDraft(pb)) : variablesOf(pb.steps);

/** Placeholders a run must supply: those with no value passed, no default, or that are secrets. */
export const missingVariables = (pb, vars = {}) =>
  namesOf(pb).filter(
    (k) =>
      vars[k] == null &&
      pb.defaults?.[k] == null &&
      (!pb.variables?.[k] || pb.variables[k].secret || pb.variables[k].default == null),
  );

/** A string as a camelCase name, capped in length. */
const CAMEL = (s) =>
  String(s)
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+(.)?/g, (_, c) => (c ? c.toUpperCase() : ''))
    .slice(0, MAX_VARIABLE_NAME);

/**
 * The variable name a step's value gets, from whatever the element says about itself.
 * Never a bare `field`: a form of anonymous boxes is one nobody can fill, so a value
 * with no handle to read is named after what it is and the step it was at.
 */
function nameFor(step, i) {
  const el = step.el || {};
  const kind = CAMEL(el.type || el.tag || '') || 'field';
  const from = (step.action === 'click' ? CLICK_HANDLES : FIELD_HANDLES).map((k) => el[k]);
  for (const raw of from) {
    const name = handleName(raw, kind);
    if (name) return name;
  }
  return `${kind}${i + 1}`;
}

/** A valid name from one handle, prefixed with the element's kind if it needs it; null if none. */
function handleName(raw, kind) {
  const name = raw && CAMEL(raw);
  if (!name) return null;
  if (IDENT.test(name)) return name;
  // "2024" or "1st line" is a fine name once it is told what it names.
  const prefixed = `${kind}${name[0].toUpperCase()}${name.slice(1)}`.slice(0, MAX_VARIABLE_NAME);
  return IDENT.test(prefixed) ? prefixed : null;
}

/**
 * Every value a person typed or picked becomes a `{{name}}`, with the
 * recorded value as its default — so a replay that passes nothing behaves exactly as
 * it was recorded, and every value is an input the caller can override. Values that
 * already carry a placeholder (an ask() run, a masked password) are left alone, and
 * so are navigate URLs: the addresses are the flow, not its data.
 *
 * ponytail: names come from DOM handles, so a field the page never labelled lands on
 * what it is and where — `input6`. A rename step in the record dialog if that bites.
 */
export function templateValues(pb) {
  const taken = new Map(); // name -> the value it was first given
  pb.labels = []; // retained for compatibility with older playbooks
  for (const [i, step] of pb.steps.entries()) templateStep(pb, step, i, taken);
  return pb;
}

/** Turns one step's typed or picked value into a placeholder. */
function templateStep(pb, step, i, taken) {
  const key = step.action === 'type' ? 'text' : step.action === 'select_option' ? 'option' : null;
  // Click targets describe the flow, not input data. Preserve their stable handles.
  const value = key ? step[key] : null;
  if (!value || HAS_PLACEHOLDER.test(value)) return;
  const name = uniqueName(nameFor(step, i), value, taken);
  step[key] = `{{${name}}}`;
  pb.defaults[name] = value;
  // A healing agent reads the prompt, so the data in it has to travel too. Only
  // what was typed or picked: substituting a button label mangles the description.
  if (key && pb.prompt && value.length >= MIN_PROMPT_VALUE_LEN) pb.prompt = pb.prompt.split(value).join(`{{${name}}}`);
}

/** `base`, or `base2`, `base3`… when another value already holds it. */
function uniqueName(base, value, taken) {
  let name = base;
  // One field typed into twice is one input. Two fields that derive the same name are two,
  // even when the person happened to type the same thing into both.
  for (let n = FIRST_NAME_SUFFIX; taken.has(name) && taken.get(name) !== value; n++) name = `${base}${n}`;
  taken.set(name, value);
  return name;
}
