/**
 * A main-process context like src/main/main.ts builds, for unit tests of the
 * services in src/main/ (app, shell, tabs, recording, connection, ipc). Electron is faked (windows, views, menus, dialogs), the socket and
 * the control state are recorders, and `with` swaps in real services so a
 * test drives the class it is about through its neighbours.
 */
const path = require('node:path');
const { EventEmitter } = require('node:events');
const { FakeWebContents } = require('./fakes.cjs');
const { DesktopDialogs } = require('../../../src/main/dialogs/index.ts');
const { Governance } = require('../../../src/main/identity/governance.ts');
const { AgentTab } = require('../../../src/main/control/agent-tab.ts');
const { IncognitoSession } = require('../../../src/main/incognito/index.ts');
const { SitePermissionStore } = require('../../../src/main/app/site-permissions.ts');
const { MediaPermissions } = require('../../../src/main/app/media-permissions.ts');
const { CallGuard } = require('../../../src/main/app/call-guard.ts');
const { Downloads } = require('../../../src/main/downloads/index.ts');

/** A JSON file that lives in memory: what the on-disk stores are tested over. */
function memoryFile(value = null) {
  return {
    read: () => value,
    write: (next) => void (value = JSON.parse(JSON.stringify(next))),
  };
}

/** A tab's webContents: loads, history and handlers a BrowserView's page has. */
class FakePageContents extends FakeWebContents {
  /** `url` is the first address. */
  constructor(options) {
    super(options);
    /** Addresses loaded, in order. */
    this.loaded = [];
    /** Whether a load is in progress. */
    this.loading = false;
    /** The window.open handler, once set. */
    this.openHandler = null;
    /** The options each loadURL was given, when it was given any. */
    this.loadOptions = [];
    /** What loadURL does; resolves by default. */
    this.loadImpl = async () => {};
    /** Back/forward state. */
    this.navigationHistory = { canGoBack: () => false, canGoForward: () => false };
    /** Calls to stop, reload, goBack, goForward, focus. */
    this.calls = [];
  }

  /** Records the address (and any load options) and runs loadImpl. */
  loadURL(url, options) {
    this.loaded.push(url);
    if (options) this.loadOptions.push(options);
    return this.loadImpl(url);
  }

  /** As Electron's. */
  isLoading() {
    return this.loading;
  }

  /** As Electron's. */
  setWindowOpenHandler(fn) {
    this.openHandler = fn;
  }

  /** Records a call by name. */
  record(name) {
    this.calls.push(name);
  }

  /** As Electron's. */
  stop() {
    this.record('stop');
  }

  /** As Electron's. */
  reload() {
    this.record('reload');
  }

  /** As Electron's: runs code in an isolated world; `probeImpl` answers (a hung page never does). */
  executeJavaScriptInIsolatedWorld() {
    this.record('probe');
    return this.probeImpl ? this.probeImpl() : Promise.resolve(1);
  }

  /** As Electron's: whether the renderer died (`crashed`, set by a test). */
  isCrashed() {
    return !!this.crashed;
  }

  /** As Electron's: ends the renderer. */
  forcefullyCrashRenderer() {
    this.record('crash');
  }

  /** As Electron's: the renderer's OS process id (`pid`, 0 by default). */
  getOSProcessId() {
    return this.pid || 0;
  }

  /** As Electron's. */
  goBack() {
    this.record('goBack');
  }

  /** As Electron's. */
  goForward() {
    this.record('goForward');
  }

  /** As Electron's. */
  focus() {
    this.record('focus');
  }

  /** As Electron's. */
  isFocused() {
    return false;
  }

  /** As Electron's: 1 unless a test sets `zoomFactor`. */
  getZoomFactor() {
    return this.zoomFactor ?? 1;
  }

  /** As Electron's. */
  loadFile(file) {
    this.loaded.push(file);
    return Promise.resolve();
  }

  /** As Electron's. */
  async executeJavaScript() {}

  /** A screenshot stand-in. */
  async capturePage() {
    return { toDataURL: () => 'data:image/png;base64,AA' };
  }
}

