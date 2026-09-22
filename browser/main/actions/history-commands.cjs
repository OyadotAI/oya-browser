/**
 * Back, forward and reload in the tab a command targets, answered once the page
 * they lead to has loaded (or the wait runs out), with where the tab ended up.
 */
const c = require('./constants.cjs');

/** The events that say a history move has landed: a load finishing, or a move within the page. */
const LANDED = ['did-stop-loading', 'did-navigate-in-page'];

/** Calls `fn` once, when the tab lands (LANDED) or LOAD_TIMEOUT_MS passes, then stops listening. */
function onceLanded(contents, fn) {
  let timer;
  const done = () => {
    clearTimeout(timer);
    for (const event of LANDED) contents.off(event, done);
    fn();
  };
  timer = setTimeout(done, c.LOAD_TIMEOUT_MS);
  for (const event of LANDED) contents.on(event, done);
}

/** Resolves once the tab finishes loading or moves within the page, or after LOAD_TIMEOUT_MS. */
const loaded = (contents) => new Promise((resolve) => onceLanded(contents, resolve));

/** Runs `move` on the tab, waits for the page it leads to, and answers with that page. */
async function moveTab(driver, id, view, move) {
  const contents = view.webContents;
  const arrived = loaded(contents);
  move(contents);
  await arrived;
  await driver.ctx.injectScripts(view);
  driver.ctx.sendResult(id, true, { url: contents.getURL(), title: contents.getTitle() });
}

/** A step through the tab's history, refused when there is nowhere to go. */
const historyStep = (canGo, go, nowhere) => (driver, id, _params, view) => {
  const history = view.webContents.navigationHistory;
  if (!history[canGo]()) return driver.ctx.sendResult(id, false, null, nowhere);
  return moveTab(driver, id, view, () => history[go]());
};

/** The handler for each history command. */
const HISTORY_COMMANDS = {
  back: historyStep('canGoBack', 'goBack', 'There is no page to go back to.'),
  forward: historyStep('canGoForward', 'goForward', 'There is no page to go forward to.'),
  reload: (driver, id, _params, view) => moveTab(driver, id, view, (contents) => contents.reload()),
};

module.exports = { HISTORY_COMMANDS };
