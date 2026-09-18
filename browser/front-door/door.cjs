/**
 * The front door's view of the browser: which targets harnesses may see, and
 * how a tab is opened for one. Every tab goes through the app's createTab, so it
 * takes the protected path (fingerprint, proxy, persona partition).
 */
const { FRONT_DOOR_TAB_WAIT_MS } = require('../constants.cjs');

/** The browser's own UI pages (the shell and the input shield): never an agent target. */
const isUi = (info) =>
  info?.type === 'page' && /^file:.*\/renderer\/(?:index|control-shield)\.html/.test(info.url || '');

/** The targets behind one front door and the app hooks that open, close and admit them. */
class FrontDoor {
  /** Target ids a harness must never see. */
  hidden = new Set();

  /** The options `start` was given; `up` is Chromium's own endpoint. */
  constructor(options) {
    Object.assign(this, options);
    this.beginCommand ??= () => () => {};
    this.clientChanged ??= () => {};
    /** Chromium's own debug endpoint, loopback only. */
    this.up = `127.0.0.1:${options.upstream}`;
  }

  /** Whether a target is off limits: the UI, or (in a validation run) outside the run. */
  blocked(info, id) {
    return isUi(info) || (!!this.allowedTarget && !this.allowedTarget(id));
  }

  /** Whether a target seen over CDP (a TargetInfo) is hidden. */
  isHidden(info) {
    return this.hidden.has(info.targetId) || this.blocked(info, info.targetId);
  }

  /** Chromium's target list, after marking the ones to hide. */
  async refreshHidden() {
    const list = await (await fetch(`http://${this.up}/json/list`)).json();
    for (const t of list) if (this.blocked(t, t.id)) this.hidden.add(t.id);
    return list;
  }

  /** refreshHidden, as true when it worked and false when Chromium did not answer. */
  tryRefresh() {
    return this.refreshHidden().then(
      () => true,
      () => false,
    );
  }

  /** A tab's targetId, asked of its own debugger (setupTabCDP attaches it). */
  async targetIdOf(tab) {
    if (!tab.targetId) {
      const { targetInfo } = await tab.view.webContents.debugger.sendCommand('Target.getTargetInfo');
      tab.targetId = targetInfo.targetId;
    }
    return tab.targetId;
  }

  /** Opens a tab the protected way and returns its targetId; a first load that never settles is not waited out. */
  async openTab(url) {
    const id = this.createTab(url || 'about:blank', true);
    const tab = this.tabs().find((t) => t.id === id);
    let timer;
    const waited = new Promise((r) => (timer = setTimeout(r, FRONT_DOOR_TAB_WAIT_MS)));
    await Promise.race([tab.ready?.catch(() => {}), waited]);
    clearTimeout(timer);
    return this.targetIdOf(tab);
  }

  /** Runs `work` inside one admitted automation command. */
  async admitted(work) {
    const finish = await this.beginCommand();
    try {
      return await work();
    } finally {
      finish();
    }
  }
}

module.exports = { FrontDoor, isUi };
