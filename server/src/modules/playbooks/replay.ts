/**
 * Replay without the LLM: each recorded step is matched to the live page by its
 * stable handles and sent to the browser as a command. Works on every provider.
 */
import * as workflow from '../../../../browser/src/workflow/index.ts';

import { sendCommand } from '../browsers/socket.ts';
import { DIALOG_BLOCKED } from '../../drivers/dialogs.ts';
import { fill, selectOptionIn, uploadFileIn, isFileValue } from '../agent/chat.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import {
  FIND_ATTEMPTS,
  FIND_RETRY_MS,
  REPLAY_PAUSE_MS,
  REPLAY_SETTLE_MS,
  NAVIGATE_TIMEOUT_MS,
  WORKFLOW_SCHEMA,
  WORKFLOW_TIMEOUT_MS,
} from './constants.ts';
import { matchElement } from './match.ts';
import { validateWorkflow } from './sanitize.ts';
import { heal } from './heal.ts';
import { answerField } from './answer.ts';

const { contradicts, volatileTarget } = workflow as any;

/** Replays one step against a browser. */
type Replayer = (browserId: string, step: any, values: any, defaults: any, task?: any) => Promise<any>;

/** Steps after which the page may have changed, so the checkpoint runs. */
const PAGE_CHANGING = new Set([
  'navigate',
  'click',
  'double_click',
  'click_coordinates',
  'press_key',
  'switch_tab',
  'go_back',
  'go_forward',
  'reload',
]);
/** Resolves after `ms`. */
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** A pause somewhere in the given range, never the same twice. */
const pauseWithin = ({ min, max }) => sleep(min + Math.random() * (max - min));
/** The variable a step's value is, when the value is exactly one placeholder. */
const soleVariable = (value) => (value || '').match(/^\{\{(\w+)\}\}$/)?.[1];
/** The first variable a label carries anywhere in it, or undefined. */
const echoedVariable = (value) => (value || '').match(/\{\{(\w+)(?:\|[^}]*)?\}\}/)?.[1];

/** Send one browser command and return its data, throwing on failure. */
async function command(browserId, action, params = {}, timeout?) {
  const r = await sendCommand(browserId, action, params, timeout);
  // An action that opened a confirm dialog did run; the recorded handle_dialog step answers it next.
  if (!r.ok && String(r.error || '').includes(DIALOG_BLOCKED)) return { dialog: r.error };
  if (!r.ok) throw new Error(r.error || `${action} failed`);
  return r.data;
}

/**
 * The analysis of the page this browser is on, kept between steps.
 *
 * Analyzing serializes the whole page, which on a heavy one costs seconds, and
 * a replay used to pay it per step, so removing the model from a ten-step Amazon
 * flow saved two seconds out of a hundred and forty. A form filled in twenty
 * fields is one page: it deserves one analysis, not twenty.
 *
 * A browser replays one playbook at a time, which is what makes a map by browser
 * id enough; the entry goes the moment anything could have changed the page.
 */
const analyses = new Map<string, any[]>();

/** The page's elements, analyzing only when there is nothing in hand. */
async function elementsOf(browserId) {
  if (analyses.has(browserId)) return analyses.get(browserId);
  const { elements } = await command(browserId, 'analyze');
  analyses.set(browserId, elements);
  return elements;
}

/** Forgets the analysis: the page may no longer be the one it described. */
function pageChanged(browserId) {
  analyses.delete(browserId);
}

/** The recorded element in the analysis in hand, or null. */
async function matchHere(browserId, el, text?, partial = false) {
  return matchElement(el, await elementsOf(browserId), text, partial);
}

/** Re-analyzes until the recorded element appears, for up to about five seconds. */
async function waitFor(browserId, el, text?, partial = false) {
  for (let attempt = 0; attempt < FIND_ATTEMPTS; attempt++) {
    const match = await matchHere(browserId, el, text, partial);
    if (match) return match;
    pageChanged(browserId);
    await sleep(FIND_RETRY_MS);
  }
  return null;
}

/**
 * The recorded element on the live page. The first look uses the analysis already
 * in hand, which on a page that has not changed is the whole cost of the step;
 * after that it re-analyzes on the same budget as before.
 */
async function find(browserId, el, text?, partial = false) {
  const found = await waitFor(browserId, el, text, partial);
  if (found) return found;
  throw missing(el);
}

/** The error a step fails with when its element is not on the page. */
function missing(el) {
  return new Error(`no element matching ${JSON.stringify(el?.text ?? el?.domId ?? el?.name ?? '')}`);
}

