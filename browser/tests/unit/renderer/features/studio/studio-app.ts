/**
 * The workflow studio's ViewModel over a real Workspace on memory stores, so
 * studio tests exercise the workspace's own rules. Workspace commands are
 * answered the way main/ipc/workspace.cjs answers them; `commands` overrides
 * one, `answers` any other bridge call.
 */
import { Workspace } from '../../../../../src/main/workflow/workspace.ts';
import { MemoryStore } from '../../../support/stores.cjs';
import { StudioViewModel } from '../../../../../src/renderer/features/studio/view-models/studio-view-model.ts';
import { ShellViewModel } from '../../../../../src/renderer/app/shell-view-model.ts';
import { PanelViewModel } from '../../../../../src/renderer/app/panel/panel-view-model.ts';
import { fakeBridge, manualFrames } from '../../support/bridge.ts';

/** Two steps that make a runnable workflow. */
export const STEPS = [
  { id: 'a', action: 'navigate', url: 'https://x.test/' },
  { id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
];

/** Lets pending promises and IPC answers settle. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await new Promise((resolve) => setImmediate(resolve));
}

/** A workspace whose runner never starts a real run; `ws.receiveFromWorker` feeds it. */
function workspace(storage?: any): any {
  const ws: any = new Workspace({
    store: new MemoryStore({}, storage),
    runStore: new MemoryStore({}),
    notify: () => {},
    runner: async (_draft: unknown, _options: unknown, receive: unknown) => (
      (ws.receiveFromWorker = receive),
      { control() {} }
    ),
  });
  return ws;
}

/** Workspace commands the main process answers itself, by type. */
function mainCommands(ws: any, extra: Record<string, (cmd: any) => unknown>) {
  return {
    get: () => ws.snapshot(),
    validate: async () => (await ws.start({}), ws.snapshot()),
    control: (cmd: any) => (ws.control(cmd.command), ws.snapshot()),
    support: () => ({ ...ws.snapshot(), supportSaved: true }),
    'export-json': () => ({ ...ws.snapshot(), exported: true }),
    ...extra,
  };
}

/** Answers one workspace command: the main process's own, or an edit. */
function answer(ws: any, commands: Record<string, (cmd: any) => unknown>, cmd: any) {
  if (Object.hasOwn(commands, cmd.type)) return commands[cmd.type](cmd);
  ws.edit(cmd);
  return ws.snapshot();
}

/** What a studio test may set up. */
interface Options {
  /** The steps the draft starts with. */
  steps?: unknown[];
  /** Whether the shell is connected. */
  connected?: boolean;
  /** Workspace command answers by type, in place of the main process's. */
  commands?: Record<string, (cmd: any) => unknown>;
  /** Other bridge answers by method. */
  answers?: Record<string, unknown>;
  /** The draft store's options ({ failSave }). */
  storage?: any;
  /** The control gate: { blocked, admit }. */
  gate?: any;
  /** Make Copy code fail. */
  clipboardFails?: boolean;
}

/** The studio over a workspace holding `steps`. */
export async function studioApp(options: Options = {}) {
  const { steps = STEPS, connected = true, commands = {}, answers = {}, storage, gate = {} } = options;
  const ws = workspace(storage);
  if (steps.length) ws.capture(structuredClone(steps), [], false);
  const table = mainCommands(ws, commands);
  const fake = fakeBridge({ workspace: async (cmd: any) => answer(ws, table, cmd), ...answers });
  const shell = new ShellViewModel(fake.bridge);
  const panel = new PanelViewModel(fake.bridge, manualFrames());
  const copied: string[] = [];
  const clipboard = {
    writeText: async (text: string) => {
      if (options.clipboardFails) throw new Error('denied');
      copied.push(text);
    },
  };
  const recordGate = { blocked: () => !!gate.blocked, admit: async () => gate.admit !== false };
  const vm = new StudioViewModel({ bridge: fake.bridge, shell, panel, gate: recordGate, clipboard });
  await settle();
  fake.emit('onWsStatus', { connected });
  const push = async () => (fake.emit('onWorkspace', ws.snapshot()), settle());
  return { vm, ws, fake, shell, panel, copied, push };
}
