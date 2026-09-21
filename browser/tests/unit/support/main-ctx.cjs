/**
 * A main-process context like main.js builds, for unit tests of the services
 * in main/app, main/shell, main/tabs, main/recording, main/connection and
 * main/ipc. Electron is faked (windows, views, menus, dialogs), the socket and
 * the control state are recorders, and `with` swaps in real services so a
 * test drives the class it is about through its neighbours.
 */
const { EventEmitter } = require('node:events');
const { FakeWebContents } = require('./fakes.cjs');

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
    /** Its page. */
    this.webContents = new FakePageContents({});
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
  /** An open window. */
  constructor() {
    super();
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

  /** Replaces every view with `view`. */
  setBrowserView(view) {
    this.views = view ? [view] : [];
  }

  /** As Electron's. */
  addBrowserView(view) {
    this.views.push(view);
  }

  /** As Electron's. */
  removeBrowserView(view) {
    this.views = this.views.filter((v) => v !== view);
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
    shell: {
      opened: [],
      /** Records the link instead of handing it to the operating system. */
      async openExternal(url) {
        this.opened.push(url);
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
    session: { fromPartition: (name) => ({ name, cookies: { flushStore: async () => {} }, flushStorageData() {} }) },
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
    electron: fakeElectron(),
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
      controlChanged() {},
    },
    layout: { layoutActiveTab() {}, reveal() {}, flush() {} },
    overlays: { names: new Set() },
    protection: { setupTabCDP: async () => {}, injectScripts: async () => {}, protectPopup() {} },
    persona: { active: null, loginState: null, partitionName: () => 'persist:oya-browser' },
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
    cookies: { pullCookiesFor: async () => {} },
    mirror: { maybeRun() {}, onOk() {}, onFailed() {}, reimport() {} },
  };
  for (const [name, Service] of Object.entries(real)) ctx[name] = new Service(ctx);
  return ctx;
}

module.exports = {
  mainCtx,
  fakeElectron,
  fakeControl,
  fakeSocket,
  fakeShell,
  FakeWindow,
  FakeBrowserView,
  FakePageContents,
};
