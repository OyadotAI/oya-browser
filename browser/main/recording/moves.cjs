/**
 * Steps for where a person sends a tab themselves: the address bar, a new tab,
 * Back and Forward. The page's typing is collected before each: leaving by the
 * browser's own controls never fires the page's goodbye in time, and the text
 * typed just before was lost.
 */
const { WEB_URL } = require('../tabs/constants.cjs');

/**
 * The start step exists so a replay begins where the person began. When the
 * first thing they do is go somewhere else, nothing happened on that page, and
 * keeping it sends every replay on a detour through it first.
 */
function dropUnusedStart(steps) {
  if (steps.length === 1 && steps[0].start) steps.pop();
}

/** Collects every page's typing; a page that cannot answer still lets the move be recorded. */
const collectTyping = (recorder) => recorder.drainAll(true).catch(() => {});

/**
 * Only a URL the person asked for, the address bar, a new tab. Where a click or a
 * form submission lands is already the click's step, and a goto over it replays past
 * whatever that click set up (and pins a one-off session URL into the playbook).
 */
async function recordNavigation(recorder, url) {
  if (!recorder.recording || !WEB_URL.test(url || '')) return;
  await collectTyping(recorder);
  const last = recorder.recordedSteps.at(-1);
  const sameTab = () => last.tab === recorder.names.recordingTab(recorder.ctx.tabs.activeTabId);
  if (last && last.action === 'navigate' && last.url === url && sameTab()) return;
  dropUnusedStart(recorder.recordedSteps);
  recorder.pushRecordedStep({ action: 'navigate', url });
}

/** Back or forward in the active tab's history. */
async function recordHistory(recorder, action) {
  if (!recorder.recording) return;
  await collectTyping(recorder);
  recorder.pushRecordedStep({ action });
}

module.exports = { recordNavigation, recordHistory };
