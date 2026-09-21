/**
 * One RecordingChannel per tab: it arms the page's recorder in the isolated
 * world and carries its events out, surviving navigations.
 */
const crypto = require('crypto');
const { RecordingChannel } = require('../../scripts/recording.cjs');
const { cdpAttach, cdp } = require('../cdp.cjs');
const { withinTime } = require('../../scripts/within-time.cjs');
const { ANALYZER_ATTR_BYTES, RECORDING_CDP_MS } = require('./constants.cjs');

/** Subscribes to one CDP event on a view; returns the unsubscribe. */
function listenOnView(view, method, fn) {
  const dbg = cdpAttach(view);
  const listener = (_event, name, params) => {
    if (name === method) fn(params);
  };
  dbg.on('message', listener);
  return () => dbg.off('message', listener);
}

/** Stops one channel; resolves to a live tab's failure, or null. */
async function stopOne(view, channel) {
  await channel.ready.catch(() => {});
  try {
    await channel.stop();
    return null;
  } catch (err) {
    return view.webContents.isDestroyed() ? null : err;
  }
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

  /** A channel for one view, remembering where the view started and which tab it is. */
  createChannel(view) {
    const startingUrl = view.webContents.getURL();
    const tabId = this.ctx.tabs.list.find((t) => t.view === view)?.id;
    return new RecordingChannel(this.channelOptions(view, startingUrl, tabId));
  }

  /** How the channel talks to the view (never waiting forever) and where its steps go. */
  channelOptions(view, startingUrl, tabId) {
    return {
      send: (method, params) => withinTime(cdp(view, method, params), RECORDING_CDP_MS, 'The page did not answer'),
      on: (method, fn) => listenOnView(view, method, fn),
      disableRuntimeOnStop: true,
      worldName: this.ctx.isolatedWorld,
      analyzer: this.analyzer(),
      receive: (out) => this.ctx.recorder.receive(view, startingUrl, out, tabId),
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

  /**
   * Stops every channel and forgets them all, whatever happens. A closed tab's
   * failure is expected; a live tab's is thrown, but only after the others are
   * stopped, so one refusing page never leaves the rest listening.
   */
  async stopAll() {
    const channels = [...this.channels];
    this.channels.clear();
    let failure = null;
    for (const [view, channel] of channels) failure = (await stopOne(view, channel)) || failure;
    if (failure) throw failure;
  }

  /** A closed tab's channel: stopped in the background and forgotten. */
  forget(view) {
    const channel = this.channels.get(view);
    this.channels.delete(view);
    if (channel) stopOne(view, channel).catch(() => {});
  }
}

module.exports = { RecordingChannels };
