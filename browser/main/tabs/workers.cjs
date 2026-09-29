/**
 * The persona in the browser's service and shared workers. They belong to the
 * browser rather than to a tab, so no tab's debugger reaches them, and they read
 * the real machine: CreepJS found a page saying Apple M1 with 6 cores beside its
 * service worker saying Apple M4 with 10, and its hasBadWebGL check compares the
 * two. Covered here over Chromium's own browser-level endpoint (the loopback
 * remote-debugging port main.js always opens), with the applier the tabs use.
 */
const fs = require('fs');
const path = require('path');
const { CdpWs } = require('../mirror/cdp-ws.cjs');
const { createPersonaApplier } = require('../../anonymity/apply');

/** Chromium's debugging port as it wrote it into its profile, or null before it has. */
function debugPort(userData) {
  try {
    return Number(fs.readFileSync(path.join(userData, 'DevToolsActivePort'), 'utf8').split('\n')[0]) || null;
  } catch {
    return null;
  }
}

/** Said, never thrown: a worker left uncovered reads the real machine, and that should show in the log. */
const workerCoverageFailed = (what, err) => {
  console.error('[anonymity] worker ' + what + ' failed:', err?.message || err);
};

/** Keeps the active persona on every service and shared worker, over one browser-level connection. */
class WorkerCoverage {
  /** `ctx` is the main-process context; `userData()` is the folder Chromium writes its port into. */
  constructor(ctx, userData) {
    /** The main-process context. */
    this.ctx = ctx;
    /** Where DevToolsActivePort is, asked when coverage starts. */
    this.userData = userData;
    /** The browser-level connection, while covering. */
    this.cdp = null;
  }

  /** Covers the active persona's workers from now on, replacing earlier coverage. Never throws. */
  async cover() {
    this.stop();
    const persona = this.ctx.protection.personaOptions();
    const port = debugPort(this.userData());
    if (!persona || !port) return;
    await this.connect(port, persona).catch((e) => workerCoverageFailed('coverage', e));
  }

  /** Opens the connection and has Chromium hold each new worker until the persona is on it. */
  async connect(port, persona) {
    const cdp = new CdpWs(port);
    this.cdp = cdp;
    await cdp.connect();
    const send = (method, params, sessionId) => cdp.send(method, params, sessionId);
    const on = (event, fn) => cdp.on(event, fn);
    await createPersonaApplier({ send, on, ...persona, onError: workerCoverageFailed }).browser();
  }

  /** Ends coverage; Chromium resumes whatever the closed connection was holding. */
  stop() {
    this.cdp?.close();
    this.cdp = null;
  }
}

module.exports = { WorkerCoverage, debugPort };
