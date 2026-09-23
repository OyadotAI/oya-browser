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

/** POSTs JSON to `/api/browsers/<this browser>/<route>`, given up after `timeoutMs` when one is set, or when `signal` aborts. */
function postToBrowserApi(ctx, route, payload, timeoutMs, signal) {
  const config = ctx.config.values;
  const signals = [signal, timeoutMs && AbortSignal.timeout(timeoutMs)].filter(Boolean);
  return fetch(`${serverHttpBase(config.serverUrl)}/api/browsers/${ctx.socket.browserId}/${route}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.apiKey}` },
    body: JSON.stringify(payload),
    ...(signals.length ? { signal: AbortSignal.any(signals) } : {}),
  });
}

/** GETs `/api/<route>` as this project: answers its JSON, or throws. */
async function getFromApi(ctx, route) {
  const config = ctx.config.values;
  const res = await fetch(`${serverHttpBase(config.serverUrl)}/api/${route}`, {
    headers: { Authorization: `Bearer ${config.apiKey}` },
  });
  if (!res.ok) throw new Error(`Server returned ${res.status}`);
  return res.json();
}

module.exports = { serverHttpBase, canCallServer, postToBrowserApi, getFromApi };
