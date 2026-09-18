/**
 * The last run per browser as replayable steps, for playbook.js. Element ids die
 * with each analysis, so steps keep the analyzer's stable metadata instead.
 * ponytail: in memory, oldest evicted past 1000 browsers; a run is lost on restart unless saved as a playbook.
 */
import { sendCommand } from '../browsers/socket.ts';
import { redact, dataKey } from './placeholders.ts';
import { MAX_RECORDED_RUNS } from './constants.ts';

/** Tools whose calls become playbook steps. */
const RECORDED = new Set([
  'navigate',
  'click',
  'type',
  'select_option',
  'upload_file',
  'press_key',
  'scroll',
  'wait',
  'handle_dialog',
]);
/** Tools that aim at an element, so their step carries that element's stable handles. */
const ELEMENT_ACTIONS = ['click', 'type', 'select_option', 'upload_file'];
/** Tool arguments copied into a step as given. */
const STEP_ARGS = [
  'url',
  'text',
  'option',
  'key',
  'direction',
  'amount',
  'selector',
  'timeout',
  'accept',
  'prompt_text',
];
/** Analyzer fields that survive a re-analysis, which a replay finds the element by. */
const STABLE_KEYS = ['type', 'tag', 'text', 'domId', 'name', 'ariaLabel', 'testId', 'placeholder', 'href'];

/** browserId -> { prompt, steps, elements } */
const runs = new Map();

/** An element's stable handles, without the id that dies with the analysis. */
const stable = (e: any = {}) => Object.fromEntries(STABLE_KEYS.map((k) => [k, e[k]]));

/** The last recorded ask() run on a browser (prompt, steps, latest elements), or null. */
export function lastRun(browserId) {
  return runs.get(browserId) || null;
}

/** An element from the browser's latest analysis, by the id the model gave. */
export function elementOf(browserId, elementId) {
  return runs.get(browserId)?.elements.find((e) => e.id === Number(elementId));
}

/** Keeps the latest analysis, so steps can name elements by their stable handles. */
export function setElements(browserId, elements) {
  const run = runs.get(browserId);
  if (run) run.elements = elements;
}

/** Makes `run` the browser's current run, starting from the page it is on so a playbook replays from the same place. */
export async function startRun(browserId, run) {
  const tabs = await sendCommand(browserId, 'list_tabs').catch(() => null);
  const startUrl = tabs?.data?.tabs?.find((t) => t.active)?.url;
  if (/^https?:/.test(startUrl || '')) run.steps.push({ action: 'navigate', url: startUrl, start: true });
  runs.delete(browserId);
  runs.set(browserId, run);
  if (runs.size > MAX_RECORDED_RUNS) runs.delete(runs.keys().next().value);
}

/** The element a step acts on, with visible data and secrets alike redacted: a playbook stores placeholders, never values. */
function redactedElement(run, elementId, values) {
  const el = stable(run.elements.find((e) => e.id === Number(elementId)));
  for (const k of Object.keys(el)) el[k] = redact(el[k], values);
  return el;
}

/** Append a replayable tool call to the browser's current run, with typed values redacted to placeholders. */
export function recordStep(browserId, name, args, values) {
  const run = runs.get(browserId);
  if (!run || !RECORDED.has(name)) return;
  const step: any = { action: name };
  if (ELEMENT_ACTIONS.includes(name) && args.element_id != null)
    step.el = redactedElement(run, args.element_id, values);
  // The bytes never enter a playbook; the variable name does, so a replay brings its own file.
  if (name === 'upload_file' && args.name) step.file = `{{${dataKey(args.name)}}}`;
  for (const k of STEP_ARGS) if (args[k] !== undefined) step[k] = args[k];
  run.steps.push(step);
}
