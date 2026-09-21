/**
 * Mirroring the user's real browser into Oya: capture every profile's device
 * and cookies, hand them to the server as personas, seed the desktop's own
 * partition with the site storage, then reconnect as the mirrored persona so
 * the desktop and its remote browsers are already signed in. Runs once, on the
 * first authenticated connection, and again on an explicit re-import.
 */
const { captureAll } = require('./capture.cjs');
const { seedStorage } = require('./storage.cjs');

/** The mirror_persona message: one entry per captured profile, all on the real device. */
function mirrorMessage(captured) {
  return { type: 'mirror_persona', profiles: captured.profiles.map((p) => profilePayload(captured, p)) };
}

/** One profile as the server needs it: its identity, the shared real device, and its session state. */
function profilePayload(captured, p) {
  const identity = { source: captured.source, profile: p.profile, name: `${captured.name} — ${p.name}` };
  return { ...identity, lastUsed: p.lastUsed, device: captured.device, cookies: p.cookies, origins: {} };
}

/** Drives one mirror run and applies the server's answer. Wired in as ctx.mirror. */
class Mirror {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    this.ctx = ctx;
    /** The capture waiting for the server's mirror_ok, or null. */
    this.pending = null;
  }

  /**
   * Runs the mirror once, on first sign-in, when it is turned on. Off by
   * default: a successful mirror reconnects to switch personas, and until the
   * capture path is proven on every default browser (Firefox is not captured
   * yet) that reconnect looked like a dropped connection. Enable with
   * OYA_MIRROR=1, or trigger it by hand with reimport().
   */
  maybeRun() {
    const { mirroredFrom, apiKey } = this.ctx.config.values;
    if (process.env.OYA_MIRROR !== '1' || mirroredFrom || !apiKey) return undefined;
    return this.run();
  }

  /** Runs it again from scratch, e.g. the Re-import button; the device stays fixed server-side. */
  reimport() {
    this.ctx.config.merge({ mirroredFrom: '' });
    return this.run();
  }

  /** Captures the real browser and sends it to the server, or records that there was nothing to do. */
  async run() {
    this.notify({ started: true });
    const captured = await captureAll(this.ctx).catch((e) => void console.log('[oya] Mirror failed:', e.message));
    if (!captured) return this.done('none', { empty: true });
    this.pending = captured;
    this.ctx.socket.send(mirrorMessage(captured));
    return undefined;
  }

  /** The server made the personas: seed the default's storage, remember it, and reconnect as it. */
  onOk(msg) {
    const captured = this.pending;
    this.pending = null;
    if (captured) this.seedDefault(captured, msg);
    this.ctx.config.merge({ mirroredFrom: captured?.source || 'none', persona: msg.defaultPersonaId });
    this.ctx.config.save();
    this.reconnect();
    this.notify({ done: true, source: captured?.name });
  }

  /** The server refused the import: stop retrying and tell the renderer why. */
  onFailed(msg) {
    this.pending = null;
    this.done('none', { error: msg.error });
  }

  /** Copies the default persona's site storage into its partition before it is loaded. */
  seedDefault(captured, msg) {
    const at = (msg.personaIds || []).indexOf(msg.defaultPersonaId);
    const profile = captured.profiles[at];
    if (!profile) return;
    try {
      seedStorage(this.ctx.electron, msg.defaultPersonaId, captured.userDataDir, profile.profile);
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

/** Builds the mirror controller for the context. */
function createMirror(ctx) {
  return new Mirror(ctx);
}

module.exports = { createMirror };
