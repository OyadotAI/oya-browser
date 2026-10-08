/**
 * Start-up: what runs once Electron is ready, in order. A signed-in launch
 * opens straight to browsing, on the Oya start page, which loads nothing from
 * the web. Session cookies are not kept across a restart, so a site loaded
 * before the server's cookies are in the jar minted a logged-out cookie that
 * then replaced the login the server's pool still held; the start page cannot,
 * so the person can begin at once.
 */
import path from 'node:path';
import type { App } from 'electron';
import type { AppServices } from './services.ts';
import { Workspace } from '../workflow/workspace.ts';
import { DraftStore } from '../workflow/draft-store.ts';
import { validate, type ValidationDeps, type ValidationSession } from '../workflow/validation.ts';
import { installApplicationMenu } from '../shell/menu.ts';
import { start as startFrontDoor, type FrontDoorOptions } from '../front-door/cdp-front-door.ts';
import { HOME_URL } from '../tabs/constants.ts';

/** The services start-up uses. */
type Deps = Pick<
  AppServices,
  | 'electron'
  | 'appDir'
  | 'cdpPort'
  | 'relayToken'
  | 'library'
  | 'config'
  | 'socket'
  | 'persona'
  | 'workspace'
  | 'recorder'
  | 'shell'
  | 'cookies'
  | 'routines'
  | 'updater'
  | 'deepLinks'
  | 'tabs'
  | 'control'
  | 'validationTabs'
  | 'governance'
>;

/** What a validation run is started with: the draft, its options and where its events go. */
type ValidationRequest = Pick<ValidationDeps, 'draft' | 'options' | 'event'>;

/** The validation worker, built by electron-vite beside the main bundle (electron.vite.config.ts). */
const WORKER_BUNDLE = ['out', 'main', 'worker.js'];

/**
 * Registering in dev needs the interpreter and script path, or the OS
 * registers the wrong executable.
 */
function registerProtocolClient(app: Pick<App, 'setAsDefaultProtocolClient'>): void {
  if (process.defaultApp && process.argv.length > 1) {
    app.setAsDefaultProtocolClient('oya', process.execPath, [path.resolve(process.argv[1])]);
  } else {
    app.setAsDefaultProtocolClient('oya');
  }
}

/** The address the front door listens on: every interface in Docker, loopback otherwise. */
function frontDoorHost(env: NodeJS.ProcessEnv): string {
  return env.OYA_REMOTE_DEBUGGING_HOST || (env.OYA_DOCKER === 'true' ? '0.0.0.0' : '127.0.0.1');
}

/** The draft store in `folder` of the user-data folder, encrypted with the keychain. */
function draftStore({ app, safeStorage }: Pick<Deps['electron'], 'app' | 'safeStorage'>, folder: string): DraftStore {
  return new DraftStore(path.join(app.getPath('userData'), folder), safeStorage);
}

/** Everything that runs once Electron is ready, in order. */
export class Boot {
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Settings and persona, the session, the workspace, then the window and the services, then any waiting links. */
  async run(): Promise<void> {
    this.settings();
    await this.deps.persona.setupBrowserSession();
    this.workspace();
    this.services();
    await this.deps.deepLinks.drain(process.argv);
  }

  /**
   * Skips the welcome screen for a desktop that has signed in before. A governed
   * browser, or one never accepted (a fresh cloud sandbox), waits for the server:
   * it must not load a page before its rules or persona.
   */
  resumeSignedIn(): void {
    if (!this.deps.config.values.apiKey || !this.deps.persona.active || this.deps.governance.configuration) return;
    this.deps.tabs.enterBrowsingMode(HOME_URL);
  }

  /** Menu, settings, protocol registration and the saved persona. */
  private settings(): void {
    const { app } = this.deps.electron;
    installApplicationMenu(this.deps);
    this.deps.config.load();
    // Provisioned sandboxes are given their id up front so the server can
    // correlate the sandbox it created with the browser that enrolls.
    if (process.env.OYA_BROWSER_ID) this.deps.socket.browserId = process.env.OYA_BROWSER_ID;
    registerProtocolClient(app);
    this.deps.persona.loadActive(app.getPath('userData'));
  }

