/**
 * The contract between the shell page and the main process: every call the
 * page may make and every event it may hear, with the IPC channel each rides
 * on. The preload builds `window.oyaBrowser` from these tables, the main
 * process registers a handler per call channel, and the renderer is typed by
 * `OyaBrowser`, so a channel missing on either side is a compile error.
 */

/** A value the main process sends as it is; its shape is owned by the feature that reads it. */
export type Payload = Record<string, unknown>;

/** The fields every failed call answers with. */
export interface Failure {
  /** What went wrong, in words for the person. */
  error?: string;
}

/** One open tab, as the tab strip draws it. */
export interface TabSummary {
  /** The tab's id in the main process. */
  id: number;
  /** The page's title. */
  title: string;
  /** The page's address. */
  url: string;
  /** Whether the tab shows the start page. */
  home: boolean;
  /** The page's icon as a data URL, once fetched. */
  favicon: string | null;
  /** Whether this is the tab on screen. */
  active: boolean;
  /** Whether the page is loading. */
  loading?: boolean;
  /** Why the last load failed, if it did. */
  error?: string | null;
  /** Whether Back has somewhere to go. */
  canGoBack?: boolean;
  /** Whether Forward has somewhere to go. */
  canGoForward?: boolean;
}

/** A rectangle in window pixels. */
export interface Bounds {
  /** Left edge. */
  x: number;
  /** Top edge. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
}

/** Where the page and the dev panel sit in the window (src/main/shell/shell-layout.ts). */
export interface ShellLayout {
  /** Whether the window is narrow, so the panel slides up instead of in. */
  compact: boolean;
  /** The panel's width beside the page. */
  panelWidth: number;
  /** The panel's height under the page, when compact. */
  panelHeight: number;
  /** How far the panel is open, 0 to 1. */
  progress: number;
  /** How many pixels the panel currently takes from the page. */
  reveal: number;
  /** The toolbar's height. */
  chromeHeight: number;
  /** The page's bounds in the window. */
  page: Bounds;
}

/** Who drives the browser (src/main/control/control-state.ts `snapshot()`). */
export interface ControlState extends Payload {
  /** 'agent' or 'human'. */
  mode?: string;
  /** Whether this desktop holds the control the mode names. */
  mine?: boolean;
  /** Whether a person may use the page right now. */
  interactive?: boolean;
}

/** The connection as the status pill shows it. */
export interface ConnectionStatus {
  /** Whether the control socket is up and authenticated. */
  connected: boolean;
  /** This browser's id on the server, once known. */
  browserId?: string | null;
  /** The persona's display name. */
  profileName?: string;
}

/** The persona's device and locale, for the debug bar (src/main/app/persona.ts). */
export type FingerprintSummary = Payload | null;

/** A view-source answer: the page's HTML and its markdown reading. */
export interface PageSource {
  /** The page's HTML. */
  html: string;
  /** The page as the agent reads it. */
  markdown: string;
  /** Why it could not be read, if it could not. */
  error?: string;
}

/** The connection, the active address and when cookies last synced. */
export interface ShellStatus extends ConnectionStatus {
  /** The active page's address. */
  url: string;
  /** Whether the shell is past the welcome screen. */
  browsing: boolean;
  /** When the server last confirmed the cookies, in ms since the epoch (0 for never). */
  syncedAt: number;
}

/** The answer to a control change: the state as it now stands, and why it failed if it did. */
export interface ControlChange extends Failure {
  /** Who drives now. */
  state: ControlState;
}

/** The answer to a save dialog. */
export interface SaveAnswer extends Failure {
  /** The file was written. */
  saved?: boolean;
  /** The person closed the dialog. */
  canceled?: boolean;
}

/** One of the project's personas. */
export interface PersonaChoice {
  /** The persona's id. */
  id: string;
  /** Its display name. */
  name: string;
  /** Whether it is the project's default. */
  isDefault?: boolean;
}

/** The project's personas and the active one's id. */
export interface PersonaList {
  /** Every persona (none while offline). */
  personas: PersonaChoice[];
  /** The persona this browser runs as. */
  active: string;
}

/** A live event from the agent's run. */
export interface AgentEvent {
  /** The run it belongs to. */
  runId: string;
  /** What happened. */
  event: Payload;
}

/**
 * Every call the shell page makes. Each returns a promise of what its handler
 * returns. `toggleDevPanel`'s argument is filled in by the preload.
 */
export interface ShellCalls {
  /** Confirms handoff when the person interacts with an agent-owned page. */
  requestTakeover(): Promise<void>;
  /** Lists, manages, imports, exports, and runs project playbooks on this browser. */
  playbooks(command: Payload): Promise<Payload>;

