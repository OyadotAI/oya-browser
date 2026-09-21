/**
 * One-click sign-in (oya:// deep links).
 *
 * The dashboard hands out `oya://connect?key=...&server=...` so a customer can
 * go from "pick Oya Browsers" to a signed-in desktop browser without copying a
 * key by hand. Cookies gathered here are what the remote browsers reuse, so
 * this is the step that makes an agent arrive already logged in.
 */
const { pairFromLink } = require('../pairing.cjs');
const { LINK_SCHEME } = require('./constants.cjs');

/** Logs a link that could not be applied. */
const reportLinkFailure = (e) => console.error('[deeplink]', e.message);

/** Receives oya:// links from every route the OS delivers them by. */
class DeepLinks {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** Links that arrived before the window existed. */
    this.pending = [];
  }

  /** Pair from an oya:// link (see main/pairing.cjs), then reconnect as the paired browser. */
  async applyDeepLink(rawUrl) {
    const paired = await pairFromLink(rawUrl, (opts) => this.ask(opts));
    if (!paired) return false;
    this.retarget(paired);
    // connect() emits ws-status, which is how the renderer learns about this.
    this.ctx.socket.connect();
    this.ctx.shell.window?.show();
    return true;
  }

  /** showMessageBox refuses a null parent, and second-instance can arrive before the window exists. */
  ask(opts) {
    const { dialog } = this.ctx.electron;
    const win = this.ctx.shell.window;
    return win ? dialog.showMessageBox(win, opts) : dialog.showMessageBox(opts);
  }

  /** Points the saved config at the paired server and drops the old connection. */
  retarget({ apiKey, persona, serverUrl }) {
    const config = this.ctx.config.values;
    // A deep link can retarget this browser at a different project (each project
    // has its own key). The durable session id is scoped to a project, so reusing
    // the previous browserId makes the new project reject the socket as a foreign
    // id, close 4003, which the client treats as fatal and never retries, so the
    // desktop sits offline until a full restart happens to mint a fresh id. Rebind
    // to a new session on any key/server change; an unchanged target keeps its id.
    if (config.apiKey !== apiKey || config.serverUrl !== serverUrl) this.ctx.socket.browserId = null;
    this.ctx.socket.disconnect();
    Object.assign(config, { serverUrl, apiKey, persona });
    this.ctx.config.save();
  }

  /** Windows and Linux: the link is an argv entry of a second launch. */
  onSecondInstance(argv) {
    const link = argv.find((a) => a.startsWith(LINK_SCHEME));
    if (link) this.applyDeepLink(link).catch(reportLinkFailure);
    this.ctx.shell.window?.show();
  }

  /** macOS delivers it as an event, which can fire before the app is ready. */
  onOpenUrl(event, url) {
    event.preventDefault();
    if (this.ctx.shell.window) this.applyDeepLink(url).catch(reportLinkFailure);
    else this.pending.push(url);
  }

  /** Once the window exists: the links that were waiting, then any on the command line. */
  async drain(argv) {
    const queued = this.pending.splice(0).concat(argv.filter((a) => a.startsWith(LINK_SCHEME)));
    for (const link of queued) await this.applyDeepLink(link).catch(reportLinkFailure);
  }
}

module.exports = { DeepLinks };
