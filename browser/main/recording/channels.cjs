/**
 * One RecordingChannel per tab: it arms the page's recorder in the isolated
 * world and carries its events out, surviving navigations.
 */
const crypto = require('crypto');
const { RecordingChannel } = require('../../scripts/recording.cjs');
const { cdpAttach, cdp } = require('../cdp.cjs');
const { ANALYZER_ATTR_BYTES } = require('./constants.cjs');

/** Subscribes to one CDP event on a view; returns the unsubscribe. */
function listenOnView(view, method, fn) {
  const dbg = cdpAttach(view);
  const listener = (_event, name, params) => {
    if (name === method) fn(params);
  };
  dbg.on('message', listener);
  return () => dbg.off('message', listener);
}

/** View → its recording channel. */
class RecordingChannels {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** View → channel. */
    this.channels = new Map();
  }

  /** Arms a view; resolves once its recorder is running. */
  async armRecordingView(view) {
    if (this.channels.has(view)) return this.channels.get(view).ready;
    const channel = this.createChannel(view);
    this.channels.set(view, channel);
    channel.ready = channel.start();
    await this.armed(view, channel);
  }

  /** Waits for a channel to start; one that fails is forgotten, so the next arm retries. */
  async armed(view, channel) {
    try {
      await channel.ready;
    } catch (err) {
      this.channels.delete(view);
      throw err;
    }
  }

  /** A channel for one view, remembering where the view started. */
  createChannel(view) {
    const startingUrl = view.webContents.getURL();
    return new RecordingChannel(this.channelOptions(view, startingUrl));
  }

  /** How the channel talks to the view and where its steps go. */
  channelOptions(view, startingUrl) {
    return {
      send: (method, params) => cdp(view, method, params),
      on: (method, fn) => listenOnView(view, method, fn),
      disableRuntimeOnStop: true,
      worldName: this.ctx.isolatedWorld,
      analyzer: this.analyzer(),
      receive: (out) => this.ctx.recorder.receive(view, startingUrl, out),
    };
  }

  /** The analyzer with a fresh tag attribute, not yet recording. */
  analyzer() {
    const attr = 'data-' + crypto.randomBytes(ANALYZER_ATTR_BYTES).toString('hex');
    return this.ctx.analyzerScript.replace('__OYA_ATTR__', attr).replace('__OYA_RECORD__', 'false');
  }

  /** Collects what one view buffered. */
  async drain(view, final) {
    await this.channels.get(view)?.drain(final);
  }

  /** Stops every channel (a closed tab's failure is expected) and forgets them. */
  async stopAll() {
    for (const [view, channel] of this.channels) {
      await channel.ready.catch(() => {});
      await channel.stop().catch((err) => {
        if (!view.webContents.isDestroyed()) throw err;
      });
    }
    this.channels.clear();
  }

  /** Clears what every channel buffered. */
  async clearAll() {
    for (const channel of this.channels.values()) await channel.clear();
  }
}

module.exports = { RecordingChannels };