  /** The workspace, and the recording it was left with. */
  private workspace(): void {
    const workspace = this.createWorkspace();
    this.deps.workspace = workspace;
    this.deps.recorder.adopt(workspace.draft.steps, workspace.draft.secrets);
  }

  /** The workflow workspace, with its drafts and runs encrypted on disk. */
  private createWorkspace(): Workspace {
    return new Workspace({
      store: draftStore(this.deps.electron, 'workflow-drafts'),
      runStore: draftStore(this.deps.electron, 'workflow-runs'),
      notify: (state) => this.deps.shell.send('workspace-state', state),
      runner: (draft, options, event) => this.runValidation({ draft, options, event }),
    });
  }

  /** Runs a draft against the real site (src/main/workflow/validation.ts) in the built worker. */
  private runValidation(run: ValidationRequest): Promise<ValidationSession> {
    const { app, utilityProcess } = this.deps.electron;
    const workerPath = path.join(this.deps.appDir, ...WORKER_BUNDLE);
    return validate({ ...run, app, utilityProcess, workerPath, startFrontDoor, ...this.validationHooks() });
  }

  /** How a validation run and the front door list, open and close the desktop's tabs. */
  private tabHooks(): Pick<FrontDoorOptions, 'tabs' | 'createTab' | 'closeTab'> {
    const tabs = this.deps.tabs;
    return {
      tabs: () => tabs.list,
      // Entering browsing mode opens the tab and makes it the active one, so there is always an id.
      createTab: (url) => tabs.openForAutomation(url)!,
      closeTab: (id, options) => tabs.closeTab(id, options),
    };
  }

  /** The validation run's tab hooks: the front door's, with createTab's automation flag dropped. */
  private validationTabHooks(): Pick<ValidationDeps, 'tabs' | 'createTab' | 'closeTab'> {
    const { tabs, createTab } = this.tabHooks();
    return {
      tabs,
      createTab: (url) => createTab(url, true),
      closeTab: (id, options) => this.deps.tabs.closeTab(id, options),
    };
  }

  /** What a validation run may do with the desktop: its tabs, the control gate and Chromium's own port. */
  private validationHooks(): Pick<
    ValidationDeps,
    'control' | 'tabs' | 'createTab' | 'closeTab' | 'leftOpen' | 'cdpPort'
  > {
    const deps = this.deps;
    // The last run's tabs stay open to show where it ended, until the next run starts.
    const leftOpen = (deps.validationTabs ??= new Set());
    const cdpPort = deps.cdpPort ? deps.cdpPort + 1 : 0;
    return { control: deps.control, ...this.validationTabHooks(), leftOpen, cdpPort };
  }

  /** The CDP front door's view of the tabs and the control gate. */
  private frontDoorOptions(): FrontDoorOptions {
    return { ...this.frontDoorAddress(), ...this.tabHooks(), ...this.gateHooks() };
  }

  /** Where the front door listens, Chromium's own port one up, and the relay's secret. */
  private frontDoorAddress(): Pick<FrontDoorOptions, 'port' | 'upstream' | 'host' | 'relayToken'> {
    const { cdpPort, relayToken } = this.deps;
    return { port: cdpPort, upstream: cdpPort + 1, host: frontDoorHost(process.env), relayToken };
  }

  /** How the front door takes the automation gate and counts its clients. */
  private gateHooks(): Pick<FrontDoorOptions, 'beginCommand' | 'clientChanged'> {
    const control = this.deps.control;
    return { beginCommand: () => control.beginLocalCommand(), clientChanged: (delta) => control.localClient(delta) };
  }

  /** The window, the front door, cookie sync, the connection, routines and updates. */
  private services(): void {
    this.deps.shell.create();
    if (this.deps.cdpPort) startFrontDoor(this.frontDoorOptions());
    this.deps.cookies.startCookieChangeListener();
    if (this.deps.config.values.apiKey || process.env.OYA_AUTO_CONNECT === 'true') this.deps.socket.connect();
    this.resumeSignedIn();
    this.deps.routines.start();
    this.deps.updater.start();
  }
}
