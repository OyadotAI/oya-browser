/**
 * Validation: runs a draft's generated Playwright module against fresh tabs of
 * this browser. An isolated worker drives only fresh validation tabs, using the
 * installed Chromium, through a run-scoped CDP front door that exposes those
 * tabs and nothing else.
 */
const { randomBytes } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { once } = require('node:events');
const { start } = require('../cdp-front-door');
const { VALIDATION } = require('./constants.cjs');
const { withinTime } = require('./within-time.cjs');

/** The address of a validation tab named `name`. */
const tabUrl = (name) => 'about:blank#oya-run-' + (name === 'main' ? 'main' : encodeURIComponent(name));

/** Resolves after `ms`. */
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Reads Chromium's ephemeral debugging port from its profile, if it has written it yet. */
function readDebugPort(app) {
  try {
    return Number(fs.readFileSync(path.join(app.getPath('userData'), 'DevToolsActivePort'), 'utf8').split('\n')[0]);
  } catch {
    return undefined;
  }
}

/** One validation run: its tabs, front door, worker and cleanup. */
class ValidationRun {
  /** `deps` are validate()'s arguments: the draft, options, event sink and the browser's hooks. */
  constructor(deps) {
    Object.assign(this, deps);
    /** The secret the worker presents to the front door. */
    this.token = randomBytes(VALIDATION.TOKEN_BYTES).toString('hex');
    /** CDP target ids the worker may reach. */
    this.targets = new Set();
    /** Tab ids opened for this run. */
    this.runTabs = new Set();
    /** Set once cleanup has run. */
    this.finished = false;
    /** Scratch folder for the generated module. */
    this.directory = fs.mkdtempSync(path.join(deps.app.getPath('temp'), 'oya-validation-'));
  }

  /** Stops everything the run started, once. */
  cleanup() {
    if (this.finished) return;
    this.finished = true;
    clearTimeout(this.stopTimer);
    this.server?.close();
    this.worker?.kill();
    fs.rmSync(this.directory, { recursive: true, force: true });
  }

  /**
   * Closes the tabs the last run left open to show where it ended, and the tabs
   * their pages opened, so each run does not add another tab.
   */
  closeLeftOpen() {
    const left = this.leftOpen;
    if (!left?.size) return;
    const stale = this.tabs().filter((t) => left.has(t.id) || left.has(t.openerId));
    left.clear();
    for (const tab of stale) this.closeTab(tab.id);
  }

  /** Opens a tab for this run and returns it. */
  addTab(url) {
    const id = this.createTab(url);
    this.runTabs.add(id);
    this.leftOpen?.add(id);
    return this.tabs().find((t) => t.id === id);
  }

  /** Records the tab's CDP target so the front door lets the worker reach it. */
  async registerTarget(tab) {
    const { targetInfo } = await tab.view.webContents.debugger.sendCommand('Target.getTargetInfo');
    tab.targetId = targetInfo.targetId;
    this.targets.add(tab.targetId);
  }

  /** Opens a run tab, waits for it, and registers its target. */
  async open(url) {
    const tab = this.addTab(url);
    const opened = Promise.resolve(tab.ready).then(() => this.registerTarget(tab));
    await withinTime(opened, VALIDATION.TAB_OPEN_MS, 'The validation tab did not open. Close some tabs and try again.');
    return tab;
  }

  /** Hands control back from a person, then opens the main tab and one per named tab the steps use. */
  async prepareTabs() {
    if (this.control.snapshot().mine || this.control.snapshot().mode === 'human') await this.control.change('return');
    const tab = await this.open(tabUrl('main'));
    const pageUrls = { main: tabUrl('main') };
    await this.openNamedTabs(pageUrls);
    return { tab, pageUrls };
  }

  /** Opens a tab for every other tab name the enabled steps use, recording each address. */
  async openNamedTabs(pageUrls) {
    for (const name of new Set(this.draft.steps.filter((step) => step.enabled).map((step) => step.tab))) {
      if (name === 'main') continue;
      pageUrls[name] = tabUrl(name);
      await this.open(pageUrls[name]);
    }
  }