/** A BrowserView. */
class FakeBrowserView {
  /** `options` are what the constructor was given. */
  constructor(options = {}) {
    /** Construction options. */
    this.options = options;
    /** Its page, in the app-owned session it was given (an agent context or incognito), as Electron's. */
    this.webContents = new FakePageContents({});
    if (options.webPreferences?.session) this.webContents.session = options.webPreferences.session;
    /** Last bounds set. */
    this.bounds = { x: 0, y: 0, width: 0, height: 0 };
    /** Last background colour. */
    this.background = null;
  }

  /** As Electron's. */
  setBounds(bounds) {
    this.bounds = bounds;
  }

  /** As Electron's. */
  getBounds() {
    return this.bounds;
  }

  /** As Electron's. */
  setBackgroundColor(colour) {
    this.background = colour;
  }
}

/** The shell's BrowserWindow: which views it shows, and what it was told. */
class FakeWindow extends EventEmitter {
  /** An open window; `options` are what it was built with. */
  constructor(options = {}) {
    super();
    /** The construction options. */
    this.options = options;
    /** What getNormalBounds() answers; a test sets it. */
    this.normalBounds = { x: 0, y: 0, width: 1280, height: 860 };
    /** Set by a test, as the person going fullscreen would. */
    this.fullScreen = false;
    /** Set by a test, as the person minimizing to the Dock would. */
    this.minimized = false;
    /** The shell page. */
    this.webContents = new FakePageContents({});
    this.webContents.mainFrame = {};
    /** Views on the window, bottom to top. */
    this.views = [];
    /** Set by destroy(). */
    this.destroyed = false;
  }

  /** As Electron's. */
  isDestroyed() {
    return this.destroyed;
  }

  /** As Electron's. */
  getContentBounds() {
    return { x: 0, y: 0, width: 1280, height: 800 };
  }

  /**
   * As Electron 44's, bug included: it removes the views while walking the list it
   * removes from, so every other one stays mounted.
   */
  setBrowserView(view) {
    this.views.forEach((v) => this.removeBrowserView(v));
    if (view) this.addBrowserView(view);
  }

  /** As Electron's. */
  addBrowserView(view) {
    this.views.push(view);
  }

  /** As Electron's: in place. */
  removeBrowserView(view) {
    const i = this.views.indexOf(view);
    if (i >= 0) this.views.splice(i, 1);
  }

  /** As Electron's. */
  getBrowserViews() {
    return [...this.views];
  }

  /** Moves `view` to the top. */
  setTopBrowserView(view) {
    this.removeBrowserView(view);
    this.views.push(view);
  }

  /** As Electron's. */
  setBackgroundColor(colour) {
    this.background = colour;
  }

  /** As Electron's. */
  show() {
    this.shown = true;
  }

  /** Records the shell page it was given. */
  loadFile(file) {
    this.file = file;
    return Promise.resolve();
  }

  /** As Electron's. */
  maximize() {
    this.maximized = true;
  }

  /** As Electron's. */
  isMaximized() {
    return !!this.maximized;
  }

  /** As Electron's. */
  isFullScreen() {
    return this.fullScreen;
  }

  /** As Electron's. */
  isMinimized() {
    return this.minimized;
  }

  /** As Electron's: the bounds before maximizing. */
  getNormalBounds() {
    return this.normalBounds;
  }
}