/**
 * A selector strong enough to aim the recorded element without analyzing at all:
 * a test id the page author wrote, or a link's own target. Deliberately not an
 * id (a framework may have made it up for that render) and not a `name` (pages
 * reuse `q` and `search` everywhere), a fast path that hits the wrong element
 * is worse than a slow one.
 */
function directSelector(el) {
  if (el?.testId) return `[data-testid=${JSON.stringify(el.testId)}]`;
  // As written, which is what the attribute selector matches, and only where what
  // was written will be written the same way again. Amazon's "page 2" link carries
  // the visit's own qid and xpid, so as a selector it finds nothing on the next
  // visit; comparing targets with that noise dropped is the analyzer pass's job.
  // `rawHref` only, never the resolved `href`: an attribute selector matches the attribute as
  // written, so a link whose markup says "news.ycombinator.com" is never found by its resolved
  // "https://news.ycombinator.com". That miss was invisible, the analyzer pass picks the
  // element up, but it cost an analysis per click and logged an error for a step that worked.
  const target = el?.rawHref;
  if (!target || String(el.tag || '').toLowerCase() !== 'a' || volatileTarget(target)) return null;
  return `a[href=${JSON.stringify(target)}]`;
}

/** Clicks the recorded element, or the option a data-driven click now names. */
async function replayClick(browserId, step, values, defaults) {
  const key = echoedVariable(step.el?.text);
  // A data-driven click aims by value, so it cannot take the fast path.
  const direct = key ? null : await clickDirect(browserId, withValues(step.el, values));
  if (direct) return direct;
  const aim = aimedAt(step, key, values, defaults);
  const el = await waitFor(browserId, ...aim);
  if (el) return command(browserId, 'click', { selector: `[data-ac-id="${el.id}"]` });
  return followLink(browserId, aim[0], !!key);
}

/**
 * A link replay could not find, followed to where it was recorded going. The run
 * clicked it where replay cannot: an agent reaches a link inside a closed menu
 * (Wikipedia's language list), or a results page lays its pager out differently,
 * yet the link's own target still says exactly where the click went. Following it
 * is that click without the menu. A data-driven click is not followed: its target
 * belongs to the recorded choice, and the value is what has to be found.
 */
async function followLink(browserId, el, dataDriven) {
  const url = dataDriven ? null : await linkDestination(browserId, el).catch(() => null);
  if (!url) throw missing(el);
  return command(browserId, 'navigate', { url }, NAVIGATE_TIMEOUT_MS);
}

/**
 * Where a recorded link goes, resolved against the page it is on: an http(s)
 * target only, never a same-page anchor, a `javascript:` handler or a target with
 * per-visit parameters, where following it is not what the click did.
 */
async function linkDestination(browserId, el) {
  const raw = el?.rawHref;
  if (String(el?.tag || '').toLowerCase() !== 'a' || !raw || raw.startsWith('#') || volatileTarget(raw)) return null;
  const { tabs = [] } = await command(browserId, 'list_tabs');
  const url = new URL(raw, tabs.find((t) => t.active)?.url);
  return /^https?:$/.test(url.protocol) ? url.href : null;
}

/**
 * What to look for: the recorded element, or the one a data-driven click now
 * names. Still the recorded label, match on the full precedence, testId first.
 * Changed, a data-driven option, an insurer, a plan, and the old DOM id
 * belonged to another choice, so the value is the only handle left.
 */
function aimedAt(step, key, values, defaults): [any, string | undefined, boolean] {
  const el = withValues(step.el, values);
  if (!key || el.text === fill(step.el.text, defaults)) return [el, undefined, false];
  // A label that is only the value is found by it whole; one that carries it among the
  // option's other words ("{{cpt}} - CT head") is found by the value, since the other
  // words belong to the recorded option and change with it.
  return soleVariable(step.el.text) ? [el, el.text, false] : [el, fill(`{{${key}}}`, values), true];
}

/**
 * The recorded handles with their placeholders filled in.
 *
 * A handle is recorded with the run's own values replaced by their variable names,
 * so a member id never sits in a saved playbook. That applies to every field, not
 * just the typed text: a Reddit post link recorded as
 * `{{start}}comments/1vz.../` is the same link once `start` is filled, and
 * nothing at all until it is.
 */
function withValues(el = {}, values) {
  const filled = Object.entries(el).map(([k, v]) => [k, typeof v === 'string' ? fill(v, values) : v]);
  return Object.fromEntries(filled);
}

