/**
 * What a recording checks as well as does. When a person's action takes a tab
 * to another page, the recording gets a step that checks a replay got there
 * too. Without it a replay that typed a wrong password "passed" on the login
 * page, because every click still found its button.
 */
const { normalizeStep } = require('../../scripts/workflow.cjs');
const { VOLATILE_PARAMS } = require('../../scripts/workflow/handles.cjs');
const { WEB_URL } = require('../tabs/constants.cjs');
const { PAGE_CHECK_SETTLE_MS } = require('./constants.cjs');

/** Actions a person takes on a page that can send it somewhere else. Typing is left out: search boxes rewrite the address as you type. */
const PAGE_INPUT = new Set(['click', 'double_click', 'press_key', 'select_option', 'upload_file']);

/** Steps after which the page a move lands on is checked: a person's actions, and Back and Forward. */
const CHECKED_AFTER = new Set([...PAGE_INPUT, 'go_back', 'go_forward']);

/** Steps that already say which page the tab is on. */
const ADDRESSED = new Set(['navigate', 'assert_page', 'assert_url']);

/**
 * A parameter value that names one record (a job id, a row) or signs one visit
 * (Amazon's `ds=v1:…`): an opaque run of 24 or more characters with no spaces.
 * A replay's results need not share it.
 */
const ID_LIKE = /^\d{5,}$|^[a-f0-9-]{16,}$|^[\w+/=:.-]{24,}$/i;

/**
 * The part of an address a page check compares, the same rule as the
 * generated `assert_page`: origin and path without Amazon's `/ref=` segment,
 * and a `#/route` fragment.
 */
function pageOf(url) {
  try {
    const u = new URL(url);
    return u.origin + u.pathname.replace(/\/ref=[^/]*/g, '') + (u.hash.startsWith('#/') ? u.hash : '');
  } catch {
    return String(url);
  }
}

/**
 * The query parameters a move changed that a replay must change the same way:
 * a filter, a sort, a search. Per-visit tokens and record ids are left out.
 */
function changedParams(before, after) {
  try {
    const [a, b] = [new URL(before).searchParams, new URL(after).searchParams];
    const names = new Set([...a.keys(), ...b.keys()]);
    const counts = (k) => !VOLATILE_PARAMS.test(k) && !ID_LIKE.test(b.get(k) || a.get(k) || '');
    return [...names].filter((k) => a.get(k) !== b.get(k) && counts(k));
  } catch {
    return [];
  }
}

/** The index of the tab's last step recorded no later than `t`, or -1. */
function lastIndexBefore(steps, tab, t) {
  for (let i = steps.length - 1; i >= 0; i--) if (steps[i].tab === tab && (steps[i].t ?? 0) <= t) return i;
  return -1;
}

/** The page a tab's steps last said it was on, or undefined. */
function lastAddressed(steps, tab) {
  const step = steps.findLast((s) => s.tab === tab && s.enabled !== false && ADDRESSED.has(s.action));
  return step && (step.url || step.expected);
}

/** A page check for `url` in `tab`, recorded at `t`, naming the query parameters it holds to. */
const pageCheck = (url, tab, t, params = []) =>
  normalizeStep({
    action: 'assert_page',
    expected: url,
    tab,
    t,
    ...(params.length ? { params: params.join(',') } : {}),
  });

/**
 * Adds the check after the step that caused the move. A second move with
 * nothing in between (a redirect the page made itself) updates the check to
 * where the page settled, because a replay passes through the first one too
 * fast to catch it.
 */
function checkPage(recorder, tab, move) {
  if (!recorder.recording) return;
  const steps = recorder.recordedSteps;
  const i = lastIndexBefore(steps, tab, move.t);
  const last = steps[i];
  if (last?.action === 'assert_page' && last.t) settle(last, move);
  else if (CHECKED_AFTER.has(last?.action)) steps.splice(i + 1, 0, pageCheck(move.url, tab, move.t, move.params));
  else return;
  recorder.emitRecording();
}

/** A check a redirect moved on from: where the page settled, holding to every parameter either move set. */
function settle(check, move) {
  const params = new Set([...String(check.params || '').split(','), ...move.params].filter(Boolean));
  Object.assign(check, { expected: move.url, params: [...params].join(',') || undefined });
  if (!check.params) delete check.params;
}

/** A move from `previous` to `url` worth checking: another page, or a filter or sort on this one; else null. */
function moveOf(previous, url) {
  if (!previous) return null;
  // Parameters are held to only on the same page (a filter, a sort): a new page's query is often per-visit context.
  const samePage = pageOf(previous) === pageOf(url);
  const params = samePage ? changedParams(previous, url) : [];
  if (samePage && !params.length) return null;
  return { url, params, t: Date.now() };
}

/**
 * A tab's main frame moved to `url`. The check waits a moment, because the
 * step that caused the move can arrive from the page just after the move does.
 */
function pageReached(recorder, tabId, url) {
  if (!recorder.recording || !WEB_URL.test(url || '')) return;
  const move = moveOf(recorder.pageUrls.get(tabId), url);
  recorder.pageUrls.set(tabId, url);
  if (!move) return;
  const tab = recorder.names.recordingTab(tabId);
  setTimeout(() => checkPage(recorder, tab, move), PAGE_CHECK_SETTLE_MS);
}

/** Where every tab is as recording starts, so the first move in each is seen as one. */
function rememberPages(recorder) {
  recorder.pageUrls = new Map();
  for (const tab of recorder.ctx.tabs.list) {
    try {
      recorder.pageUrls.set(tab.id, tab.view.webContents.getURL());
    } catch {}
  }
}

/** The active tab's address and recorded name. */
function activePage(recorder) {
  const tabId = recorder.ctx.tabs.activeTabId;
  const view = recorder.ctx.tabs.list.find((t) => t.id === tabId)?.view;
  return { url: view?.webContents?.getURL?.(), tab: recorder.names.recordingTab(tabId) };
}

/**
 * On stop, a check that the replay ends on the page the recording ended on,
 * unless the steps already say it is there. A tab whose steps never said where
 * it was gets none: its replay has no known start to end up away from.
 */
function finalPageCheck(recorder) {
  const { url, tab } = activePage(recorder);
  const steps = recorder.recordedSteps;
  if (!WEB_URL.test(url || '') || !steps.some((s) => s.tab === tab && PAGE_INPUT.has(s.action))) return;
  const known = lastAddressed(steps, tab);
  if (!known || pageOf(known) === pageOf(url)) return;
  steps.push(pageCheck(url, tab, Date.now()));
}

module.exports = { pageReached, rememberPages, finalPageCheck, pageOf };
