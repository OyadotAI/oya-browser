/** Server dialog regressions exercise Oya's actual command runner, native dialog queue and page driver. */
const fs = require('node:fs');
const path = require('node:path');
const { app } = require('electron');
const { World } = require('../../src/main/native/index.ts');
const { NativeDialogs } = require('../../src/main/dialogs/index.ts');
const { CommandRunner } = require('../../src/main/connection/commands.ts');
const { PageDriver } = require('../../src/main/actions/driver.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
/** Fixture-only composition; command behavior and dialog arbitration are production implementations. */
module.exports = function fixtureActions(window) {
  const view = { webContents: window.webContents };
  const world = new World({
    analyzerScript: fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8'),
    worldName: 'server-native-fixture',
  });
  const dialogs = new NativeDialogs();
  dialogs.watch(view.webContents);
  window.show();
  app.focus({ steal: true });
  window.focus();
  view.webContents.focus();
  const pending = new Map();
  let sequence = 0,
    runner;
  const actions = new PageDriver({
    keyboard: new Keyboard(process.platform),
    mouse: new Mouse(),
    getActiveView: () => view,
    tabs: () => [{ id: 1, view, protection: 'protected' }],
    injectScripts: (v) => world.ensure(v),
    worldEval: (v, source) => world.evaluate(v, source),
    sendResult: (...args) => runner.sendResult(...args),
    pullCookiesFor: async () => {},
    analysisStarted() {},
    analysisFinished() {},
  });
  runner = new CommandRunner({
    actions,
    dialogs,
    tabs: { getActiveView: () => view },
    shell: { browsingMode: true, devLog() {} },
    socket: {
      isOpen: () => true,
      send: (result) => {
        const settle = pending.get(result.id);
        pending.delete(result.id);
        settle?.(result);
      },
    },
  });
  return (action, params) =>
    new Promise((resolve, reject) => {
      const id = ++sequence;
      pending.set(id, resolve);
      const translated = params.element_id === undefined ? params : { ...params, selector: String(params.element_id) };
      runner.handleCommand({ id, action, params: translated }).catch((error) => {
        pending.delete(id);
        reject(error);
      });
    });
};