  /** The browser's debugging port: given, or waited for until Chromium writes it. */
  async debugPort() {
    // Chromium writes the ephemeral debugging port to its profile directory.
    let upstream = this.cdpPort;
    for (let i = 0; !upstream && i < VALIDATION.PORT_POLLS; i++) {
      upstream = readDebugPort(this.app);
      if (!upstream) await sleep(VALIDATION.PORT_POLL_MS);
    }
    if (!upstream) throw new Error('The browser debugging endpoint did not start. Restart Oya Browser.');
    return upstream;
  }

  /** A tab the worker opens: its target is added once it is ready, before Playwright sees it. */
  createRunTab(url) {
    const tab = this.addTab(url);
    const ready = tab.ready;
    tab.ready = ready.then(() => this.registerTarget(tab));
    return tab.id;
  }

  /** Starts the run-scoped CDP front door and waits until it listens. */
  async startFrontDoor(upstream) {
    this.server = start({
      ...{ port: 0, upstream, host: '127.0.0.1', runToken: this.token, allowedTarget: (id) => this.targets.has(id) },
      tabs: () => this.tabs().filter((t) => this.runTabs.has(t.id)),
      // The proxy awaits tab.ready; add its target before exposing it to Playwright.
      createTab: (url) => this.createRunTab(url),
      closeTab: this.closeTab,
      ...{ beginCommand: () => this.control.beginLocalCommand(), clientChanged: (d) => this.control.localClient(d) },
    });
    await once(this.server, 'listening');
  }

  /** Relays a worker message, cleaning up once the run has finished. */
  relay(message) {
    if (this.finished) return;
    try {
      this.event(message);
    } finally {
      if (message.type === 'finished') this.cleanup();
    }
  }

  /** The worker exited before finishing: report the run as interrupted. */
  workerExited() {
    if (this.finished) return;
    this.event({
      type: 'finished',
      status: 'interrupted',
      error: 'Validation process exited. No step was automatically resubmitted.',
    });
    this.cleanup();
  }

  /** Forks the Playwright worker and wires its messages and exit. */
  forkWorker() {
    const options = { serviceName: 'Oya Playwright validation', stdio: 'pipe' };
    this.worker = this.utilityProcess.fork(path.join(__dirname, 'workflow-worker.cjs'), [], options);
    this.worker.on('message', (message) => this.relay(message));
    this.worker.on('exit', () => this.workerExited());
  }

  /** Tells the worker to start the run. */
  startWorker(tab, pageUrls) {
    const { options } = this;
    this.worker.postMessage({
      ...{ type: 'start', draft: this.draft, endpoint: `http://127.0.0.1:${this.server.address().port}` },
      ...{ token: this.token, targetId: tab.targetId, pageUrls, vars: options.vars || {}, directory: this.directory },
      ...{ command: options.command, runTo: options.runTo, evidence: !!options.evidence },
      autoHeal: options.autoHeal !== false,
      slowMo: Math.min(Math.max(Number(options.slowMo) || 0, 0), VALIDATION.MAX_SLOW_MO_MS),
    });
  }

  /** Passes a run control to the worker; a stop that gets no answer is reported after a grace period. */
  sendControl(command) {
    if (this.finished) return;
    this.worker.postMessage({ type: 'control', command });
    if (command === 'stop') this.stopTimer = setTimeout(() => this.stopExpired(), VALIDATION.STOP_GRACE_MS);
  }

  /** The worker did not stop in time: report the run as interrupted. */
  stopExpired() {
    this.event({
      type: 'finished',
      status: 'interrupted',
      error: 'Worker stopped. The last website action may have completed; check before retrying.',
    });
    this.cleanup();
  }

  /** Every step of starting the run, in order. */
  async begin() {
    this.closeLeftOpen();
    const { tab, pageUrls } = await this.prepareTabs();
    await this.startFrontDoor(await this.debugPort());
    this.forkWorker();
    this.startWorker(tab, pageUrls);
    return { control: (command) => this.sendControl(command), dispose: () => this.cleanup() };
  }
}

/** Starts validating a draft; returns `{ control(command), dispose() }`. Any failure cleans up and rethrows. */
async function validate(deps) {
  const run = new ValidationRun(deps);
  try {
    return await run.begin();
  } catch (error) {
    run.cleanup();
    throw error;
  }
}

module.exports = { validate };
