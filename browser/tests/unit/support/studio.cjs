/**
 * The workflow studio as the person sees it: the real renderer (fake DOM)
 * over a real Workspace on memory stores, so studio tests exercise the
 * workspace's own rules. Workspace commands are answered the way
 * main/ipc/workspace.cjs answers them.
 */
const { Workspace } = require('../../../scripts/workspace.cjs');
const { MemoryStore } = require('./stores.cjs');
const { loadRenderer, settle } = require('./renderer-harness.cjs');

/** Two steps that make a runnable workflow. */
const STEPS = [
  { id: 'a', action: 'navigate', url: 'https://x.test/' },
  { id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
];

/** Workspace commands the main process answers itself, by type. */
function mainCommands(ws, extra) {
  return {
    get: () => ws.snapshot(),
    validate: async () => (await ws.start({}), ws.snapshot()),
    control: (cmd) => (ws.control(cmd.command), ws.snapshot()),
    support: () => ({ ...ws.snapshot(), supportSaved: true }),
    ...extra,
  };
}

/** Answers one workspace command: the main process's own, or an edit. */
function answer(ws, commands, cmd) {
  if (Object.hasOwn(commands, cmd.type)) return commands[cmd.type](cmd);
  ws.edit(cmd);
  return ws.snapshot();
}

/** A workspace whose runner never starts a real run; `ws.receiveFromWorker` feeds it. */
function workspace(storage) {
  const ws = new Workspace({
    store: new MemoryStore({}, storage),
    runStore: new MemoryStore({}),
    notify: () => {},
    runner: async (draft, options, receive) => ((ws.receiveFromWorker = receive), { control() {} }),
  });
  return ws;
}

/** The studio loaded over a workspace holding `steps`; `commands` and `answers` override the main process. */
async function studioApp({ steps = STEPS, connected = true, commands = {}, answers = {}, storage } = {}) {
  const ws = workspace(storage);
  if (steps.length) ws.capture(structuredClone(steps), [], false);
  const table = mainCommands(ws, commands);
  const app = loadRenderer({ answers: { workspace: async (cmd) => answer(ws, table, cmd), ...answers } });
  // After start-up settles: the shell's own status read would otherwise answer "offline" last.
  await settle();
  app.bridge.emit('WsStatus', { connected });
  await settle();
  const push = async () => (app.bridge.emit('Workspace', ws.snapshot()), settle());
  return { app, ws, push, studio: () => app.run('Studio'), $: app.$, settle };
}

module.exports = { studioApp, STEPS };