/**
 * Whether what the browser clicked is what the recording meant. The click reports
 * the element's own handles, so a selector that resolved to something else, the
 * same test id reused on a different control, is caught here rather than three
 * steps later, when the replay has already typed a member id into the wrong form.
 *
 * The judgement is the shared one, not a second opinion. Asking separately went
 * wrong on Amazon, where the "Next" button and the "2" link carry the same href:
 * the fast path clicked Next, the texts differed, and a step that had in fact
 * reached page 2 was called a failure. Text is a handle that drifts, so it does
 * not decide identity here either.
 */
function clickedTheRight(el, clicked) {
  return !clicked || !contradicts(el, clicked);
}

/** Clicks a strong handle straight away, or null when there is none or it missed. */
async function clickDirect(browserId, el) {
  const selector = directSelector(el);
  if (!selector) return null;
  const result = await command(browserId, 'click', { selector }).catch(() => null);
  // Nothing resolved: fall back to analyzing and matching as before.
  if (!result) return null;
  if (clickedTheRight(el, result.handle)) return result;
  throw new Error(`${selector} no longer points at ${JSON.stringify(el.text || el.testId)}${whereItWent(result)}`);
}

/**
 * Where the click actually left the browser, for the message that says the handle
 * missed. "The page has changed" was true of eBay redirecting a pagination click to
 * its own sign-up wall, and useless: the page it changed to is the whole diagnosis.
 */
function whereItWent(result) {
  const title = String(result?.title || '').trim();
  return title ? `, the page is now ${JSON.stringify(title)}` : ', the page has changed';
}

/** Types the step's value into the recorded field. */
async function replayType(browserId, step, values, _defaults, task: any = {}) {
  const el = await find(browserId, withValues(step.el, values));
  const text = step.answer ? await answerFor(step.answer, values, task) : fill(step.text ?? '', values);
  return command(browserId, 'type', { selector: `[data-ac-id="${el.id}"]`, text });
}

/** A free-text field's words: the caller's own when passed, otherwise the model's, written for this run. */
async function answerFor(answer, values, task) {
  if (values[answer.key] != null) return String(values[answer.key]);
  return answerField(task.apiKey, answer.question, { task: task.prompt, values, example: answer.example });
}

/** Picks the step's option in the recorded select or dropdown. */
async function replaySelect(browserId, step, values) {
  const el = await find(browserId, withValues(step.el, values));
  const r = await selectOptionIn(browserId, el, fill(step.option ?? '', values));
  if (!r.ok) throw new Error(`${r.error}${r.options ? ` (options: ${r.options.join(' | ')})` : ''}`);
  return r;
}

/** Uploads the file() value passed for the step's variable. */
async function replayUpload(browserId, step, values) {
  // Only the variable name was recorded, so the replay brings its own file.
  const key = soleVariable(step.file);
  const f = key && values[key];
  if (!isFileValue(f)) throw new Error(`${key || 'this upload'} needs a file() value`);
  const el = step.el ? await find(browserId, withValues(step.el, values)) : null;
  const r = await uploadFileIn(browserId, el, f);
  if (!r.ok) throw new Error(r.error);
  return r;
}

/**
 * The tab whose url the step recorded, waited for rather than demanded: an SSO
 * handoff opens its tab when the other portal is ready, which is a second or two
 * after the click that started it.
 */
async function findTab(browserId, tabUrl) {
  for (let attempt = 0; attempt < FIND_ATTEMPTS; attempt++) {
    const { tabs } = await command(browserId, 'list_tabs');
    const match = (tabs || []).find((t) => sameTarget(t.url, tabUrl));
    if (match) return match;
    await sleep(FIND_RETRY_MS);
  }
  throw new Error(`no tab at ${tabUrl}, the handoff did not open the tab this run expects`);
}

/**
 * Whether two urls are the same destination. Query strings carry SSO tokens and
 * session ids that differ every run, so origin and path decide.
 */
function sameTarget(a, b) {
  try {
    const [x, y] = [new URL(a), new URL(b)];
    return x.origin === y.origin && x.pathname === y.pathname;
  } catch {
    return a === b;
  }
}

/** Switches to the tab the recording ended on, waiting for it to exist. */
async function replaySwitchTab(browserId, step) {
  if (!step.tabUrl) throw new Error('this switch_tab was recorded without a tab url and cannot be aimed');
  const tab = await findTab(browserId, step.tabUrl);
  return command(browserId, 'switch_tab', { tab_id: tab.id });
}

