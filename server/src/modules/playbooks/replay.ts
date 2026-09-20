/**
 * Replay without the LLM: each recorded step is matched to the live page by its
 * stable handles and sent to the browser as a command. Works on every provider.
 */
import { sendCommand } from '../browsers/socket.ts';
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

/** Replays one step against a browser. */
type Replayer = (browserId: string, step: any, values: any, defaults: any) => Promise<any>;

/** Steps after which the page may have changed, so the checkpoint runs. */
const PAGE_CHANGING = new Set(['navigate', 'click', 'double_click', 'click_coordinates', 'press_key', 'switch_tab']);
/** Resolves after `ms`. */
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** A pause somewhere in the given range, never the same twice. */
const pauseWithin = ({ min, max }) => sleep(min + Math.random() * (max - min));
/** The variable a step's value is, when the value is exactly one placeholder. */
const soleVariable = (value) => (value || '').match(/^\{\{(\w+)\}\}$/)?.[1];

/** Send one browser command and return its data, throwing on failure. */
async function command(browserId, action, params = {}, timeout?) {
  const r = await sendCommand(browserId, action, params, timeout);
  if (!r.ok) throw new Error(r.error || `${action} failed`);
  return r.data;
}

/**
 * The analysis of the page this browser is on, kept between steps.
 *
 * Analyzing serializes the whole page, which on a heavy one costs seconds — and
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
async function matchHere(browserId, el, text?) {
  return matchElement(el, await elementsOf(browserId), text);
}

/** Re-analyzes until the recorded element appears, for up to about five seconds. */
async function waitFor(browserId, el, text?) {
  for (let attempt = 0; attempt < FIND_ATTEMPTS; attempt++) {
    const match = await matchHere(browserId, el, text);
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
async function find(browserId, el, text?) {
  const found = await waitFor(browserId, el, text);
  if (found) return found;
  throw new Error(`no element matching ${JSON.stringify(el?.text ?? el?.domId ?? el?.name ?? '')}`);
}

/**
 * A selector strong enough to aim the recorded element without analyzing at all:
 * a test id the page author wrote, or a link's own target. Deliberately not an
 * id (a framework may have made it up for that render) and not a `name` (pages
 * reuse `q` and `search` everywhere) — a fast path that hits the wrong element
 * is worse than a slow one.
 */
function directSelector(el) {
  if (el?.testId) return `[data-testid=${JSON.stringify(el.testId)}]`;
  // As written, which is what the attribute selector matches.
  const target = el?.rawHref ?? el?.href;
  if (target && String(el.tag || '').toLowerCase() === 'a') return `a[href=${JSON.stringify(target)}]`;
  return null;
}

/** Clicks the recorded element, or the option a data-driven click now names. */
async function replayClick(browserId, step, values, defaults) {
  const key = soleVariable(step.el?.text);
  // A data-driven click aims by value, so it cannot take the fast path.
  const direct = key ? null : await clickDirect(browserId, withValues(step.el, values));
  if (direct) return direct;
  const el = await find(browserId, ...aimedAt(step, key, values, defaults));
  return command(browserId, 'click', { selector: `[data-ac-id="${el.id}"]` });
}

/**
 * What to look for: the recorded element, or the one a data-driven click now
 * names. Still the recorded label — match on the full precedence, testId first.
 * Changed — a data-driven option, an insurer, a plan — and the old DOM id
 * belonged to another choice, so the value is the only handle left.
 */
function aimedAt(step, key, values, defaults): [any, string | undefined] {
  const el = withValues(step.el, values);
  if (!key) return [el, undefined];
  const byValue = el.text !== defaults[key] ? el.text : undefined;
  return [el, byValue];
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
 * the element's own handles, so a selector that resolved to something else — the
 * same test id reused on a different control, a link whose target now sits
 * elsewhere — is caught here rather than three steps later, when the replay has
 * already typed a member id into the wrong form.
 */
function clickedTheRight(el, clicked) {
  if (!clicked) return true;
  if (el.testId && clicked.testId && el.testId !== clicked.testId) return false;
  if (el.tag && clicked.tag && el.tag.toLowerCase() !== clicked.tag.toLowerCase()) return false;
  // Text drifts legitimately (a count, a date), so it is compared only when both
  // sides have one and neither contains the other.
  const [was, now] = [String(el.text || '').trim(), String(clicked.text || '').trim()];
  if (!was || !now) return true;
  return was.includes(now) || now.includes(was);
}

/** Clicks a strong handle straight away, or null when there is none or it missed. */
async function clickDirect(browserId, el) {
  const selector = directSelector(el);
  if (!selector) return null;
  const result = await command(browserId, 'click', { selector }).catch(() => null);
  // Nothing resolved: fall back to analyzing and matching as before.
  if (!result) return null;
  if (clickedTheRight(el, result.handle)) return result;
  throw new Error(`${selector} no longer points at ${JSON.stringify(el.text || el.testId)} — the page has changed`);
}

/** Types the step's value into the recorded field. */
async function replayType(browserId, step, values) {
  const el = await find(browserId, withValues(step.el, values));
  return command(browserId, 'type', { selector: `[data-ac-id="${el.id}"]`, text: fill(step.text ?? '', values) });
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
  throw new Error(`no tab at ${tabUrl} — the handoff did not open the tab this run expects`);
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

/** Closes the tab the recording closed, found by where it was. */
async function replayCloseTab(browserId, step) {
  const tab = step.tabUrl ? await findTab(browserId, step.tabUrl).catch(() => null) : null;
  return tab ? command(browserId, 'close_tab', { tab_id: tab.id }) : { skipped: 'that tab is already gone' };
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
};

/** Replay one recorded step, filling its placeholders from `values`. */
async function runStep(browserId, step, values, defaults = {}) {
  const known = typeof step.action === 'string' && Object.hasOwn(REPLAYERS, step.action);
  if (!known) throw new Error(`unknown step ${step.action}`);
  return REPLAYERS[step.action](browserId, step, values, defaults);
}

/**
 * Replay without the LLM. `checkpoint` runs after page-changing steps (CAPTCHA,
 * MFA). When a step no longer fits the page: autoHeal off throws; on, the agent
 * finishes the task and its steps are saved as the draft `<name>:draft`.
 */
export async function play(apiKey, browserId, pb, vars = {}, { autoHeal = true, checkpoint, requestHuman }: any = {}) {
  if (pb.schemaVersion === WORKFLOW_SCHEMA) return playWorkflow(browserId, pb, vars, autoHeal);
  return replaySteps(apiKey, browserId, pb, { ...pb.defaults, ...vars }, { autoHeal, checkpoint, requestHuman });
}

/** Replays the steps in order, running the checkpoint after page-changing ones; a failure heals or throws. */
async function replaySteps(apiKey, browserId, pb, values, options) {
  const total = pb.steps.length;
  // Nothing in hand describes this page yet.
  pageChanged(browserId);
  for (let i = 0; i < total; i++) {
    const failed = await tryStep(browserId, pb, i, values);
    if (failed) return recover(apiKey, browserId, pb, i, failed.err, values, options);
    await afterStep(browserId, pb.steps[i].action, options);
  }
  return { steps: total, total, fellBack: false };
}

/**
 * After a step that may have changed the page: drop the analysis, let the page be
 * read the way a person would before the next action, and run the checkpoint.
 */
async function afterStep(browserId, action, options) {
  if (!PAGE_CHANGING.has(action)) return await pauseWithin(REPLAY_PAUSE_MS);
  pageChanged(browserId);
  await pauseWithin(REPLAY_SETTLE_MS);
  await options.checkpoint?.();
}

/** Runs step `i`; the error it failed with, or null. */
async function tryStep(browserId, pb, i, values) {
  try {
    await runStep(browserId, pb.steps[i], values, pb.defaults || {});
    return null;
  } catch (err) {
    return { err };
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