/** The Electron module, faked: views, menus, dialogs, sessions, IPC. */
function fakeElectron() {
  const menuItems = new Map();
  const dialogAnswers = { messageBox: [], saveDialog: [] };
  return {
    BrowserView: FakeBrowserView,
    BrowserWindow: FakeWindow,
    Menu: {
      getApplicationMenu: () => ({ getMenuItemById: (id) => menuItems.get(id) }),
      buildFromTemplate: (template) => ({
        template,
        popup(options) {
          this.popped = options;
        },
      }),
      setApplicationMenu(menu) {
        this.installed = menu;
      },
      items: menuItems,
    },
    nativeTheme: Object.assign(new EventEmitter(), { shouldUseDarkColors: false }),
    screen: { getAllDisplays: () => [{ workArea: { x: 0, y: 0, width: 1920, height: 1080 } }] },
    autoUpdater: new EventEmitter(),
    shell: {
      opened: [],
      /** Records the link instead of handing it to the operating system. */
      async openExternal(url) {
        this.opened.push(url);
      },
      /** Records the file instead of opening it. */
      async openPath(file) {
        this.opened.push(file);
        return '';
      },
      /** Records the file instead of showing its folder. */
      showItemInFolder(file) {
        this.opened.push(`folder:${file}`);
      },
    },
    dialog: {
      answers: dialogAnswers,
      asked: [],
      async showMessageBox(...args) {
        this.asked.push(args);
        return dialogAnswers.messageBox.shift() || { response: 0 };
      },
      async showSaveDialog(...args) {
        this.asked.push(args);
        return dialogAnswers.saveDialog.shift() || { canceled: true };
      },
    },
    ipcMain: {
      handlers: new Map(),
      handle(channel, fn) {
        this.handlers.set(channel, fn);
      },
    },
    webContents: { getAllWebContents: () => [] },
    session: {
      fromPartition: (name) => ({ name, on() {}, cookies: { flushStore: async () => {} }, flushStorageData() {} }),
    },
    app: Object.assign(new EventEmitter(), {
      quit() {
        this.quitted = true;
      },
      getVersion: () => '1.0.0',
      setAboutPanelOptions() {},
    }),
  };
}

/** A control state that is interactive unless told otherwise. */
function fakeControl(state = { interactive: true, mode: 'human', mine: true }) {
  return {
    state,
    changes: [],
    snapshot() {
      return { ...this.state };
    },
    async change(action) {
      this.changes.push(action);
      return this.snapshot();
    },
    connect(value) {
      this.connected = value;
    },
    disconnect() {
      this.disconnected = (this.disconnected || 0) + 1;
    },
    receive(value) {
      this.received = value;
    },
    result(message) {
      this.results = [...(this.results || []), message];
    },
  };
}

/** A control socket that records what it sends. */
function fakeSocket() {
  return {
    sent: [],
    open: true,
    ready: true,
    browserId: 'b1',
    isOpen() {
      return this.open;
    },
    send(payload) {
      this.sent.push(payload);
      return this.open;
    },
    ofType(type) {
      return this.sent.filter((m) => m.type === type);
    },
    connect() {
      this.connects = (this.connects || 0) + 1;
    },
    disconnect() {
      this.disconnects = (this.disconnects || 0) + 1;
    },
    heard() {
      this.heardCount = (this.heardCount || 0) + 1;
    },
    startPingLoop() {
      this.pinging = true;
    },
    sendStatus() {
      this.statusSent = true;
    },
  };
}

/** A shell window stand-in that records what the renderer is sent. */
function fakeShell() {
  return {
    window: new FakeWindow(),
    browsingMode: true,
    sent: [],
    logs: [],
    alive() {
      return !!this.window && !this.window.isDestroyed();
    },
    send(channel, data) {
      this.sent.push({ channel, data });
    },
    sentOn(channel) {
      return this.sent.filter((m) => m.channel === channel).map((m) => m.data);
    },
    devLog(...entry) {
      this.logs.push(entry);
    },
    background: () => '#fff',
  };
}

