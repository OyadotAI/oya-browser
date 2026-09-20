/**
 * The last run per browser as replayable steps, for playbook.js. Element ids die
 * with each analysis, so steps keep the analyzer's stable metadata instead.
 * ponytail: in memory, oldest evicted past 1000 browsers; a run is lost on restart unless saved as a playbook.
 */
import { sendCommand } from '../browsers/socket.ts';
import { redact, dataKey } from './placeholders.ts';
import { MAX_RECORDED_RUNS } from './constants.ts';

/**
 * Tools whose calls become playbook steps.
 *
 * A tool left out of here is not a tool that replays badly — it is a hole in the
 * recording. An SSO handoff that switches tabs, a click the page only accepts at
 * coordinates, a value typed into whatever has focus: leaving those out produced
 * a playbook that looked complete and replayed the second portal's steps into the
 * first portal's tab. Everything the agent can do to a page is recorded; a step
 * that cannot be aimed again says so at replay instead of going missing here.
 */
const RECORDED = new Set([
  'navigate',
  'click',
  'double_click',
  'click_coordinates',
  'type',
  'keyboard_type',
  'select_option',
  'upload_file',
  'press_key',
  'scroll',
  'wait',
  'handle_dialog',
  'open_tab',
  'switch_tab',
  'close_tab',
]);
/** Tools that aim at an element, so their step carries that element's stable handles. */
const ELEMENT_ACTIONS = ['click', 'double_click', 'type', 'select_option', 'upload_file'];
/** Tools whose step needs the tab it ended up on, since a tab id dies with the session. */
const TAB_ACTIONS = new Set(['open_tab', 'switch_tab', 'close_tab']);
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
  'x',
  'y',
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

/**
 * The tab a tab step ended on, as a url rather than an id: ids are handed out per
 * session, so a recorded `tab_id` names a different tab — or nothing — on the next
 * run. A replay finds the tab by where it went.
 */
async function tabHandle(browserId) {
  const tabs = await sendCommand(browserId, 'list_tabs').catch(() => null);
  return tabs?.data?.tabs?.find((t) => t.active)?.url;
}

/** What the step aims at: the element's stable handles, and the file a replay must bring. */
function aim(step, run, name, args, values) {
  if (ELEMENT_ACTIONS.includes(name) && args.element_id != null)
    step.el = redactedElement(run, args.element_id, values);
  // The bytes never enter a playbook; the variable name does, so a replay brings its own file.
  if (name === 'upload_file' && args.name) step.file = `{{${dataKey(args.name)}}}`;
  for (const k of STEP_ARGS) if (args[k] !== undefined) step[k] = args[k];
}

/** Append a replayable tool call to the browser's current run, with typed values redacted to placeholders. */
export async function recordStep(browserId, name, args, values) {
  const run = runs.get(browserId);
  if (!run || !RECORDED.has(name)) return;
  const step: any = { action: name };
  aim(step, run, name, args, values);
  if (TAB_ACTIONS.has(name)) step.tabUrl = await tabHandle(browserId);
  run.steps.push(step);
}