/**
 * Closes the tab the run closed and goes back to the one it landed on. A close is
 * recorded with the tab it left the run on (the recorder reads the active tab after
 * the close), so that is where the replay has to end up. Closing the tab at that
 * address instead closed the form the run was filling and carried on in the page it
 * had only opened to read.
 */
async function replayCloseTab(browserId, step) {
  const { tabs = [] } = await command(browserId, 'list_tabs');
  const active = tabs.find((t) => t.active);
  const landed = tabs.find((t) => t !== active && step.tabUrl && sameTarget(t.url, step.tabUrl));
  if (!active || !landed) return { skipped: 'that tab is already gone' };
  await command(browserId, 'close_tab', { tab_id: active.id });
  return command(browserId, 'switch_tab', { tab_id: landed.id });
}

/** Hovers over the recorded element, for a menu that opens on hover. */
async function replayHover(browserId, step, values) {
  const el = await find(browserId, withValues(step.el, values));
  return command(browserId, 'hover', { selector: `[data-ac-id="${el.id}"]` });
}

/** Double-clicks the recorded element, or the recorded point when no element was named. */
async function replayDoubleClick(browserId, step, values) {
  if (!step.el) return command(browserId, 'double_click', { x: step.x, y: step.y });
  const el = await find(browserId, withValues(step.el, values));
  return command(browserId, 'double_click', { element_id: el.id, selector: `[data-ac-id="${el.id}"]` });
}

/**
 * Clicks the point the run clicked. A point is not a handle: the same pixel is a
 * different control once a layout moves, so this is recorded and replayed to keep
 * the run honest, and the step stays visible in the playbook for a person to
 * replace with something aimable.
 */
function replayClickCoordinates(browserId, step) {
  if (step.x == null || step.y == null) throw new Error('a coordinate click was recorded without coordinates');
  return command(browserId, 'click_coordinates', { x: step.x, y: step.y });
}

/** Action → how it replays. */
const REPLAYERS: Record<string, Replayer> = {
  navigate: (browserId, step, values) =>
    command(browserId, 'navigate', { url: fill(step.url, values) }, NAVIGATE_TIMEOUT_MS),
  press_key: (browserId, step) => command(browserId, 'press_key', { key: step.key }),
  scroll: (browserId, step) => command(browserId, 'scroll', { direction: step.direction, amount: step.amount }),
  wait: (browserId, step) => command(browserId, 'wait', { selector: step.selector, timeout: step.timeout }),
  click: replayClick,
  type: replayType,
  select_option: replaySelect,
  upload_file: replayUpload,
  handle_dialog: (browserId, step) =>
    command(browserId, 'handle_dialog', { accept: step.accept !== false, prompt_text: step.prompt_text }),
  keyboard_type: (browserId, step, values) =>
    command(browserId, 'keyboard_type', { text: fill(step.text ?? '', values) }),
  open_tab: (browserId, step, values) =>
    command(browserId, 'open_tab', { url: fill(step.url ?? step.tabUrl ?? '', values) }, NAVIGATE_TIMEOUT_MS),
  switch_tab: replaySwitchTab,
  close_tab: replayCloseTab,
  double_click: replayDoubleClick,
  click_coordinates: replayClickCoordinates,
  hover: replayHover,
  go_back: (browserId) => command(browserId, 'back', {}, NAVIGATE_TIMEOUT_MS),
  go_forward: (browserId) => command(browserId, 'forward', {}, NAVIGATE_TIMEOUT_MS),
  reload: (browserId) => command(browserId, 'reload', {}, NAVIGATE_TIMEOUT_MS),
};

/** Whether replay knows how to run this action, for a playbook arriving from elsewhere. */
export const replayableAction = (action) => typeof action === 'string' && Object.hasOwn(REPLAYERS, action);

/** Replay one recorded step, filling its placeholders from `values`. */
async function runStep(browserId, step, values, defaults = {}, task = {}) {
  const known = typeof step.action === 'string' && Object.hasOwn(REPLAYERS, step.action);
  if (!known) throw new Error(`unknown step ${step.action}`);
  return REPLAYERS[step.action](browserId, step, values, defaults, task);
}

/**
 * Replay without the LLM. `checkpoint` runs after page-changing steps (CAPTCHA,
 * MFA). When a step no longer fits the page: autoHeal off throws; on, the agent
 * finishes the task and its steps replace the broken ones in the playbook.
 */
export async function play(apiKey, browserId, pb, vars = {}, { autoHeal = true, checkpoint, requestHuman }: any = {}) {
  if (pb.schemaVersion === WORKFLOW_SCHEMA) return playWorkflow(browserId, pb, vars, autoHeal);
  return replaySteps(apiKey, browserId, pb, { ...pb.defaults, ...vars }, { autoHeal, checkpoint, requestHuman });
}

