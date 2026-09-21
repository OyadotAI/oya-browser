/** IPC: settings, connection status, control handoff, the saved profile and the fingerprint. */
/**
 * The workspace's own console, derived from the server address rather than
 * taken from the renderer: openExternal will hand any scheme to the operating
 * system, so only a ws or wss address becomes a link, and only its origin.
 */
function consoleUrl(serverUrl) {
  try {
    const url = new URL(serverUrl);
    if (!['ws:', 'wss:'].includes(url.protocol)) return null;
    return `${url.protocol === 'wss:' ? 'https:' : 'http:'}//${url.host}/dashboard?connect=desktop`;
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

/**
 * Logs out of Oya: forgets the key and goes back to the welcome screen. The
 * server address, browser name and the sites this browser is logged in to stay.
 * `signedOut` keeps an OYA_API_KEY in the environment from signing it back in
 * on the next launch; signing in again clears it.
 */
function signOut(ctx) {
  ctx.socket.disconnect();
  ctx.socket.browserId = null;
  ctx.config.merge({ apiKey: '', signedOut: true });
  ctx.config.save();
  ctx.tabs.leaveBrowsingMode();
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
    if (url) await ctx.electron.shell.openExternal(url);
    return url;
  },
  'save-config': (ctx, _e, newConfig) => {
    ctx.config.merge({ ...newConfig, signedOut: false });
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
  'sign-out': signOut,
  'get-fingerprint': (ctx) => ctx.persona.summary(),
};

module.exports = { SESSION_HANDLERS };
