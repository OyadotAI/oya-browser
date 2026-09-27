/**
 * A playbook's `{{name}}` placeholders: which it uses, which a run must supply,
 * and turning recorded values into placeholders with the value as their default.
 */
import workflow from '../../../../browser/scripts/workflow.cjs';

import {
  ECHO_WINDOW,
  FIRST_NAME_SUFFIX,
  MAX_VARIABLE_NAME,
  MIN_ANSWER_WORDS,
  MIN_CHOICE_LEN,
  MIN_PICK_LEN,
  MIN_PROMPT_VALUE_LEN,
  WORKFLOW_SCHEMA,
} from './constants.ts';

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
 * recorded value as its default, so a replay that passes nothing behaves exactly as
 * it was recorded, and every value is an input the caller can override. Values that
 * already carry a placeholder (an ask() run, a masked password) are left alone. A
 * navigate URL stays as recorded, except where it echoes one of those values.
 *
 * ponytail: names come from DOM handles, so a field the page never labelled lands on
 * what it is and where, `input6`. A rename step in the record dialog if that bites.
 */
export function templateValues(pb) {
  // name -> the value it was first given; a healed playbook's variables are already taken.
  const taken = new Map(Object.entries(pb.defaults || {}));
  pb.labels = []; // retained for compatibility with older playbooks
  // What the person asked, before any value in it becomes a placeholder: whether text
  // came from the prompt is judged on their words, not on the half-templated copy.
  const said = pb.prompt || '';
  for (const [i, step] of pb.steps.entries()) templateStep(pb, step, i, taken, said);
  for (const step of pb.steps) templatePick(pb, step, taken);
  for (const [name, value] of Object.entries(pb.defaults || {})) templateEchoes(pb.steps, name, String(value));
  return pb;
}

/** The field each action's value sits in, for the actions that carry data. */
const VALUE_KEYS = { type: 'text', keyboard_type: 'text', select_option: 'option' };

/** The ways a value is spelled inside an address: as typed, and encoded the two ways forms and scripts encode it. */
const urlForms = (value) => [
  ...new Set([encodeURIComponent(value).replace(/%20/g, '+'), encodeURIComponent(value), value]),
];

/**
 * Puts a variable where its recorded value echoes later in the flow. A search the
 * agent ran by address (`/s?k=red+shoes`), or the typeahead option it picked right
 * after typing ("70450 - CT head" after typing 70450), replays the recorded data
 * forever unless these follow the variable too.
 */
function templateEchoes(steps, name, value) {
  if (value.length < MIN_PROMPT_VALUE_LEN) return;
  const typedAt = steps.flatMap((s, i) => (valueOf(s) === `{{${name}}}` ? [i] : []));
  steps.forEach((step, j) => {
    if (step.action === 'navigate' && step.url) step.url = inUrl(step.url, name, value);
    if (step.action === 'click' && typedAt.some((i) => j > i && j - i <= ECHO_WINDOW)) echoInLabel(step, name, value);
  });
}

/** A step's typed or picked value, if it carries one. */
const valueOf = (step) => (Object.hasOwn(VALUE_KEYS, step.action) ? step[VALUE_KEYS[step.action]] : undefined);

/**
 * The value, where a click's label holds it as a whole word, becomes the variable.
 * Only a click just after the value was typed: that is a pick from what the typing
 * brought up. A "Yes" answered on one question says nothing about a "Yes, continue"
 * button three pages on.
 */
function echoInLabel(step, name, value) {
  const text = step.el?.text;
  if (!text || HAS_PLACEHOLDER.test(text)) return;
  const at = new RegExp(`(?<!\\w)${escaped(value.trim())}(?!\\w)`, 'i');
  if (at.test(text)) step.el = { ...step.el, text: text.replace(at, `{{${name}}}`) };
}

/** A regex source for literal text. */
const escaped = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * The address with the value's spellings turned into the URL-encoded variable, only
 * where one fills a whole path segment or query value: "new" typed into a box must not
 * turn `/news/` into a variable.
 */
function inUrl(url, name, value) {
  // The host is where the flow goes, never its data.
  const cut = url.indexOf('/', url.indexOf('//') + '//'.length);
  if (cut < 0) return url;
  const at = (form) => new RegExp(`(?<=[/=])${escaped(form)}(?=[/?&#]|$)`, 'g');
  const path = urlForms(value).reduce((p, form) => p.replace(at(form), `{{${name}|url}}`), url.slice(cut));
  return url.slice(0, cut) + path;
}

/**
 * A click on a choice the person named in their prompt becomes a variable too: "service
 * category Diagnostic Imaging" typed in the chat and clicked as a button is data, and a
 * replay with another category has to click another button. Only a label the prompt
 * holds word for word, of several words or with a digit in it, so "Submit" in "submit
 * it" stays a step; and never a link, which is where the flow goes, not what it carries.
 */
