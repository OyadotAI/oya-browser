/**
 * Browser session registry, tracks connected browsers, scoped by API key.
 * Each key only sees its own browsers.
 */

import { EventEmitter } from 'events';
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { newRecord, type BrowserSpec } from './registry/record.ts';
import { applyActivity } from './registry/activity.ts';
import { rowOf } from './registry/row.ts';
import { sendFrame, endViewers } from './registry/viewers.ts';
import { closeOutbound } from './registry/teardown.ts';

export { summarise } from './registry/summary.ts';

/** Every browser this replica is connected to, with its activity; emits connect, disconnect and stream start/stop events. */
class ConnectionRegistry extends EventEmitter {
  /** Connected browsers by id. */
  declare browsers: Map<any, any>;
  /** Set on shutdown: new browsers are turned away while in-flight work finishes. */
  declare draining: any;
  constructor() {
    super();
    this.browsers = new Map();
  }

  /** Registers a browser: an inbound client (Oya) that dialled us, or one we dialled over CDP, which brings its engine. */
  add(browserId, spec: BrowserSpec) {
    const record = newRecord(spec, browserId);
    this.browsers.set(browserId, record);
    const { clientType, provider } = record;
    this.emit('browser:connected', { id: browserId, name: spec.name, clientType, provider });
  }

  /** Forget a browser, close its driver, release its vendor session and end its stream viewers. */
  remove(browserId) {
    const browser = this.browsers.get(browserId);
    if (!browser) return;
    // Remove before closing: a synchronous close callback can re-enter.
    this.browsers.delete(browserId);
    closeOutbound(browser);
    endViewers(browser);
    const seconds = Math.round((Date.now() - browser.connectedAt.getTime()) / MS_PER_SECOND);
    const { name, apiKey, provider } = browser;
    this.emit('browser:disconnected', { id: browserId, name, apiKey, provider, seconds });
  }

  /** The live record for a browser, or undefined. */
  get(browserId) {
    return this.browsers.get(browserId);
  }

  /** A command was sent. Paired with recordActivity when it settles. */
  commandStarted(browserId) {
    const b = this.browsers.get(browserId);
    if (b) b.pending++;
  }

  /**
   * A command settled. `summary` is already reduced by summarise(), the log
   * never holds what was typed.
   */
  recordActivity(browserId, activity) {
    const b = this.browsers.get(browserId);
    if (b) applyActivity(b, activity);
  }

  /** One browser, shaped for the API, with the actions it does and its activity. */
  describe(browserId) {
    const b = this.browsers.get(browserId);
    if (!b) return null;
    return { ...this.row(browserId, b), actions: b.driver.actions(), activity: b.activity };
  }

  /** A browser as the API lists it: identity, health and counters, without activity. */
  row(id, b) {
    return rowOf(id, b);
  }

  /** Whether this replica holds the browser. */
  isConnected(browserId) {
    return this.browsers.has(browserId);
  }

  /** Check if a browser belongs to a specific API key */
  belongsTo(browserId, apiKey) {
    const browser = this.browsers.get(browserId);
    return browser ? browser.apiKey === apiKey : false;
  }

  /** Mark the browser as just heard from. */
  updateLastSeen(browserId) {
    const browser = this.browsers.get(browserId);
    if (browser) browser.lastSeen = new Date();
  }

  /** Remember the page the browser is on. */
  updateUrl(browserId, url) {
    const browser = this.browsers.get(browserId);
    if (browser) browser.currentUrl = url;
  }

  /** Keep the latest screencast frame and send it to viewers; a viewer more than 1MB behind skips it. */
  pushFrame(browserId, dataUrl) {
    const browser = this.browsers.get(browserId);
    if (!browser) return;
    browser.lastFrame = dataUrl;
    browser.lastFrameAt = Date.now();
    sendFrame(browser, dataUrl);
  }

  /** Attach a stream viewer; the first one starts the screencast. False when the browser is gone. */
  addViewer(browserId, res) {
    const browser = this.browsers.get(browserId);
    if (!browser) return false;
    browser.streamViewers.add(res);
    if (browser.streamViewers.size === 1) this.emit('stream:start', { id: browserId });
    return true;
  }

  /** Detach a stream viewer; the last one stops the screencast. */
  removeViewer(browserId, res) {
    const browser = this.browsers.get(browserId);
    if (!browser) return;
    browser.streamViewers.delete(res);
    if (browser.streamViewers.size === 0) this.emit('stream:stop', { id: browserId });
  }

  /** Whether anyone is watching the browser's stream. */
  hasViewers(browserId) {
    const browser = this.browsers.get(browserId);
    return browser ? browser.streamViewers.size > 0 : false;
  }

  /** List browsers visible to a specific API key */
  list(apiKey?) {
    const result = [];
    for (const [id, b] of this.browsers) {
      if (apiKey && b.apiKey !== apiKey) continue;
      result.push(this.row(id, b));
    }
    return result;
  }
}

/** The process-wide registry. */
export const registry = new ConnectionRegistry();
