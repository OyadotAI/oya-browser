/** IPC: settings, connection status, control handoff, the saved profile and the fingerprint. */

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
  }),
  'save-profile': saveProfile,
  'get-fingerprint': (ctx) => ctx.persona.summary(),
};

module.exports = { SESSION_HANDLERS };