function templatePick(pb, step, taken) {
  const text = step.action === 'click' ? step.el?.text?.trim() : '';
  if (!pickable(step.el, text)) return;
  const name = pickName(pb, step, text, taken);
  if (!name) return;
  step.el = { ...step.el, text: `{{${name}}}` };
  pb.defaults[name] = text;
  // A one-word answer ("Yes") is not rewritten in the prompt: it would land on whichever question says it first.
  if (text.length >= MIN_PICK_LEN) pb.prompt = pb.prompt?.replace(wholeWord(text), `{{${name}}}`);
}

/**
 * The variable a picked choice fills, or null when it is not one. A radio or checkbox is
 * named by its group ("size", "conservative"), so two questions both answered "Yes"
 * stay two variables; any other choice clicked again reuses the variable it had,
 * since the prompt no longer holds its words by then.
 */
function pickName(pb, step, text, taken) {
  const group = isChoice(step.el) && handleName(step.el.name, 'choice');
  const known = !group && Object.keys(pb.defaults).find((k) => pb.defaults[k] === text);
  if (known) return known;
  if (!(pb.prompt && holds(pb.prompt, text))) return null;
  return uniqueName(group || promptName(pb.prompt, text) || nameFor(step, 0), text, taken);
}

/** A radio button or checkbox: a pick among fixed options, data by nature. */
const isChoice = (el: any = {}) => el.type === 'radio' || el.type === 'checkbox';

/**
 * Whether a clicked label could be data: a choice, not a link. A radio or checkbox may be
 * one word ("Medium", "No"); any other control needs several words or a digit, so a
 * button called "Submit" never becomes a variable because the prompt says "submit".
 */
const pickable = (el: any = {}, text) => {
  if (!text || el.tag === 'a' || el.href || HAS_PLACEHOLDER.test(text)) return false;
  if (isChoice(el)) return text.length >= MIN_CHOICE_LEN;
  return text.length >= MIN_PICK_LEN && /\s|\d/.test(text);
};

/** A regex for `text` as a whole phrase, any case. */
const wholeWord = (text) => new RegExp(`(?<!\\w)${escaped(text)}(?!\\w)`, 'i');

/** Whether `prompt` holds `text` as a whole phrase. */
const holds = (prompt, text) => wholeWord(text).test(prompt);

/** A name from the one or two words the prompt puts before the value: "service category Diagnostic Imaging". */
function promptName(prompt, value) {
  const before = prompt.match(new RegExp(`(?<![A-Za-z])([A-Za-z]+(?: [A-Za-z]+)?):? +${escaped(value)}`, 'i'))?.[1];
  return before ? handleName(before, 'choice') : null;
}

/**
 * Whether the agent wrote this text itself rather than copying it from the task: a
 * free-text answer (a comment, a reason, the answer to a question) the prompt never
 * spelled out. Replayed as recorded, the first case's words land in every later one.
 */
function composed(said, step, value) {
  if (step.action !== 'type' || !said) return false;
  const inPrompt = said.toLowerCase().includes(value.trim().toLowerCase());
  const freeText = step.el?.tag === 'textarea' || value.trim().split(/\s+/).length >= MIN_ANSWER_WORDS;
  return !inPrompt && freeText;
}

/**
 * The step asks the model at replay time instead of typing recorded words. It keeps a
 * variable name, so a caller who wants fixed text still passes it, and the recorded
 * text as an example of length and tone, never as a default.
 */
function asAnswer(step, name, value) {
  step.answer = { key: name, question: questionOf(step.el || {}), example: value };
  delete step.text;
}

/** What a field asks, as its label says; an unlabelled one by its id or name read as words ("permanent Address"). */
function questionOf(el) {
  const named = el.name || el.domId;
  const spoken =
    named &&
    String(named)
      .replace(/[_-]+/g, ' ')
      .replace(/([a-z])([A-Z])/g, '$1 $2');
  return el.text || el.ariaLabel || el.placeholder || spoken || 'the text field';
}

/** Turns one step's typed or picked value into a placeholder. */
function templateStep(pb, step, i, taken, said = '') {
  const key = Object.hasOwn(VALUE_KEYS, step.action) ? VALUE_KEYS[step.action] : null;
  // Click targets describe the flow, not input data. Preserve their stable handles.
  const value = key ? step[key] : null;
  if (!value || HAS_PLACEHOLDER.test(value)) return;
  const name = uniqueName(nameFor(step, i), value, taken);
  if (composed(said, step, value)) return asAnswer(step, name, value);
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
