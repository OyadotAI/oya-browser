/**
 * Server commands the main process answers itself rather than a page: tabs,
 * recording and workflow playback. Command → handler(runner, id, params); each
 * answers through runner.sendResult.
 */
import { TRACE_COMMANDS } from './trace-commands.ts';
import { NOTIFICATION_COMMANDS } from './notification-commands.ts';
import { SHORTCUT_COMMANDS } from './shortcut-commands.ts';
import { LIBRARY_COMMANDS } from './library-commands.ts';
import { normalizeDraft } from '../../workflow/index.ts';
import { isWebAddress, NOT_A_WEB_ADDRESS } from '../tabs/navigation.ts';
import { whenProtected } from '../tabs/load.ts';
import { sleep } from '../input/timing.ts';
import type { CommandId } from '../actions/types.ts';
import type { CommandRunner, CommandDeps, RemoteParams } from './commands.ts';
import { WORKFLOW_LIMIT_MS, WORKFLOW_POLL_MS } from './constants.ts';
import { HOME_URL } from '../tabs/constants.ts';

/** A command answered without the active page. */
export type TabHandler = (
  runner: CommandRunner,
  id: CommandId | undefined,
  params: RemoteParams | undefined,
) => Promise<void> | void;

/** A tab as list_tabs reports it. */
interface ListedTab {
  /** The tab's number. */
  id: number;
  /** Its page title. */
  title: string;
  /** Its page address. */
  url: string;
}

/** Existing tab result fields stay stable when window ownership is added. */
function tSummary(tab: ListedTab, active: number | null) {
  return { id: tab.id, title: tab.title, url: tab.url, active: tab.id === active };
}

/** Loads the server's draft into the workspace as the one to play. */
function loadWorkflowDraft(deps: CommandDeps, params: RemoteParams | undefined): void {
  const workspace = deps.workspace!; // assertCanPlay saw it
  workspace.persist();
  workspace.draft = normalizeDraft({ ...params!.draft, id: crypto.randomUUID(), phase: 'paused' });
  Object.assign(workspace, { history: [], future: [] });
  workspace.persist();
  deps.recorder.adopt(structuredClone(workspace.draft.steps), workspace.draft.secrets);
}

/** Waits for the run, and interrupts one that lost its connection or ran too long. */
async function waitForWorkflow(deps: CommandDeps): Promise<void> {
  const workspace = deps.workspace!; // assertCanPlay saw it
  const started = Date.now();
  while (workspace.busy() && Date.now() - started < WORKFLOW_LIMIT_MS && deps.socket.ready)
    await sleep(WORKFLOW_POLL_MS);
  if (!workspace.busy()) return;
  workspace.session?.dispose();
  const error = 'Remote validation disconnected or exceeded its time limit. Check the website before retrying.';
  workspace.receive({ type: 'finished', status: 'interrupted', error });
}

/** Only one recording or validation at a time. */
function assertCanPlay(deps: CommandDeps): void {
  if (!deps.workspace || deps.workspace.busy() || deps.recorder.recording) {
    throw new Error('Finish the active recording or validation before playing a workflow');
  }
}

/**
 * The server drives recording too, so a flow can be demonstrated from the
 * dashboard's live view. Both routes share one buffer: the panel here and the
 * dashboard show the same steps.
 */
async function playWorkflow(runner: CommandRunner, id: CommandId | undefined, params?: RemoteParams): Promise<void> {
  const { deps } = runner;
  assertCanPlay(deps);
  loadWorkflowDraft(deps, params);
  const workspace = deps.workspace!; // assertCanPlay saw it
  await workspace.start({ vars: params!.variables || {}, autoHeal: params!.autoHeal !== false });
  await waitForWorkflow(deps);
  const run = workspace.run!; // start() began one
  runner.sendResult(id, true, { id: run.id, status: run.status, assertions: run.assertions || 0, error: run.error });
}

/** Records the lane's starting tab and opens a new tab in its lane without stealing human focus. */
function createAgentTab({ tabs, agentTab }: CommandDeps, url: string): number {
  agentTab.noteHome?.();
  return agentTab.opened(tabs.createTab(url, agentTab.following()));
}

/** Opens a tab on a web address and answers once it is ready; any other address is the caller's way into this machine, and a tab that could not be protected is refused. */
async function openTab(runner: CommandRunner, id: CommandId | undefined, params?: RemoteParams): Promise<void> {
  const { tabs, actions } = runner.deps;
  const url = params?.url || 'about:blank';
  if (!isWebAddress(url)) return runner.sendResult(id, false, null, NOT_A_WEB_ADDRESS);
  // With no address it opens on the start page, which the agent drives like a blank one, so a person never sees a bare about:blank.
  const tabId = createAgentTab(runner.deps, params?.url ? url : HOME_URL);
  const tab = tabs.list.find((t: ListedTab) => t.id === tabId);
  await whenProtected(tab);
  await actions.waitForTabReady(tab);
  runner.sendResult(id, true, { tab_id: tabId, url });
}

