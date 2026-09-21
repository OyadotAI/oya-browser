/**
 * HTTP calls to the server behind the control socket, made as this browser
 * with the project key.
 */

/** The HTTP origin behind the control socket. */
function serverHttpBase(serverUrl) {
  return (serverUrl || '')
    .replace(/^wss/, 'https')
    .replace(/^ws/, 'http')
    .replace(/\/ws\/?$/, '');
}

/** Whether the socket is authenticated and this browser has an id to call as. */
function canCallServer(ctx) {
  return ctx.socket.ready && !!ctx.socket.browserId;
}

/** POSTs JSON to `/api/browsers/<this browser>/<route>`, given up after `timeoutMs` when one is set. */
function postToBrowserApi(ctx, route, payload, timeoutMs) {
  const config = ctx.config.values;
  return fetch(`${serverHttpBase(config.serverUrl)}/api/browsers/${ctx.socket.browserId}/${route}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.apiKey}` },
    body: JSON.stringify(payload),
    ...(timeoutMs ? { signal: AbortSignal.timeout(timeoutMs) } : {}),
  });
}

module.exports = { serverHttpBase, canCallServer, postToBrowserApi };
