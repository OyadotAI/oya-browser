/**
 * Message type → handler: everything the server can say to this browser. The
 * socket looks the type up here; an unknown type is ignored.
 */
const governance = require('../../governance');
const { HOME_URL } = require('../tabs/constants.cjs');
const { DEFAULT_STREAM_FPS } = require('./constants.cjs');

/** The server accepted us: take the persona it sent, go online, and share our cookies. */
async function acceptAuth(ctx, msg) {
  ctx.socket.reconnectAttempts = 0;
  if (msg.browser_id) ctx.socket.browserId = msg.browser_id;
  ctx.persona.ensureLoginState(msg);
  // Apply fingerprint from the server, the server is the single source of truth.
  // Same API key = same fingerprint on every browser, guaranteed.
  if (msg.fingerprint) await ctx.persona.applyServerFingerprint(msg.fingerprint, msg.cookies || []);
  goOnline(ctx, msg);
  if (!ctx.shell.browsingMode) ctx.tabs.enterBrowsingMode(governance.configuration ? 'about:blank' : HOME_URL);
  await shareProfile(ctx);
}

/** Sends our cookies to the pool, then, on first sign-in only, mirrors the user's real browser. */
async function shareProfile(ctx) {
  await ctx.cookies.dumpCookies();
  ctx.socket.send({ type: 'profile_flush' });
  // A no-op once done; it reconnects as the mirrored persona itself.
  void ctx.mirror.maybeRun();
}

/** Marks the socket ready, takes the control state, and starts the heartbeat. */
function goOnline(ctx, msg) {
  ctx.socket.ready = true;
  ctx.control.connect(msg.control);
  if (msg.control) governance.setMode(msg.control.mode);
  ctx.config.values.profileName = msg.persona?.name || 'Default';
  ctx.config.save();
  ctx.socket.startPingLoop();
  ctx.socket.sendStatus();
}

/** A relayed CDP session closed on the server's side. */
function closeRelay(ctx, msg) {
  const sock = ctx.relay.cdpRelays.get(msg.sid);
  ctx.relay.cdpRelays.delete(msg.sid);
  sock?.close();
}

/** The handler for each message type. */
const SERVER_MESSAGES = {
  control_mode: (ctx, msg) => {
    if (msg.state) ctx.control.receive(msg.state);
    governance.setMode(msg.state ? ctx.control.snapshot().mode : msg.mode);
  },
  desktop_control_result: (ctx, msg) => {
    ctx.control.result(msg);
    if (msg.state) governance.setMode(ctx.control.snapshot().mode);
  },
  auth_ok: acceptAuth,
  profile_saved: (ctx, msg) => ctx.shell.send('profile-saved', msg),
  cookie_sync: async (ctx, msg) => {
    await ctx.cookies.applyCookieSync(msg.cookies);
    ctx.cookies.answerPull(msg.pullId);
  },
  mirror_ok: (ctx, msg) => ctx.mirror.onOk(msg),
  mirror_failed: (ctx, msg) => ctx.mirror.onFailed(msg),
  ping: (ctx) => {
    ctx.socket.heard();
    ctx.socket.send({ type: 'pong' });
  },
  pong: (ctx) => ctx.socket.heard(),
  stream_start: (ctx, msg) => ctx.stream.startStream(msg.fps || DEFAULT_STREAM_FPS),
  stream_stop: (ctx) => ctx.stream.stopStream(),
  // Commands run concurrently; the queue does not wait for one to finish.
  cmd: (ctx, msg) => void ctx.commands.handleCommand(msg),
  cdp_open: (ctx, msg) => void ctx.relay.openCdpRelay(msg.sid),
  cdp: (ctx, msg) => ctx.relay.cdpRelays.get(msg.sid)?.send(String(msg.data)),
  cdp_close: closeRelay,
};

/** Runs the handler for `msg.type`, if there is one. */
function handleServerMessage(ctx, msg) {
  if (!Object.hasOwn(SERVER_MESSAGES, msg.type)) return undefined;
  return SERVER_MESSAGES[msg.type](ctx, msg);
}

module.exports = { handleServerMessage, SERVER_MESSAGES };
