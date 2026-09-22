/**
 * The page tools beyond clicking and typing: hover, history, reading the page
 * with a script, waiting for it to settle, and finding an element by words.
 * run_script, wait_for and find only read the page, so none is recorded.
 */
import { sendCommand } from '../browsers/socket.ts';
import { rememberHandle, setElements } from './recorder.ts';
import { elementList } from './element-index.ts';
import { noteAnalysis } from './changes.ts';
import { byId, withControls } from './controls.ts';
import {
  FIND_LIMIT,
  NAVIGATE_TIMEOUT_MS,
  NETWORK_QUIET_MS,
  RUN_SCRIPT_OUTPUT_CHARS,
  WAIT_FOR_DEFAULT_MS,
  WAIT_FOR_MAX_MS,
  WAIT_FOR_POLL_MS,
  WAIT_FOR_TRIES,
} from './constants.ts';

/**
 * What a read-only script may not do: act on the page, send a request, or leave.
 * ponytail: a pattern check keeps an honest model to reading; it is not a sandbox
 * (the isolated world is what keeps the page from seeing the script). A proxy-wrapped
 * DOM is the upgrade if scripts ever come from somewhere untrusted.
 */
const MUTATES =
  /\.(click|submit|requestSubmit|focus|blur|remove|append|prepend|appendChild|removeChild|replaceChildren|insertAdjacent\w*|setAttribute|removeAttribute|dispatchEvent|setItem|removeItem|clear|pushState|replaceState|back|forward|go|assign|reload|play|pause)\s*\(|\b(fetch|XMLHttpRequest|sendBeacon|WebSocket|EventSource|open|postMessage|eval|Function)\s*\(|new\s+(XMLHttpRequest|WebSocket|EventSource|Function)\b|\.(value|checked|selected|innerHTML|outerHTML|textContent|innerText|href|src|cookie|hash|search|pathname|disabled|scrollTop|scrollLeft|location)\s*[+-]?=[^=]|\blocation\s*=[^=]/;

/** Why a script was refused. */
const READ_ONLY =
  'Error: run_script only reads the page. Act with the element tools (click, type, select_option), which are recorded and replay.';

/** A result the model can read in one piece. */
const capped = (text) =>
  text.length > RUN_SCRIPT_OUTPUT_CHARS ? `${text.slice(0, RUN_SCRIPT_OUTPUT_CHARS)}\n… (cut)` : text;

/** Runs a read-only script in the analyzer's isolated world and returns what it returned, as JSON. */
async function runScript(browserId, args) {
  const script = String(args.script || '');
  if (MUTATES.test(script)) return READ_ONLY;
  const r = await sendCommand(browserId, 'run_script', { script });
  if (!r.ok) return `Error: ${r.error}`;
  if (r.data?.error) return `Error: the script threw: ${r.data.error}`;
  return capped(JSON.stringify(r.data?.value ?? null, null, 1));
}

/** What wait_for watches for, bounded, with the tempo it polls at. */
function waitPlan(args) {
  const timeout = Math.min(Number(args.timeout) || WAIT_FOR_DEFAULT_MS, WAIT_FOR_MAX_MS);
  const idle = args.network_idle === true || (!args.text && !args.url);
  return { text: args.text || '', url: args.url || '', idle, timeout, poll: WAIT_FOR_POLL_MS, quiet: NETWORK_QUIET_MS };
}

/** The in-page loop behind wait_for, reading its plan from `w`: the text shown, the url reached, and no request started for a while. */
const WAIT_LOOP = `
const met = () => (!w.text || (document.body?.innerText || '').includes(w.text)) && (!w.url || location.href.includes(w.url));
const seen = () => performance.getEntriesByType('resource').length;
const end = Date.now() + w.timeout;
let last = seen(), since = Date.now();
while (Date.now() < end) {
  if (seen() !== last) { last = seen(); since = Date.now(); }
  const settled = document.readyState === 'complete' && (!w.idle || Date.now() - since >= w.quiet);
  if (met() && settled) return { met: true, url: location.href };
  await new Promise((r) => setTimeout(r, w.poll));
}
return { met: false, url: location.href };`;

/** The wait_for script for one plan. */
const waitScript = (plan) => `const w = ${JSON.stringify(plan)};${WAIT_LOOP}`;

/** One watch of the page; a navigation mid-wait reads as not met yet, so the next try watches the new page. */
async function watchOnce(browserId, plan) {
  const r = await sendCommand(
    browserId,
    'run_script',
    { script: waitScript(plan) },
    plan.timeout + NAVIGATE_TIMEOUT_MS,
  );
  return r.ok && r.data?.value ? r.data.value : { met: false, error: r.error || r.data?.error };
}

/** What waiting found, in words. */
const waitedSaid = (plan, seen) =>
  seen.met
    ? `The page is ready (${[plan.text && `shows "${plan.text}"`, plan.url && `at ${seen.url}`, plan.idle && 'network quiet'].filter(Boolean).join(', ')}).`
    : `Waited ${plan.timeout} ms and the page is not there yet${seen.url ? ` (at ${seen.url})` : ''}. Analyze it to see what it shows.`;

/** Waits for text, a url or a quiet network, starting again only when the page navigated underneath it. */
async function waitFor(browserId, args) {
  const plan = waitPlan(args);
  let seen = await watchOnce(browserId, plan);
  for (let i = 1; i < WAIT_FOR_TRIES && seen.error; i++) seen = await watchOnce(browserId, plan);
  return seen.error ? `Error: ${seen.error}` : waitedSaid(plan, seen);
}

/** Moves the mouse onto an element, for menus and tooltips that open on hover. */
async function hover(browserId, args) {
  const r = await sendCommand(browserId, 'hover', { selector: byId(args.element_id) });
  if (!r.ok) return `Error: ${r.error}`;
  rememberHandle(browserId, args.element_id, r.data?.handle);
  return withControls(browserId, `Hovered over element ${args.element_id}`, true);
}

/** A history move (back, forward, reload) that waits for the page it lands on. */
const historyMove = (action, said) => async (browserId) => {
  const r = await sendCommand(browserId, action, {}, NAVIGATE_TIMEOUT_MS);
  return r.ok ? withControls(browserId, said, true) : `Error: ${r.error}`;
};

/** The words an element is known by, lowercased, for matching a query. */
const wordsOf = (e) =>
  [e.text, e.label, e.ariaLabel, e.placeholder, e.name, e.value, e.type, e.href]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();

/** How well an element matches the query's words: one point a word, and more for the whole phrase. */
function score(e, terms, phrase) {
  const words = wordsOf(e);
  const hits = terms.filter((t) => words.includes(t)).length;
  return hits + (words.includes(phrase) ? terms.length : 0);
}

/** The elements that best match a query, best first. */
function ranked(elements, query) {
  const phrase = String(query || '')
    .toLowerCase()
    .trim();
  const terms = phrase.split(/\s+/).filter(Boolean);
  const scored = elements.map((e) => ({ e, s: score(e, terms, phrase) })).filter((x) => x.s > 0);
  return scored.sort((a, b) => b.s - a.s).map((x) => x.e);
}

/** The page's elements that match a description, with ids click and type accept, without reading the whole page. */
async function find(browserId, args) {
  const r = await sendCommand(browserId, 'analyze', {});
  if (!r.ok) return `Error: ${r.error}`;
  setElements(browserId, r.data.elements);
  noteAnalysis(browserId, r.data);
  const hits = ranked(r.data.elements || [], args.query);
  if (!hits.length) return `Nothing on the page matches "${args.query}". Analyze the page, or scroll to load more.`;
  return elementList({ ...r.data, elements: hits }, FIND_LIMIT);
}

/** The handler for each tool this file adds to BROWSER_TOOLS. */
export const PAGE_TOOL_HANDLERS = {
  run_script: runScript,
  wait_for: waitFor,
  hover,
  go_back: historyMove('back', 'Went back'),
  go_forward: historyMove('forward', 'Went forward'),
  reload: historyMove('reload', 'Reloaded the page'),
  find,
};