  /** Loads an address or search in the active tab (or leaves the start screen for it). */
  navigate(url: string): Promise<void>;
  /** Goes back in the active tab. */
  goBack(): Promise<void>;
  /** Goes forward in the active tab. */
  goForward(): Promise<void>;
  /** Reloads the active page. */
  reload(): Promise<void>;
  /** The saved settings. */
  getConfig(): Promise<Payload>;
  /** Saves settings and reconnects. */
  saveConfig(changes: Payload): Promise<boolean>;
  /** Opens the server's console in the system browser; answers its address. */
  openConsole(serverUrl?: string): Promise<string | null>;
  /** Forgets the key and goes back to the welcome screen. */
  signOut(): Promise<void>;
  /** The signed-in account, or the last one known. */
  getAccount(): Promise<Payload | null>;
  /** The browsers on this machine that logins can be imported from. */
  importSources(): Promise<Payload[]>;
  /** Imports a browser's logins again. */
  reimportBrowser(sourceId: string): Promise<void>;
  /** The connection, the active address and when cookies last synced. */
  getStatus(): Promise<ShellStatus>;
  /** Who drives the browser. */
  getControlState(): Promise<ControlState>;
  /** Asks to take or hand back control. */
  changeControl(action: string): Promise<ControlChange>;
  /** The shell drew the backdrop it was sent; `token` is the string the backdrop carried. */
  backdropReady(token: string): Promise<void>;
  /** Leaves the welcome screen for the start page. */
  enterBrowsing(): Promise<void>;
  /** An overlay opened over the page. */
  showOverlay(name: string): Promise<void>;
  /** An overlay closed. */
  hideOverlay(name: string): Promise<void>;
  /** The shell's appearance settings and the platform. */
  getUiPreferences(): Promise<Payload>;
  /** Saves the shell's appearance settings. */
  saveUiPreferences(preferences: Payload): Promise<boolean>;
  /** Saves a workflow as a Playwright script. */
  exportPlaywright(payload: Payload): Promise<SaveAnswer>;
  /** Opens or closes the workspace panel. */
  toggleDevPanel(reducedMotion?: boolean): Promise<void>;
  /** Opens a tab; answers its id. */
  newTab(url?: string): Promise<number>;
  /** Closes a tab. */
  closeTab(id: number): Promise<void>;
  /** Shows a tab. */
  activateTab(id: number): Promise<void>;
  /** Moves a tab in the strip. */
  moveTab(id: number, toIndex: number): Promise<unknown>;
  /** Opens a tab's right-click menu. */
  showTabMenu(id: number): Promise<void>;
  /** Runs a workflow studio command; answers the new studio state. */
  workspace(command: Payload): Promise<Payload>;
  /** Starts recording a workflow. */
  startRecording(): Promise<unknown>;
  /** Stops recording. */
  stopRecording(): Promise<unknown>;
  /** Saves the recording as a playbook. */
  saveRecording(name: string, description?: string): Promise<unknown>;
  /** Sets the workspace panel width. */
  resizeDevPanel(width: number): Promise<void>;
  /** The active page's HTML and markdown. */
  getPageSource(): Promise<PageSource>;
  /** Renders an analysis in a page format ('' for an unknown format or no analysis). */
  renderPage(analysis: unknown, format: string): Promise<string>;
  /** Runs an agent page action by hand. */
  devAction(action: string, params?: Payload): Promise<Payload>;
  /** Sends the conversation to the agent; answers its reply. */
  sendChat(messages: Payload[], data?: Payload): Promise<Payload & Failure>;
  /** Stops the running chat. */
  stopChat(): Promise<void>;
  /** The project's model and whether it has a key. */
  modelStatus(): Promise<Payload>;
  /** Saves the project's model provider and key. */
  saveModelKey(choice: Payload): Promise<Payload & Failure>;
  /** The project's personas and the active one. */
  listPersonas(): Promise<PersonaList>;
  /** The routines and their runs. */
  listRoutines(): Promise<Payload>;
  /** Creates or updates a routine. */
  saveRoutine(routine: Payload): Promise<Payload>;
  /** Deletes a routine. */
  deleteRoutine(id: string): Promise<Payload>;
  /** Runs a routine at once. */
  runRoutineNow(id: string): Promise<Payload>;
  /** Turns a routine on or off. */
  setRoutineEnabled(id: string, enabled: boolean): Promise<Payload>;
  /** Clears a routine's runs. */
  clearRoutineHistory(id: string): Promise<Payload>;
  /** Stops this app's run of a routine; answers whether one was stopped. */
  stopRoutine(id: string): Promise<boolean>;
  /** Saves the agent's last run as a playbook. */
  saveChatPlaybook(name: string): Promise<Payload & Failure>;
  /** The persona's device and locale. */
  getFingerprint(): Promise<FingerprintSummary>;
  /** Sends the cookies and storage to the server now. */
  saveProfile(): Promise<void>;
  /** The app's version. */
  getVersion(): Promise<string>;
  /** Where the updater is. */
  getUpdateStatus(): Promise<Payload>;
  /** Checks for an update now. */
  checkForUpdates(): Promise<unknown>;
  /** Quits and installs the downloaded update. */
  installUpdate(): Promise<void>;
}

/** Every event the shell page hears, with its payload. */
export interface ShellEvents {
  /** A login import moved on. */
  onMirrorStatus: Payload;
  /** Control changed hands. */
  onControlState: ControlState;
  /** A still of the page to show behind a dialog, or null to drop it. */
  onPageBackdrop: Payload | null;
  /** The window or panel moved. */
  onShellLayout: ShellLayout;
  /** A keyboard shortcut or menu command for the shell. */
  onShellCommand: string;
  /** The system went dark (true) or light. */
  onShellAppearance: boolean;
  /** The workflow studio state changed. */
  onWorkspace: Payload;
  /** The active tab's address changed. */
  onUrlChanged: string;
  /** The active tab's title changed. */
  onTitleChanged: string;
  /** The connection changed. */
  onWsStatus: ConnectionStatus;
  /** A live event from the agent's run. */
  onAgentEvent: AgentEvent;
  /** The shell left or entered the welcome screen. */
  onModeChanged: 'setup' | 'browsing';
  /** A line for the activity log. */
  onDevLog: Payload;
  /** The workspace panel opened (true) or closed. */
  onDevPanelState: boolean;
  /** What an Inspect read found. */
  onInspectResult: Payload;
  /** A view-source request from the page menu. */
  onViewSource: PageSource;
  /** The tab list changed. */
  onTabsUpdated: TabSummary[];
  /** The routines or their runs changed. */
  onRoutinesChanged: Payload;
  /** The server stored the profile. */
  onProfileSaved: Payload;
  /** The project's settings changed on the server. */
  onSettingsChanged: Payload;
  /** The persona changed. */
  onFingerprintChanged: FingerprintSummary;
  /** The updater moved on. */
  onUpdateStatus: Payload;
}

/** The IPC channel of each call. */
export const CALL_CHANNELS = {
  /** Confirm a human handoff without replaying blocked page input. */
  requestTakeover: 'request-takeover',
  /** Project playbook library and replay operations. */
  playbooks: 'playbooks',

  /** Loads an address or search in the active tab (or leaves the start screen for it). */
  navigate: 'navigate',
  /** Goes back in the active tab. */
  goBack: 'go-back',
  /** Goes forward in the active tab. */
  goForward: 'go-forward',
  /** Reloads the active page. */
  reload: 'reload',
  /** The saved settings. */
  getConfig: 'get-config',
  /** Saves settings and reconnects. */
  saveConfig: 'save-config',
  /** Opens the server's console in the system browser; answers its address. */
  openConsole: 'open-console',
  /** Forgets the key and goes back to the welcome screen. */
  signOut: 'sign-out',
  /** The signed-in account, or the last one known. */
  getAccount: 'get-account',
  /** The browsers on this machine that logins can be imported from. */
  importSources: 'import-sources',
  /** Imports a browser's logins again. */
  reimportBrowser: 'reimport-browser',
  /** The connection, the active address and when cookies last synced. */
  getStatus: 'get-status',
  /** Who drives the browser. */
  getControlState: 'get-control-state',
  /** Asks to take or hand back control. */
  changeControl: 'change-control',
  /** The shell drew the backdrop it was sent. */
  backdropReady: 'backdrop-ready',
  /** Leaves the welcome screen for the start page. */
  enterBrowsing: 'enter-browsing',
  /** An overlay opened over the page. */
  showOverlay: 'show-overlay',
  /** An overlay closed. */
  hideOverlay: 'hide-overlay',
  /** The shell's appearance settings and the platform. */
  getUiPreferences: 'get-ui-preferences',
  /** Saves the shell's appearance settings. */
  saveUiPreferences: 'save-ui-preferences',
  /** Saves a workflow as a Playwright script. */
  exportPlaywright: 'export-playwright',
  /** Opens or closes the workspace panel. */
  toggleDevPanel: 'toggle-dev-panel',
  /** Opens a tab; answers its id. */
  newTab: 'new-tab',
  /** Closes a tab. */
  closeTab: 'close-tab',
  /** Shows a tab. */
  activateTab: 'activate-tab',
  /** Moves a tab in the strip. */
  moveTab: 'move-tab',
  /** Opens a tab's right-click menu. */
  showTabMenu: 'tab-menu',
  /** Runs a workflow studio command; answers the new studio state. */
  workspace: 'workspace',
  /** Starts recording a workflow. */
  startRecording: 'start-recording',
  /** Stops recording. */
  stopRecording: 'stop-recording',
  /** Saves the recording as a playbook. */
  saveRecording: 'save-recording',
  /** Sets the workspace panel width. */
  resizeDevPanel: 'resize-dev-panel',
  /** The active page's HTML and markdown. */
  getPageSource: 'get-page-source',
  /** Renders an analysis in a page format. */
  renderPage: 'render-page',
  /** Runs an agent page action by hand. */
  devAction: 'dev-action',
  /** Sends the conversation to the agent; answers its reply. */
  sendChat: 'send-chat',
  /** Stops the running chat. */
  stopChat: 'stop-chat',
  /** The project's model and whether it has a key. */
  modelStatus: 'model-status',
  /** Saves the project's model provider and key. */
  saveModelKey: 'save-model-key',
  /** The project's personas and the active one. */
  listPersonas: 'list-personas',
  /** The routines and their runs. */
  listRoutines: 'list-routines',
  /** Creates or updates a routine. */
  saveRoutine: 'save-routine',
  /** Deletes a routine. */
  deleteRoutine: 'delete-routine',
  /** Runs a routine at once. */
  runRoutineNow: 'run-routine-now',
  /** Turns a routine on or off. */
  setRoutineEnabled: 'set-routine-enabled',
  /** Clears a routine's runs. */
  clearRoutineHistory: 'clear-routine-history',
  /** Stops a routine's run. */
  stopRoutine: 'stop-routine',
  /** Saves the agent's last run as a playbook. */
  saveChatPlaybook: 'save-chat-playbook',
  /** The persona's device and locale. */
  getFingerprint: 'get-fingerprint',
  /** Sends the cookies and storage to the server now. */
  saveProfile: 'save-profile',
  /** The app's version. */
  getVersion: 'get-version',
  /** Where the updater is. */
  getUpdateStatus: 'get-update-status',
  /** Checks for an update now. */
  checkForUpdates: 'check-for-updates',
  /** Quits and installs the downloaded update. */
  installUpdate: 'install-update',
} as const satisfies Record<keyof ShellCalls, string>;

/** The IPC channel of each event. */
export const EVENT_CHANNELS = {
  /** A login import moved on. */
  onMirrorStatus: 'mirror-status',
  /** Control changed hands. */
  onControlState: 'control-state',
  /** A still of the page to show behind a dialog, or null to drop it. */
  onPageBackdrop: 'page-backdrop',
  /** The window or panel moved. */
  onShellLayout: 'shell-layout',
  /** A keyboard shortcut or menu command for the shell. */
  onShellCommand: 'shell-command',
  /** The system went dark (true) or light. */
  onShellAppearance: 'shell-appearance',
  /** The workflow studio state changed. */
  onWorkspace: 'workspace-state',
  /** The active tab's address changed. */
  onUrlChanged: 'url-changed',
  /** The active tab's title changed. */
  onTitleChanged: 'title-changed',
  /** The connection changed. */
  onWsStatus: 'ws-status',
  /** A live event from the agent's run. */
  onAgentEvent: 'agent-event',
  /** The shell left or entered the welcome screen. */
  onModeChanged: 'mode-changed',
  /** A line for the activity log. */
  onDevLog: 'dev-log',
  /** The workspace panel opened (true) or closed. */
  onDevPanelState: 'dev-panel-state',
  /** What an Inspect read found. */
  onInspectResult: 'inspect-result',
  /** A view-source request from the page menu. */
  onViewSource: 'view-source',
  /** The tab list changed. */
  onTabsUpdated: 'tabs-updated',
  /** The routines or their runs changed. */
  onRoutinesChanged: 'routines-changed',
  /** The server stored the profile. */
  onProfileSaved: 'profile-saved',
  /** The project's settings changed on the server. */
  onSettingsChanged: 'settings-changed',
  /** The persona changed. */
  onFingerprintChanged: 'fingerprint-changed',
  /** The updater moved on. */
  onUpdateStatus: 'update-status',
} as const satisfies Record<keyof ShellEvents, string>;

/** A call's channel name. */
export type CallChannel = (typeof CALL_CHANNELS)[keyof ShellCalls];

/** An event's channel name. */
export type EventChannel = (typeof EVENT_CHANNELS)[keyof ShellEvents];

/** The bridge method that rides on call channel `C`. */
export type CallOf<C extends CallChannel> = {
  [K in keyof ShellCalls]: (typeof CALL_CHANNELS)[K] extends C ? K : never;
}[keyof ShellCalls];

/** The arguments call channel `C` carries. */
export type CallArgs<C extends CallChannel> = Parameters<ShellCalls[CallOf<C>]>;

/** What call channel `C` answers with, before the promise. */
export type CallResult<C extends CallChannel> = Awaited<ReturnType<ShellCalls[CallOf<C>]>>;

/** The payload event channel `E` carries. */
export type EventPayload<E extends EventChannel> = ShellEvents[{
  [K in keyof ShellEvents]: (typeof EVENT_CHANNELS)[K] extends E ? K : never;
}[keyof ShellEvents]];

/** Subscribes to an event; the returned function unsubscribes. */
export type Subscribe<T> = (listener: (payload: T) => void) => () => void;

/** `window.oyaBrowser`: every call, and a subscription per event. */
export type OyaBrowser = ShellCalls & { [K in keyof ShellEvents]: Subscribe<ShellEvents[K]> };