/** The main-process context with every service faked; `real` maps a name to a class to build for real. */
function mainCtx(real = {}) {
  const ctx = {
    library: {
      visit() {},
      snapshot() {
        return { history: [], bookmarks: [] };
      },
      toggle() {},
      clearHistory() {},
    },
    electron: fakeElectron(),
    appDir: path.join(__dirname, '..', '..', '..'),
    isolatedWorld: 'w-test',
    analyzerScript: 'analyzer(__OYA_ATTR__, __OYA_RECORD__)',
    cdpPort: 0,
    workspace: null,
    config: {
      values: {},
      saves: 0,
      save() {
        this.saves++;
      },
      merge(c) {
        this.values = { ...this.values, ...c };
      },
    },
    shell: fakeShell(),
    control: fakeControl(),
    socket: fakeSocket(),
    shortcuts: { install() {} },
    shield: {
      sync() {},
      prepare() {},
      requireHumanControl() {},
      keepFocusOnShell() {},
      adoptPopup() {},
      takeover: {
        asked: [],
        /** Records the handoff it was asked to offer. */
        async request(options) {
          this.asked.push(options?.message);
        },
      },
      controlChanged() {},
      runEnded() {
        this.ended = (this.ended || 0) + 1;
      },
    },
    layout: { open: false, layoutActiveTab() {}, reveal() {}, closeAll() {}, flush() {}, inherit() {}, openFor() {} },
    overlays: { names: new Set() },
    findBar: { open() {}, search() {}, step() {}, close() {}, dismiss() {}, follow() {}, sync() {}, height: () => 0 },
    protection: {
      assertSession() {},
      configureSession() {},
      setupTabCDP: async () => true,
      resetTabCDP() {},
      injectScripts: async () => {},
      protectPopup() {},
    },
    dialogs: new DesktopDialogs(
      () => false,
      () => () => {},
    ),
    // Ungoverned, as a person's own browser is; a test about governance swaps in a configured one.
    externalApps: {
      wire() {},
      request() {
        return false;
      },
    },
    governance: new Governance(null),
    persona: {
      active: null,
      loginState: null,
      partitionName: () => 'persist:oya-browser',
      session() {
        return ctx.electron.session.fromPartition(this.partitionName());
      },
      /** As the real one: the jar's cookies and storage to disk. */
      async flushStorage() {
        return true;
      },
      async flushJar() {
        const session = this.session();
        await Promise.allSettled([session.cookies.flushStore(), session.flushStorageData()]);
      },
    },
    recorder: {
      recording: false,
      recordNavigation() {},
      joinIfRecording() {},
      controlLost() {},
      forgotten: [],
      channels: {
        /** Records the view a closed tab left. */
        forget(view) {
          ctx.recorder.forgotten.push(view);
        },
      },
    },
    cookies: { pullCookiesFor: async () => {}, flushCookieChanges() {} },
    routines: { start() {}, refresh() {} },
    mirror: { maybeRun() {}, onOk() {}, onFailed() {}, reimport() {} },
    onboarding: { active: false, status() {}, finish() {}, connect: async () => ({ connected: true, email: '' }) },
    chatgpt: {
      status: () => ({ connected: false, email: '' }),
      accessToken: async () => 'token',
      signIn: async () => ({ connected: true, email: '' }),
      signOut() {},
    },
    // Each tab's chat in flight.
    chatRuns: new Map(),
    // No tabs until a test gives some.
    tabs: { list: [], activeTabId: null, getActiveView: () => null, getShownView: () => null, find: () => undefined },
    // The real store over a file in memory, so grants are per partition as in the app.
    sitePermissions: new SitePermissionStore(memoryFile()),
    // Records what the session store was asked; no file.
    sessionTabs: {
      calls: [],
      changed() {},
      flush() {
        this.calls.push('flush');
      },
      restore() {
        this.calls.push('restore');
      },
    },
  };
  // The real claim: it reads the faked control and tabs, so control fakes keep their old meaning.
  ctx.agentTab = new AgentTab(ctx);
  // The real device gate and call guard, on macOS where the guard asks; they read ctx when asked, so swaps still apply.
  ctx.mediaPermissions = new MediaPermissions(ctx, 'darwin');
  ctx.callGuard = new CallGuard(ctx, 'darwin');
  ctx.downloads = new Downloads(ctx, memoryFile());
  ctx.incognito = new IncognitoSession(ctx);
  for (const [name, Service] of Object.entries(real)) ctx[name] = new Service(ctx);
  return ctx;
}

module.exports = {
  mainCtx,
  memoryFile,
  fakeElectron,
  fakeControl,
  fakeSocket,
  fakeShell,
  FakeWindow,
  FakeBrowserView,
  FakePageContents,
};
