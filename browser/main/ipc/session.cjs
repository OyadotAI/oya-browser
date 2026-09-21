/** IPC: settings, connection status, control handoff, the saved profile and the fingerprint. */
const { shell } = require('electron');

/**
 * The workspace's own console, derived from the server address rather than
 * taken from the renderer: openExternal will hand any scheme to the operating
 * system, so only a ws or wss address becomes a link, and only its origin.
 */
function consoleUrl(serverUrl) {
  try {
    const url = new URL(serverUrl);
    if (!['ws:', 'wss:'].includes(url.protocol)) return null;
    return `${url.protocol === 'wss:' ? 'https:' : 'http:'}//${url.host}/dashboard`;
  } catch {
    return null;
  }
}

/** Pushes this browser's cookies and storage to the server, now. */
async function saveProfile(ctx) {
  if (!ctx.socket.ready || !ctx.socket.isOpen()) throw new Error('Connect the desktop before saving your profile.');
  ctx.cookies.flushCookieChanges();
  await ctx.cookies.dumpCookies();
  await ctx.persona.session().cookies.flushStore();
  ctx.persona.session().flushStorageData();
  ctx.socket.send({ type: 'profile_flush' });
}

/** Channel → handler. */
const SESSION_HANDLERS = {
  'get-control-state': (ctx) => ctx.control.snapshot(),
  'change-control': async (ctx, _event, action) => {
    try {
      return { state: await ctx.control.change(action) };
    } catch (error) {
      return { error: error.message, state: ctx.control.snapshot() };
    }
  },
  'get-config': (ctx) => ctx.config.values,
  'open-console': async (ctx, _event, serverUrl) => {
    const url = consoleUrl(serverUrl || ctx.config.values.serverUrl);
    if (url) await shell.openExternal(url);
    return url;
  },
  'save-config': (ctx, _e, newConfig) => {
    ctx.config.merge(newConfig);
    ctx.config.save();
    ctx.socket.disconnect();
    ctx.socket.connect();
    return true;
  },
  'get-status': (ctx) => ({
    connected: ctx.socket.ready,
    browserId: ctx.socket.browserId,
    url: ctx.tabs.getActiveView()?.webContents.getURL() || '',
    // The renderer asks for this after it loads. `mode-changed` is sent once, and
    // a shell that was still loading when the server accepted the browser would
    // otherwise sit on the setup screen for the rest of the session.
    browsing: !!ctx.shell.browsingMode,
  }),
  'save-profile': saveProfile,
  'get-fingerprint': (ctx) => ctx.persona.summary(),
};

module.exports = { SESSION_HANDLERS };
