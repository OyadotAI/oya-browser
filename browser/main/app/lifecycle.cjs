/**
 * Start-up and shut-down: what runs once Electron is ready, in order, and
 * what must finish before the app quits.
 */
const path = require('path');
const { Workspace } = require('../../scripts/workspace.cjs');
const { DraftStore } = require('../../scripts/draft-store.cjs');
const { installApplicationMenu } = require('../shell/menu.cjs');
const { resumeSignedIn } = require('./resume.cjs');

/**
 * Registering in dev needs the interpreter and script path, or the OS
 * registers the wrong executable.
 */
function registerProtocolClient(app) {
  if (process.defaultApp && process.argv.length > 1) {
    app.setAsDefaultProtocolClient('oya', process.execPath, [path.resolve(process.argv[1])]);
  } else {
    app.setAsDefaultProtocolClient('oya');
  }
}

/** The workflow workspace, with its drafts and runs encrypted on disk. */
function createWorkspace(ctx) {
  const { app, safeStorage } = ctx.electron;
  const userData = app.getPath('userData');
  return new Workspace({
    store: new DraftStore(path.join(userData, 'workflow-drafts'), safeStorage),
    runStore: new DraftStore(path.join(userData, 'workflow-runs'), safeStorage),
    notify: (state) => ctx.shell.send('workspace-state', state),
    runner: (draft, options, event) => runValidation(ctx, { draft, options, event }),
  });
}

/** Runs a draft against the real site (scripts/validation.cjs, loaded on first use). */
function runValidation(ctx, run) {
  const { app, utilityProcess } = ctx.electron;
  return require('../../scripts/validation.cjs').validate({ ...run, app, utilityProcess, ...validationHooks(ctx) });
}

/** What a validation run may do with the desktop's tabs. */
function validationHooks(ctx) {
  return {
    control: ctx.control,
    tabs: () => ctx.tabs.list,
    createTab: (url) => ctx.tabs.openForAutomation(url),
    closeTab: (id, options) => ctx.tabs.closeTab(id, options),
    // The last run's tabs stay open to show where it ended, until the next run starts.
    leftOpen: (ctx.validationTabs ??= new Set()),
    cdpPort: ctx.cdpPort ? ctx.cdpPort + 1 : 0,
  };
}

/** The CDP front door's view of the tabs and the control gate. */
function frontDoorOptions(ctx) {
  const env = process.env;
  return {
    port: ctx.cdpPort,
    upstream: ctx.cdpPort + 1,
    host: env.OYA_REMOTE_DEBUGGING_HOST || (env.OYA_DOCKER === 'true' ? '0.0.0.0' : '127.0.0.1'),
    relayToken: ctx.relayToken,
    ...frontDoorHooks(ctx),
  };
}

/** How the front door opens and closes tabs and takes the automation gate. */
function frontDoorHooks(ctx) {
  return {
    tabs: () => ctx.tabs.list,
    createTab: (url) => ctx.tabs.openForAutomation(url),
    closeTab: (id, options) => ctx.tabs.closeTab(id, options),
    beginCommand: () => ctx.control.beginLocalCommand(),
    clientChanged: (delta) => ctx.control.localClient(delta),
  };
}

/** Menu, settings, protocol registration and the saved persona. */
function bootSettings(ctx) {
  const { app } = ctx.electron;
  installApplicationMenu(ctx);
  ctx.config.load();
  // Provisioned sandboxes are given their id up front so the server can
  // correlate the sandbox it created with the browser that enrolls.
  if (process.env.OYA_BROWSER_ID) ctx.socket.browserId = process.env.OYA_BROWSER_ID;
  registerProtocolClient(app);
  ctx.persona.loadActive(app.getPath('userData'));
}

/** The workspace, and the recording it was left with. */
function bootWorkspace(ctx) {
  ctx.workspace = createWorkspace(ctx);
  ctx.recorder.adopt(ctx.workspace.draft.steps, ctx.workspace.draft.secrets);
}

/** The window, the front door, cookie sync, the connection, routines and updates. */
function bootServices(ctx) {
  ctx.shell.create();
  if (ctx.cdpPort) require('../../cdp-front-door').start(frontDoorOptions(ctx));
  ctx.cookies.startCookieChangeListener();
  if (ctx.config.values.apiKey || process.env.OYA_AUTO_CONNECT === 'true') ctx.socket.connect();
  resumeSignedIn(ctx);
  ctx.routines.start();
  ctx.startAutoUpdate();
}

/** Everything that runs once Electron is ready, in order. */
async function bootBrowser(ctx) {
  bootSettings(ctx);
  await ctx.persona.setupBrowserSession();
  bootWorkspace(ctx);
  bootServices(ctx);
  await ctx.deepLinks.drain(process.argv);
}

/** Marks a running validation interrupted, and ends its session. */
function interruptValidation(workspace) {
  const error = 'Oya closed during validation. Check the website before retrying.';
  workspace.receive({ type: 'finished', status: 'interrupted', error });
  workspace.session?.dispose();
}

/** Quitting: a recording is finished first, a validation is marked interrupted, the jar is written to disk. */
class Lifecycle {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** Set once a quit has waited for a recording, so the second quit goes through. */
    this.finishingQuit = false;
    /** Set once the jar has been written to disk for this quit, so the next quit goes through. */
    this.jarFlushed = false;
  }

  /** Writes the jar to disk once; later calls answer at once. The updater calls it before restarting. */
  flushJar() {
    if (this.jarFlushed) return Promise.resolve();
    this.jarFlushed = true;
    return this.ctx.persona.flushJar();
  }

  /** Installs the quit handlers. */
  install() {
    const { app } = this.ctx.electron;
    app.on('window-all-closed', () => {
      // A login made seconds ago is still queued: send it while the socket is up.
      this.ctx.cookies.flushCookieChanges();
      this.ctx.socket.disconnect();
      app.quit();
    });
    app.on('before-quit', (event) => this.beforeQuit(event));
  }

  /** Runs before the app quits. */
  beforeQuit(event) {
    if (this.ctx.recorder.recording && !this.finishingQuit) return this.finishRecordingFirst(event);
    if (this.ctx.workspace?.busy()) interruptValidation(this.ctx.workspace);
    this.ctx.cookies.flushCookieChanges();
    this.ctx.layout.flush();
    if (!this.jarFlushed) this.holdForJar(event);
  }

  /** Holds the quit until the cookies and storage are on disk, then quits again. */
  holdForJar(event) {
    event.preventDefault();
    const quit = () => this.ctx.electron.app.quit();
    // A jar that cannot be written must not keep the app from quitting.
    this.flushJar().then(quit, quit);
  }

  /** Holds the quit until the recording is stopped and saved. */
  finishRecordingFirst(event) {
    event.preventDefault();
    this.finishingQuit = true;
    const recorder = this.ctx.recorder;
    recorder.queueRecording(() => recorder.stopRecording()).finally(() => this.ctx.electron.app.quit());
  }
}

module.exports = { bootBrowser, Lifecycle };