/** Replays the steps in order, running the checkpoint after page-changing ones; a failure heals or throws. */
async function replaySteps(apiKey, browserId, pb, values, options) {
  pageChanged(browserId); // nothing in hand describes this page yet
  const run = { task: { apiKey, prompt: pb.prompt }, options, signedIn: false };
  for (let i = 0; i < pb.steps.length; i++) {
    const failed = await stepOrSignIn(browserId, pb, i, values, run);
    if (failed) return recover(apiKey, browserId, pb, i, failed.err, values, options);
    await afterStep(browserId, pb, i, run);
  }
  return { steps: pb.steps.length, total: pb.steps.length, fellBack: false };
}

/**
 * After a step that may have changed the page: drop the analysis, let the page be
 * read the way a person would before the next action, and run the checkpoint.
 */
async function afterStep(browserId, pb, i, run) {
  if (!PAGE_CHANGING.has(pb.steps[i].action)) return await pauseWithin(REPLAY_PAUSE_MS);
  pageChanged(browserId);
  await pauseWithin(REPLAY_SETTLE_MS);
  // Signed in: the next steps may be the login's own (a method to pick), so they run
  // where the sign-in left the page, and the flow's address is gone back to only if
  // the next step is not there.
  if (await run.options.checkpoint?.()) run.signedIn = true;
}

/**
 * Step `i`, and when it fails, one more try after the checkpoint: a session that ran
 * out mid-flow shows a login where the step's element was, and signing in again is
 * what a person would do before calling the flow broken.
 */
async function stepOrSignIn(browserId, pb, i, values, run) {
  const failed = await tryStep(browserId, pb, i, values, run.task);
  if (!failed) return null;
  const signedIn = run.signedIn || (await run.options.checkpoint?.());
  run.signedIn = false;
  if (!signedIn) return failed;
  await returnTo(browserId, pb, i - 1, values);
  return tryStep(browserId, pb, i, values, run.task);
}

/**
 * After a sign-in, back to the last address the playbook went to. A login lands on
 * the site's home page, not the deep link the flow was on, and the next step's
 * element is on that page, not this one.
 */
async function returnTo(browserId, pb, i, values) {
  const last = pb.steps.slice(0, i + 1).findLast((step) => step.action === 'navigate' && step.url);
  if (!last) return;
  await command(browserId, 'navigate', { url: fill(last.url, values) }, NAVIGATE_TIMEOUT_MS);
  pageChanged(browserId);
  await pauseWithin(REPLAY_SETTLE_MS);
}

/** Runs step `i`; the error it failed with, or null. */
async function tryStep(browserId, pb, i, values, task) {
  try {
    await runStep(browserId, pb.steps[i], values, pb.defaults || {}, task);
    return null;
  } catch (err) {
    // A popup the recording closed that this run never showed: nothing to close.
    return pb.steps[i].optional ? null : { err };
  }
}

/** After step `i` failed: throw without autoHeal, otherwise heal. */
function recover(apiKey, browserId, pb, i, err, values, { autoHeal, checkpoint, requestHuman }) {
  const total = pb.steps.length;
  if (!autoHeal)
    throw new HttpError(
      Status.UNPROCESSABLE,
      `Step ${i + 1} of ${total} (${pb.steps[i].action}) failed: ${err.message}`,
    );
  return heal(apiKey, browserId, pb, i, err, values, { checkpoint, requestHuman });
}

/** A version-2 workflow runs whole in the browser, which validates it with Playwright. */
async function playWorkflow(browserId, pb, vars, autoHeal) {
  const draft = validateWorkflow(pb);
  if (draft.steps.some((step) => step.enabled && step.action === 'checkpoint'))
    throw new HttpError(Status.CONFLICT, 'Open this workflow in Oya Browser for interactive human checkpoints.');
  const result = await command(browserId, 'workflow', { draft, variables: vars, autoHeal }, WORKFLOW_TIMEOUT_MS);
  if (result.status !== 'succeeded')
    throw new HttpError(Status.UNPROCESSABLE, result.error || `Playwright validation ${result.status}`);
  return workflowResult(draft, result);
}

/** What a workflow play answers. */
function workflowResult(draft, result) {
  return {
    steps: draft.steps.filter((step) => step.enabled).length,
    total: draft.steps.length,
    fellBack: false,
    assertions: result.assertions,
    runId: result.id,
  };
}
