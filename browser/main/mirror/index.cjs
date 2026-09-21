/**
 * Mirroring the user's real browser into Oya: capture every profile's device
 * and cookies, hand them to the server as personas, seed the desktop's own
 * partition with the site storage, then reconnect as the mirrored persona so
 * the desktop and its remote browsers are already signed in. Runs once, on the
 * first authenticated connection, and again on an explicit re-import.
 */
const { captureAll } = require('./capture.cjs');
const { seedStorage } = require('./storage.cjs');
const { installedSources } = require('./locate.cjs');
const { MIRROR_ANSWER_TIMEOUT_MS } = require('./constants.cjs');

/** The mirror_persona message: one entry per captured profile, all on the real device. */
function mirrorMessage(captured) {
  return { type: 'mirror_persona', profiles: captured.profiles.map((p) => profilePayload(captured, p)) };
}

/** One profile as the server needs it: its identity, the shared real device, and its session state. */
function profilePayload(captured, p) {
  const identity = { source: captured.source, profile: p.profile, name: `${captured.name} · ${p.name}` };
  return { ...identity, lastUsed: p.lastUsed, device: captured.device, cookies: p.cookies, origins: {} };
}

/** Shown when an import is asked for with no server to import into. */
const NOT_CONNECTED = 'Connect to Oya first, then import your logins.';
/** Shown when the server never answers an import. */
const NO_ANSWER = 'The server did not answer. Check your connection and try again.';

/** How many profiles and cookies a capture holds, for the person to read. */
function tally(captured) {
  const cookies = captured.profiles.reduce((sum, p) => sum + (p.cookies?.length || 0), 0);
  return { source: captured.name, profiles: captured.profiles.length, cookies };
}

/** Drives one mirror run and applies the server's answer. Wired in as ctx.mirror. */
class Mirror {
  /** `ctx` is the main-process context (see main.js); `capture` and `seed` are the capture and storage steps. */
  constructor(ctx, { capture = captureAll, seed = seedStorage } = {}) {
    Object.assign(this, { ctx, capture, seed });
    /** The capture waiting for the server's mirror_ok, or null. */
    this.pending = null;
    /** Gives up on a server that never answers. */
    this.answerTimer = null;
  }

  /**
   * Runs the mirror once, on first sign-in, when it is turned on. Off by
   * default: a successful mirror reconnects to switch personas, which nobody
   * asked for at sign-in. The person starts it themselves from "Import logins"
   * (reimport), where the reconnect is the expected result. OYA_MIRROR=1 turns
   * the automatic run on.
   */
  maybeRun() {
    const { mirroredFrom, apiKey } = this.ctx.config.values;
    if (process.env.OYA_MIRROR !== '1' || mirroredFrom || !apiKey) return undefined;
    return this.run();
  }

  /** The browsers on this machine an import can read, the default one first. */
  sources() {
    return installedSources();
  }

  /** Imports `sourceId` (the default browser when omitted) from scratch; the device stays fixed server-side. */
  reimport(sourceId) {
    this.ctx.config.merge({ mirroredFrom: '' });
    return this.run(sourceId);
  }

  /** Captures the real browser and sends it to the server. Every way it can end is told to the person. */
  async run(sourceId) {
    if (!this.ctx.socket.ready || !this.ctx.socket.isOpen()) return this.notify({ done: true, error: NOT_CONNECTED });
    this.notify({ started: true });
    const captured = await this.capture(this.ctx, sourceId).catch((e) => e);
    if (captured instanceof Error) return this.failed(captured.message);
    if (!captured) return this.done('none', { empty: true });
    this.send(captured);
  }

  /** Hands the capture to the server and waits, not for ever, for its answer. */
  send(captured) {
    this.pending = captured;
    this.ctx.socket.send(mirrorMessage(captured));
    this.answerTimer = setTimeout(() => this.failed(NO_ANSWER), MIRROR_ANSWER_TIMEOUT_MS);
  }

  /** The import did not happen: say why, and leave it free to be tried again. */
  failed(error) {
    console.log('[oya] Mirror failed:', error);
    this.settle();
    this.notify({ done: true, error });
  }

  /** Forgets the capture in flight and stops waiting for its answer. */
  settle() {
    clearTimeout(this.answerTimer);
    const captured = this.pending;
    this.pending = null;
    return captured;
  }

  /** The server made the personas: seed the default's storage, remember it, and reconnect as it. */
  onOk(msg) {
    const captured = this.settle();
    if (!captured) return;
    this.seedDefault(captured, msg);
    this.ctx.config.merge({ mirroredFrom: captured.source, persona: msg.defaultPersonaId });
    this.ctx.config.save();
    this.reconnect();
    this.notify({ done: true, ...tally(captured) });
  }

  /** The server refused the import: tell the person why. */
  onFailed(msg) {
    this.failed(msg.error || 'The server refused the import.');
  }

  /** Copies the default persona's site storage into its partition before it is loaded. */
  seedDefault(captured, msg) {
    const at = (msg.personaIds || []).indexOf(msg.defaultPersonaId);
    const profile = captured.profiles[at];
    if (!profile) return;
    try {
      this.seed(this.ctx.electron, msg.defaultPersonaId, captured.userDataDir, profile.profile);
    } catch (e) {
      console.log('[oya] Mirror storage seed failed:', e.message);
    }
  }

  /** Drops the socket and dials again, so the next auth runs as the mirrored persona. */
  reconnect() {
    this.ctx.socket.disconnect();
    this.ctx.socket.connect();
  }

  /** Marks the run finished so it does not repeat, and notifies the renderer. */
  done(mirroredFrom, status) {
    this.ctx.config.merge({ mirroredFrom });
    this.ctx.config.save();
    this.notify({ done: true, ...status });
  }

  /** Tells the renderer where the import is, for the setup and profile screens. */
  notify(status) {
    this.ctx.shell.send('mirror-status', status);
  }
}

/** Builds the mirror controller for the context; `steps` replaces the capture and seeding (tests). */
function createMirror(ctx, steps) {
  return new Mirror(ctx, steps);
}

module.exports = { createMirror };
