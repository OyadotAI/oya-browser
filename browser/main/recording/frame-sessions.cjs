/**
 * The cross-site iframes of each tab, as CDP sessions. Such an iframe runs in
 * its own process, a separate target the page's own session cannot reach, so a
 * recording has to arm it through its session. The persona applier attaches to
 * every one as the tab starts (anonymity/apply.js); this keeps the list, so a
 * recording started later still finds the iframes already on the page.
 */

/** Follows one attach or detach message into the tracked sessions, telling the watchers of a new iframe. */
function follow(tracked, method, params) {
  const info = params?.targetInfo;
  if (method === 'Target.attachedToTarget' && info?.type === 'iframe') {
    tracked.sessions.set(params.sessionId, info.targetId);
    for (const watcher of tracked.watchers) watcher.attached(params.sessionId, info.targetId);
  } else if (method === 'Target.detachedFromTarget') tracked.sessions.delete(params?.sessionId);
}

/** Hands a message from an iframe's session to whoever listens for that event there. */
function dispatch(tracked, method, params, sessionId) {
  for (const fn of tracked.listeners.get(`${sessionId} ${method}`) || []) fn(params);
}

/**
 * Tracks a view's iframe sessions from now on; call once per view, before its page loads.
 * One debugger listener serves every iframe: a page with dozens of them would otherwise
 * add four listeners for each while recording.
 */
function trackFrameSessions(view) {
  const tracked = { sessions: new Map(), watchers: new Set(), listeners: new Map() };
  view.webContents.debugger.on('message', (_e, method, params, sessionId) => {
    follow(tracked, method, params);
    if (sessionId) dispatch(tracked, method, params, sessionId);
  });
  view.frameSessions = tracked;
}

/** Subscribes to one CDP event from one session; returns the unsubscribe. */
function onSession(tracked, sessionId, event, fn) {
  const key = `${sessionId} ${event}`;
  if (!tracked.listeners.has(key)) tracked.listeners.set(key, new Set());
  tracked.listeners.get(key).add(fn);
  return () => tracked.listeners.get(key)?.delete(fn);
}

/** A port that talks to one iframe's session. */
function sessionPort(dbg, tracked, sessionId) {
  return {
    send: (method, params = {}) => dbg.sendCommand(method, params, sessionId),
    on: (event, fn) => onSession(tracked, sessionId, event, fn),
  };
}

/** Starts telling `attached` of each iframe that attaches; returns the stop. */
function watchFrames(tracked, attached) {
  const watcher = { attached };
  tracked.watchers.add(watcher);
  return () => tracked.watchers.delete(watcher);
}

/**
 * What a recording channel needs to arm a view's iframes: the ones attached
 * now, a way to hear of new ones, and a port that talks to one of them.
 */
function framePorts(view) {
  const tracked = view.frameSessions;
  if (!tracked) return null;
  return {
    list: () => [...tracked.sessions].map(([sessionId, frameId]) => ({ sessionId, frameId })),
    watch: (attached) => watchFrames(tracked, attached),
    port: (sessionId) => sessionPort(view.webContents.debugger, tracked, sessionId),
  };
}

module.exports = { trackFrameSessions, framePorts };
