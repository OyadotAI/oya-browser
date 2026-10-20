/**
 * Every main-process service, by name: what the composition root
 * (src/main/main.ts) builds once and hands to each service's constructor.
 * A service names the slice it uses: `type Deps = Pick<AppServices, 'tabs' | 'shell'>`.
 *
 * The services call one another in a cycle (tabs join recordings, the recorder
 * reads tabs, the socket applies personas, the persona closes tabs), which is
 * why they share one object built in steps rather than taking each other as
 * constructor arguments directly.
 */
import type * as Electron from 'electron';
import type { Workspace } from '../workflow/workspace.ts';
import type { PageDriver } from '../actions/driver.ts';
import type { DesktopDialogs } from '../dialogs/index.ts';
import type { World } from '../native/index.ts';
import type { CdpRelay } from '../connection/cdp-relay.ts';
import type { CommandRunner } from '../connection/commands.ts';
import type { ControlSocket } from '../connection/socket.ts';
import type { LiveStream } from '../connection/stream.ts';
import type { DesktopControl } from '../control/control-state.ts';
import type { Mirror } from '../mirror/mirror.ts';
import type { ChatgptAuth, Onboarding } from '../chatgpt/index.ts';
import type { Observer } from '../observe/observer.ts';
import type { Recorder } from '../recording/recorder.ts';
import type { Routines } from '../routines/routines.ts';
import type { ControlShield } from '../shell/control-shield.ts';
import type { FindBar } from '../shell/find-bar.ts';
import type { PanelLayout } from '../shell/layout.ts';
import type { Overlays } from '../shell/overlays.ts';
import type { Shortcuts } from '../shell/shortcuts.ts';
import type { ShellWindow } from '../shell/window.ts';
import type { CookieSync } from '../sync/cookie-sync.ts';
import type { Protection } from '../tabs/protection.ts';
import type { TabManager } from '../tabs/tabs.ts';
import type { WorkerCoverage } from '../tabs/workers.ts';
import type { ConfigStore } from './config-store.ts';
import type { DeepLinks } from './deep-links.ts';
import type { Persona } from './persona.ts';
import type { Updater } from './updater.ts';
import type { Governance } from '../identity/governance.ts';
import type { AgentTab } from '../control/agent-tab.ts';

/** The main process's services and the values they share. */
export interface AppServices {
  /** Native window ownership, absent only in isolated unit-test contexts. */
  windows?: import('../windows/index.ts').BrowserWindows;
  /** The incognito windows' shared throwaway session (src/main/incognito/). */
  incognito: import('../incognito/index.ts').IncognitoSession;
  /** Native session lifecycle; production requires complete engine-owned persona protection. */
  nativeBrowsing?: boolean;