/** Why the agent may not take tab `tabId`: it does not exist, or the person is using it beside the agent's. */
function tabRefusal(deps: CommandRunner['deps'], tabId: number | undefined): string | null {
  if (!deps.tabs.find(tabId)) return `Tab ${tabId} not found`;
  if (deps.agentTab.othersDrive(tabId)) return 'Another agent is using that tab. Use another tab, or open_tab.';
  const theirs = !deps.agentTab.following() && tabId === deps.tabs.activeTabId;
  return theirs ? 'The person is using that tab. Use another tab, or open_tab.' : null;
}

/**
 * Whether the running agent may work in tab `tabId`: its first tab or one it opened. With
 * no lanes (no server agents, such as a local MCP agent on a desktop not signed in), it
 * drives the tab in front, so that one too, never the person's other tabs.
 */
function agentMay(deps: CommandRunner['deps'], tabId: number): boolean {
  if (deps.agentTab.mayWorkIn(tabId)) return true;
  return !deps.agentTab.scoped() && tabId === deps.tabs.activeTabId;
}

/** An agent works in its own tab (its first one, or one it opened); the person's other tabs and other agents' are not its to take. */
function switchRefusal(deps: CommandRunner['deps'], tabId: number): string | null {
  if (agentMay(deps, tabId)) return null;
  return 'Not switched: that tab belongs to the person or another agent. Work in your own tab; use open_tab to open what you need.';
}

/** Closing is the one tab action that cannot be undone, so an agent closes only the tabs it opened itself. */
function closeRefusal(deps: CommandRunner['deps'], tabId: number): string | null {
  if (deps.agentTab.openedByThisLane(tabId)) return null;
  return 'Not closed: an agent may close only tabs it opened itself with open_tab. Leave this tab open and use switch_tab or open_tab.';
}

/** Command → handler, for commands that do not need the active page. */
export const TAB_COMMANDS: Record<string, TabHandler> = {
  ...LIBRARY_COMMANDS,
  ...TRACE_COMMANDS,
  ...NOTIFICATION_COMMANDS,
  ...SHORTCUT_COMMANDS,
  workflow: playWorkflow,
  record: async (runner, id, params) => {
    const recorder = runner.deps.recorder;
    const result = await recorder.queueRecording(() => recorder.remote(params?.mode));
    runner.sendResult(id, true, result);
  },
  list_tabs: (runner, id) => {
    const { list } = runner.deps.tabs;
    const mine = (tabId: number) => agentMay(runner.deps, tabId);
    const tabs = list.map((t: ListedTab) => ({
      ...tSummary(t, runner.deps.agentTab.tabId()),
      yours: mine(t.id),
      window_id: runner.deps.windows?.owner(t.id)?.shell.window?.id,
    }));
    runner.sendResult(id, true, { tabs });
  },
  read_console: (runner, id, params) => {
    const entries = runner.deps.observer?.readConsole(params || {}) || [];
    runner.sendResult(id, true, { entries });
  },
  read_network: (runner, id, params) => {
    const requests = runner.deps.observer?.readNetwork(params || {}) || [];
    runner.sendResult(id, true, { requests });
  },
  open_tab: openTab,
  switch_tab: (runner, id, params) => {
    const refused = tabRefusal(runner.deps, params?.tab_id) ?? switchRefusal(runner.deps, params!.tab_id!);
    if (refused) return runner.sendResult(id, false, null, refused);
    const { tabs, agentTab, layout } = runner.deps;
    if (layout.open) layout.openFor(params!.tab_id!); // the panel the person had open follows the agent
    if (agentTab.following()) tabs.activateTab(params!.tab_id!); // found by tabRefusal
    agentTab.claim(params?.tab_id);
    runner.sendResult(id, true, { tab_id: params?.tab_id });
  },
  close_tab: (runner, id, params) => {
    const target = params?.tab_id || runner.deps.agentTab.tabId()!;
    const refused = tabRefusal(runner.deps, target) ?? closeRefusal(runner.deps, target);
    if (refused) return runner.sendResult(id, false, null, refused);
    runner.deps.tabs.closeTab(target);
    runner.sendResult(id, true, { closed: true });
  },
};
