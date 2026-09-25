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

/** Sends `method` with a JSON body to `/api/<route>` as this project: answers its JSON, or throws with the server's own error and status. */
async function sendToApi(ctx, method, route, payload) {
  const config = ctx.config.values;
  const res = await fetch(`${serverHttpBase(config.serverUrl)}/api/${route}`, {
    method,
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.apiKey}` },
    ...(payload === undefined ? {} : { body: JSON.stringify(payload) }),
  });
  return readApiAnswer(res);
}

/** POSTs JSON to `/api/<route>` as this project: answers its JSON, or throws with the server's own error. */
const postToApi = (ctx, route, payload) => sendToApi(ctx, 'POST', route, payload);

/** A response's JSON, or an error with the server's own message and the status (a 409 is a lost race, say). */
async function readApiAnswer(res) {
  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw Object.assign(new Error(body.error || `Server returned ${res.status}`), { status: res.status });
  return body;
}

module.exports = { serverHttpBase, canCallServer, postToBrowserApi, getFromApi, postToApi, sendToApi };