  /** Native passkey account selection, never exposed to agent tools. */
  passkeys: import('./passkeys.ts').Passkeys;
  /** Explicit microphone and camera permission prompts. */
  mediaPermissions: import('./media-permissions.ts').MediaPermissions;
  /** Asks before a window close or a quit ends a call; owns whether a quit is going through. */
  callGuard: import('./call-guard.ts').CallGuard;
  /** Explicit screen-share prompts, through the same gate. */
  screenShare?: import('./screen-share.ts').ScreenShare;
  /** Native tab/window/display previews and an explicit source choice on each share. */
  screenPicker: import('../screen-share/index.ts').ScreenPicker;
  /** Which tabs a call is capturing, for the strip's ring and the toolbar's share chip. */
  shareIndicator?: import('../screen-share/index.ts').ShareIndicator;
  /** The sites each profile allowed a device on, on disk. */
  sitePermissions: import('./site-permissions.ts').SitePermissionStore;
  /** The tabs open at quit, brought back at the next sign-in. */
  sessionTabs?: import('../tabs/session-tabs.ts').SessionTabs;
  /** The log of tab renderer deaths, Reload recoveries and helper-process deaths (tabs/tab-health.ts). */
  tabHealthLog?: import('../tabs/tab-health.ts').TabHealthLog;
  /** The main window's position and size at quit, reopened at the next launch. */
  windowBounds?: import('../shell/window-bounds.ts').WindowBounds;
  /** Human-approved desktop app protocol handoff. */
  externalApps: import('./external-apps.ts').ExternalApps;
  /** Session-only alert inbox. */
  notifications: import('../notifications/index.ts').Notifications;
  /** Local history and bookmarks for the current persona. */
  library: import('../library/index.ts').BrowsingLibrary;
  /** The person's downloads, listed on the Downloads page. */
  downloads: import('../downloads/index.ts').Downloads;
  /** Each window's download chip: what started there, told only to that window. */
  downloadBubble: import('../downloads/index.ts').DownloadBubble;
  /** Electron's main-process API; only the composition root imports it. */
  electron: typeof Electron;
  /** The app's folder: the package root in development, the asar when packaged. */
  appDir: string;
  /** The page reader's source, injected into every page's isolated world. */
  analyzerScript: string;
  /** The isolated world's random name. */
  isolatedWorld: string;
  /** The public CDP port for harnesses, or 0 when off. */
  cdpPort: number;
  /** The secret the CDP relay presents to the front door. */
  relayToken: string;
  /** What pages said and fetched (src/main/observe/observer.ts). */
  observer: Observer;
  /** The managed egress rules and the control mode they read (src/main/identity/governance.ts). */
  governance: Governance;
  /** The workflow studio, once booted (src/main/workflow/workspace.ts); null until then. */
  workspace: Workspace | null;
  /** config.json (src/main/app/config-store.ts). */
  config: ConfigStore;
  /** The shell window (src/main/shell/window.ts). */
  shell: ShellWindow;
  /** Keyboard shortcuts (src/main/shell/shortcuts.ts). */
  shortcuts: Shortcuts;
  /** The control shield over the page (src/main/shell/control-shield.ts). */
  shield: ControlShield;
  /** The page and panel layout (src/main/shell/layout.ts). */
  layout: PanelLayout;
  /** Overlays over the page (src/main/shell/overlays.ts). */
  overlays: Overlays;
  /** Find in page on the shown tab (src/main/shell/find-bar.ts). */
  findBar: FindBar;
  /** The tabs (src/main/tabs/tabs.ts). */
  tabs: TabManager;
  /** A tab's protection before its first page (src/main/tabs/protection.ts). */
  protection: Protection;
  /** Service and shared workers' coverage (src/main/tabs/workers.ts). */
  workers: WorkerCoverage;
  /** Native JavaScript dialogs with human/agent ownership on every tab and popup. */
  dialogs: DesktopDialogs;
  /** The active persona (src/main/app/persona.ts). */
  persona: Persona;
  /** Recording demonstrations (src/main/recording/recorder.ts). */
  recorder: Recorder;
  /** The control socket to the server (src/main/connection/socket.ts). */
  socket: ControlSocket;
  /** Runs the server's commands (src/main/connection/commands.ts). */
  commands: CommandRunner;
  /** oya:// links (src/main/app/deep-links.ts). */
  deepLinks: DeepLinks;
  /** Oya's MCP port for coding agents on this computer (src/main/local-mcp), once started. */
  localMcp?: import('../local-mcp/index.ts').LocalMcp;
  /** Scheduled routines (src/main/routines/routines.ts). */
  routines: Routines;
  /** The analyzer's isolated world (src/main/native/world.ts). */
  world: World;
  /** Who drives: agent or person (src/main/control/control-state.ts). */
  control: DesktopControl;
  /** The one tab the agent drives while the rest stay the person's (src/main/control/agent-tab.ts). */
  agentTab: AgentTab;
  /** The cookie pool sync (src/main/sync/cookie-sync.ts). */
  cookies: CookieSync;
  /** CDP relayed over the control socket (src/main/connection/cdp-relay.ts). */
  relay: CdpRelay;
  /** Logins imported from the person's browsers (src/main/mirror/mirror.ts). */
  mirror: Mirror;
  /** This app's ChatGPT sign-in, which makes the model calls for the server's agent (src/main/chatgpt). */
  chatgpt: ChatgptAuth;
  /** The first-run checklist: ChatGPT, logins, default browser (src/main/chatgpt/onboarding.ts). */
  onboarding: Onboarding;
  /** The live view (src/main/connection/stream.ts). */
  stream: LiveStream;
  /** The agent's page commands (src/main/actions/driver.ts). */
  actions: PageDriver;
  /** The auto-updater and the toolbar's view of it (src/main/app/updater.ts). */
  updater: Updater;
  /** Stops the running chat, while one runs (src/main/connection/chat.ts). */
  chatAbort: AbortController | null;
  /** Each tab's chat in flight, by tab id (src/main/connection/chat.ts). */
  chatRuns: Map<number, AbortController>;
  /** Tabs a validation run left open to show where it ended. */
  validationTabs: Set<number> | undefined;
}
