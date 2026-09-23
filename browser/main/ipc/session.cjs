/** IPC: settings, connection status, control handoff, the saved profile and the fingerprint. */
const { getFromApi } = require('../connection/server-api.cjs');
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
  await ctx.persona.flushJar();
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
  ctx.config.merge({ apiKey: '', signedOut: true, keyFromApp: false });
  ctx.config.save();
  ctx.tabs.leaveBrowsingMode();
}

/** The project's personas for the chat's profile picker, and the one this browser asked for. */
async function listPersonas(ctx) {
  const active = ctx.config.values.persona || 'default';
  if (!ctx.socket.ready) return { personas: [], active };
  const { personas } = await getFromApi(ctx, 'personas');
  return { personas: personas.map(({ id, name, isDefault }) => ({ id, name, isDefault })), active };
}

/** Whether `changes` point this browser at another key or server, that is another project. */
function movesProject(config, changes) {
  const differs = (field) => field in changes && changes[field] !== config[field];
  return differs('apiKey') || differs('serverUrl');
}

/**
 * Saves the settings and reconnects. A key typed here is the person's choice,
 * kept over OYA_API_KEY from then on. Another project's server refused the old
 * session id and persona (a foreign id, an unknown persona) on every retry, so
 * a move starts both fresh, as a pairing link does.
 */
function saveConfig(ctx, _e, changes) {
  const moved = movesProject(ctx.config.values, changes);
  if (moved) ctx.socket.browserId = null;
  const chosen = 'apiKey' in changes ? { keyFromApp: true } : {};
  ctx.config.merge({ ...(moved && { persona: 'default' }), ...changes, signedOut: false, ...chosen });
  ctx.config.save();
  ctx.socket.disconnect();
  ctx.socket.connect();
  return true;
}

/** `sourceId` when it names a browser the import lists; undefined (the default browser) otherwise. */
function listedSource(ctx, sourceId) {
  return ctx.mirror.sources().some((source) => source.id === sourceId) ? sourceId : undefined;
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
  'save-config': saveConfig,
  'get-status': (ctx) => ({
    connected: ctx.socket.ready,
    browserId: ctx.socket.browserId,
    profileName: ctx.config.values.profileName,
    url: ctx.tabs.getActiveView()?.webContents.getURL() || '',
    // The renderer asks for this after it loads. `mode-changed` is sent once, and
    // a shell that was still loading when the server accepted the browser would
    // otherwise sit on the setup screen for the rest of the session.
    browsing: !!ctx.shell.browsingMode,
  }),
  'save-profile': saveProfile,
  'import-sources': (ctx) => ctx.mirror.sources(),
  // The id comes from the shell page: only a listed browser is ever opened.
  'reimport-browser': (ctx, _e, sourceId) => ctx.mirror.reimport(listedSource(ctx, sourceId)),
  'sign-out': signOut,
  'get-fingerprint': (ctx) => ctx.persona.summary(),
  'list-personas': listPersonas,
};

module.exports = { SESSION_HANDLERS };
