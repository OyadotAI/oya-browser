/**
 * The last run per browser as replayable steps, for playbook.js. Element ids die
 * with each analysis, so steps keep the analyzer's stable metadata instead.
 * ponytail: in memory, oldest evicted past 1000 browsers; a run is lost on restart unless saved as a playbook.
 */
import workflow from '../../../../browser/scripts/workflow.cjs';

import { sendCommand } from '../browsers/socket.ts';
import { redact, dataKey } from './placeholders.ts';
import { MAX_RECORDED_RUNS } from './constants.ts';

const { handlesOf } = workflow as any;

/**
 * Tools whose calls become playbook steps.
 *
 * A tool left out of here is not a tool that replays badly, it is a hole in the
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
  'hover',
  'go_back',
  'go_forward',
  'reload',
]);
/** Tools that aim at an element, so their step carries that element's stable handles. */
const ELEMENT_ACTIONS = ['click', 'double_click', 'type', 'select_option', 'upload_file', 'hover'];
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
const STABLE_KEYS = [
  'type',
  'tag',
  'text',
  'domId',
  'name',
  'ariaLabel',
  'testId',
  'placeholder',
  'href',
  'rawHref',
  'role',
  // Where it sits, and the names the analyzer worked out: the only handles left for
  // an element with no text and no target, a citation link, an icon button.
  'path',
  'stableText',
  'scoped',
  'repeats',
];

/** browserId -> { prompt, steps, elements } */
const runs = new Map();

/** Only the fields the browser actually reported. */
const defined = (handle = {}) => Object.fromEntries(Object.entries(handle || {}).filter(([, v]) => v !== undefined));

/**
 * The handles the browser reads off the element as it clicks it, whether or not the
 * element has them. What it leaves out, the analyzer's own workings, is all the
 * analysis still contributes once the browser has spoken.
 */
const REPORTED_KEYS = [
  'tag',
  'text',
  'domId',
  'name',
  'ariaLabel',
  'testId',
  'placeholder',
  'rawHref',
  'href',
  'role',
  'path',
];

/** The analysis without the fields the browser reports for itself. */
const analyzerOnly = (e = {}) => Object.fromEntries(Object.entries(e).filter(([k]) => !REPORTED_KEYS.includes(k)));

/** Form fields, whose own text is always empty: what names one is its label. */
const FIELD_TAGS = new Set(['input', 'select', 'textarea']);

/**
 * The analyzer's label for a field the browser read no text off. The browser reads an
 * element's own text, and a field has none, so its empty answer means "cannot see the
 * label", not "has none". Taken at its word it erased "More than 12 weeks" from a
 * radio, leaving only the name its whole group shares, and every replay then answered
 * the question with the group's first choice.
 */
function fieldLabel(analyzed, reported) {
  // A submit or button input is named by its value, not a label; the analyzer's text for
  // one is whatever surrounds it ("Has this procedure been performed? No Yes").
  const unread = FIELD_TAGS.has(reported.tag) && !reported.text && analyzed?.type !== 'button';
  return unread && analyzed?.text ? { text: analyzed.text } : {};
}

/** An element's stable handles, without the id that dies with the analysis. */
const stable = (e: any = {}) => Object.fromEntries(STABLE_KEYS.filter((k) => e[k] !== undefined).map((k) => [k, e[k]]));

/** The last recorded ask() run on a browser (prompt, steps, latest elements), or null. */
export function lastRun(browserId) {
  return runs.get(browserId) || null;
}

/**
 * Whether a run did something a playbook can replay. Navigating alone is not
 * enough: a run that only visited and read pages replays as page loads that
 * produce nothing, since reading needs the model. Refused calls never become
 * steps, so this is what actually happened, not what was attempted.
 */
export const hasReplayableSteps = (run) => !!run?.steps.some((s) => s.action !== 'navigate');

/** An element from the browser's latest analysis, by the id the model gave. */
export function elementOf(browserId, elementId) {
  return runs.get(browserId)?.elements.find((e) => e.id === Number(elementId));
}

/**
 * The handles the browser read off the element as it acted on it.
 *
 * The analysis list is a snapshot: by the time a click is recorded, the id the
 * model used may name a different element or none at all, and a step recorded
 * without handles replays as a click on the page body. What the browser saw
 * when it actually clicked cannot be stale, so it wins.
 */
export function rememberHandle(browserId, elementId, handle) {
  const run = runs.get(browserId);
  if (run && handle) run.handles.set(Number(elementId), handle);
}

/**
 * Forgets what the browser read off an element id, before a new action on it.
 *
 * Ids are renumbered on every analysis, so a handle kept from an earlier action
 * describes whatever element held the id then. A select or an upload reports no
 * handle of its own, and inherited that one: a select on a wizard's second panel
 * was recorded as the header link clicked earlier under the same id.
 */
export function forgetHandle(browserId, elementId) {
  runs.get(browserId)?.handles?.delete(Number(elementId));
}

/** The elements the model was last given for this browser, or none. */
export const elementsOf = (browserId): any[] => runs.get(browserId)?.elements || [];

/** Keeps the latest analysis, so steps can name elements by their stable handles. */
export function setElements(browserId, elements) {
  const run = runs.get(browserId);
  if (run) run.elements = elements;
}

/**
 * Drops what the current message recorded: the agent is starting it over. Steps an
 * earlier message of the same chat recorded stay, since that part was done.
 */
