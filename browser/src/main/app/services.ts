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
import type { Dialogs } from '../cdp/dialogs.ts';
import type { World } from '../cdp/world.ts';
import type { CdpRelay } from '../connection/cdp-relay.ts';
import type { CommandRunner } from '../connection/commands.ts';
import type { ControlSocket } from '../connection/socket.ts';
import type { LiveStream } from '../connection/stream.ts';
import type { DesktopControl } from '../control/control-state.ts';
import type { Mirror } from '../mirror/mirror.ts';
import type { Observer } from '../observe/observer.ts';
import type { Recorder } from '../recording/recorder.ts';
import type { Routines } from '../routines/routines.ts';
import type { ControlShield } from '../shell/control-shield.ts';
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

/** The main process's services and the values they share. */
export interface AppServices {
  /** Explicit microphone and camera permission prompts. */
  mediaPermissions: import('./media-permissions.ts').MediaPermissions;
  /** Human-approved desktop app protocol handoff. */
  externalApps: import('./external-apps.ts').ExternalApps;
  /** Session-only alert inbox. */
  notifications: import('../notifications/index.ts').Notifications;
  /** Local history and bookmarks for the current persona. */
  library: import('../library/index.ts').BrowsingLibrary;
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
  /** The tabs (src/main/tabs/tabs.ts). */
  tabs: TabManager;
  /** A tab's protection before its first page (src/main/tabs/protection.ts). */
  protection: Protection;
  /** Service and shared workers' coverage (src/main/tabs/workers.ts). */
  workers: WorkerCoverage;
  /** Native JavaScript dialogs on every tab and popup (src/main/cdp/dialogs.ts). */
  dialogs: Dialogs;
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
  /** Scheduled routines (src/main/routines/routines.ts). */
  routines: Routines;
  /** The analyzer's isolated world (src/main/cdp/world.ts). */
  world: World;
  /** Who drives: agent or person (src/main/control/control-state.ts). */
  control: DesktopControl;
  /** The cookie pool sync (src/main/sync/cookie-sync.ts). */
  cookies: CookieSync;
  /** CDP relayed over the control socket (src/main/connection/cdp-relay.ts). */
  relay: CdpRelay;
  /** Logins imported from the person's browsers (src/main/mirror/mirror.ts). */
  mirror: Mirror;
  /** The live view (src/main/connection/stream.ts). */
  stream: LiveStream;
  /** The agent's page commands (src/main/actions/driver.ts). */
  actions: PageDriver;
  /** The auto-updater and the toolbar's view of it (src/main/app/updater.ts). */
  updater: Updater;
  /** Stops the running chat, while one runs (src/main/connection/chat.ts). */
  chatAbort: AbortController | null;
  /** Tabs a validation run left open to show where it ended. */
  validationTabs: Set<number> | undefined;
}
