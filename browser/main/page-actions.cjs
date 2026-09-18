/**
 * Commands that drive the active page: the server's command socket and the dev
 * panel's quick actions both land here. Tab management, recording and workflows
 * stay in main.js, which owns that state. Facade over main/actions/: the
 * PageDriver (driver.cjs) and its command maps (page-commands.cjs,
 * pointer-commands.cjs, dev-commands.cjs).
 */
const { PageDriver } = require('./actions/driver.cjs');

/** `ctx` is main.js's tab state (read through getters) and the helpers that act on it. */
function createPageActions(ctx) {
  const driver = new PageDriver(ctx);
  return {
    runPageAction: (id, action, params, view) => driver.runPageAction(id, action, params, view),
    runDevAction: (action, params) => driver.runDevAction(action, params),
    waitForTabReady: (tab) => driver.waitForTabReady(tab),
  };
}

module.exports = { createPageActions };