export async function restartRun(browserId) {
  const run = runs.get(browserId);
  if (!run) return;
  run.steps = run.steps.slice(0, run.mark ?? 0);
  // The new attempt starts where the agent is: it may have gone back to the form
  // before saying it was starting over, and that navigation was just dropped.
  const here = await tabHandle(browserId);
  const last = run.steps.at(-1);
  const there = last?.action === 'navigate' && last.url === here;
  if (/^https?:/.test(here || '') && !there) run.steps.push({ action: 'navigate', url: here, start: true });
}

/** Marks where the current message's steps begin, for restartRun. */
export function markMessage(browserId) {
  const run = runs.get(browserId);
  if (run) run.mark = run.steps.length;
}

/** Makes `run` the browser's current run, starting from the page it is on so a playbook replays from the same place. */
export async function startRun(browserId, run) {
  run.handles = new Map();
  const tabs = await sendCommand(browserId, 'list_tabs').catch(() => null);
  const startUrl = tabs?.data?.tabs?.find((t) => t.active)?.url;
  if (/^https?:/.test(startUrl || '')) run.steps.push({ action: 'navigate', url: startUrl, start: true });
  run.mark = run.steps.length;
  runs.delete(browserId);
  runs.set(browserId, run);
  if (runs.size > MAX_RECORDED_RUNS) runs.delete(runs.keys().next().value);
}

/** The element a step acts on, with visible data and secrets alike redacted: a playbook stores placeholders, never values. */
function redactedElement(run, elementId, values, acted?) {
  // The browser's own reading of the element it acted on cannot be stale, so where
  // it speaks it decides, including about the handles the element does not have.
  // Letting a missing field fall through to the analysis inherited an href from a
  // different element: a click on a Start button was recorded as a click on the
  // footer link that shared its id in a stale analysis, and replayed as one. The
  // analysis still contributes what the browser never reports, the type, and the
  // names it worked out for a repeated or drifting label.
  // `acted` is the element as the model saw it before acting: the action re-analyzes the
  // page, after which the same id may name a different element.
  const analyzed = acted ?? run.elements.find((e) => e.id === Number(elementId));
  const reported = run.handles?.get(Number(elementId));
  const el = stable(
    reported ? { ...analyzerOnly(analyzed), ...defined(reported), ...fieldLabel(analyzed, reported) } : analyzed,
  );
  for (const k of Object.keys(el)) el[k] = redact(el[k], values);
  return el;
}

/**
 * The tab a tab step ended on, as a url rather than an id: ids are handed out per
 * session, so a recorded `tab_id` names a different tab, or nothing, on the next
 * run. A replay finds the tab by where it went.
 */
async function tabHandle(browserId) {
  const tabs = await sendCommand(browserId, 'list_tabs').catch(() => null);
  return tabs?.data?.tabs?.find((t) => t.active)?.url;
}

/**
 * Whether any handle on this element could find it again, asked of the one table
 * that knows. "Any field with something in it" was not the same question: an element
 * carrying only a tag and an empty label passed it, and the step then failed on
 * replay with nothing to name ("no element matching \"\"").
 */
const aimable = (el) => handlesOf(el || {}).length > 0;

/** What the step aims at: the element's stable handles, and the file a replay must bring. */
function aim(step, run, name, args, values, acted?) {
  if (ELEMENT_ACTIONS.includes(name) && args.element_id != null) {
    const el = redactedElement(run, args.element_id, values, acted);
    // An element nothing can find again is not a handle. Say so on the step, so a
    // replay refuses it instead of clicking the page body and carrying on.
    if (aimable(el)) step.el = el;
    else step.unaimable = true;
  }
  // The bytes never enter a playbook; the variable name does, so a replay brings its own file.
  if (name === 'upload_file' && args.name) step.file = `{{${dataKey(args.name)}}}`;
  for (const k of STEP_ARGS) if (args[k] !== undefined) step[k] = args[k];
}

/**
 * Drops the start step when the run's first act is to navigate somewhere itself.
 *
 * The start step exists to put a replay back on the page the run began on. A run
 * that immediately navigates read nothing from that page, and keeping the step
 * sends every replay through wherever the browser happened to be left, the
 * previous run's page, challenge wall and all, before going where it meant to.
 */
function dropRedundantStart(run, name) {
  if (name === 'navigate' && run.steps.at(-1)?.start) run.steps.pop();
}

/**
 * Whether this call closes the tab the run's last step opened, with nothing done in it.
 * An agent that opens a page only to read it (a help page, a file the portal serves)
 * and closes it again changed nothing a replay has to repeat; recorded, the round
 * trip made every replay open and close pages for no reason.
 */
function closesUnusedTab(run, name) {
  return name === 'close_tab' && run.steps.at(-1)?.action === 'open_tab';
}

/** Append a replayable tool call to the browser's current run, with typed values redacted to placeholders. */
export async function recordStep(browserId, name, args, values, acted?) {
  const run = runs.get(browserId);
  if (!run || !RECORDED.has(name)) return;
  if (closesUnusedTab(run, name)) return void run.steps.pop();
  dropRedundantStart(run, name);
  const step: any = { action: name };
  aim(step, run, name, args, values, acted);
  if (TAB_ACTIONS.has(name)) step.tabUrl = await tabHandle(browserId);
  run.steps.push(step);
}
