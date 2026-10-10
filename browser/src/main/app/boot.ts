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
import type { ValidationSession } from '../workflow/validation.ts';
import { validateNative, type NativeValidationDeps } from '../workflow/native-validation.ts';
import { installApplicationMenu } from '../shell/menu.ts';
import { HOME_URL } from '../tabs/constants.ts';

/** The services start-up uses. */
type Deps = Pick<
  AppServices,
  | 'electron'
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
  | 'actions'
  | 'nativeBrowsing'
>;

/** What a validation run is started with: the draft, its options and where its events go. */
type ValidationRequest = Pick<NativeValidationDeps, 'draft' | 'options' | 'event'>;

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

  /** Execute supported recorded steps through the existing authorized native page driver. */
  private runValidation(run: ValidationRequest): Promise<ValidationSession> {
    const deps = this.deps;
    if (!deps.nativeBrowsing)
      throw Error('Native workflow validation requires native browsing; legacy persona lifecycle is unsupported');
    return validateNative({ ...run, driver: deps.actions, control: deps.control, ...this.validationHooks() });
  }

  /** Fresh validation tabs retain the normal protection path and remain visible after completion. */
  private validationHooks(): Pick<NativeValidationDeps, 'tabs' | 'createTab' | 'closeTab' | 'leftOpen'> {
    const deps = this.deps;
    return {
      tabs: () => deps.tabs.list,
      createTab: (url) => deps.tabs.openForAutomation(url)!,
      closeTab: (id) => deps.tabs.closeTab(id),
      leftOpen: (deps.validationTabs ??= new Set()),
    };
  }

  /** The window, cookie sync, the connection, routines and updates. */
  private services(): void {
    this.deps.shell.create();
    this.deps.cookies.startCookieChangeListener();
    if (this.deps.config.values.apiKey || process.env.OYA_AUTO_CONNECT === 'true') this.deps.socket.connect();
    this.resumeSignedIn();
    this.deps.routines.start();
    this.deps.updater.start();
  }
}
