/**
 * The server's gateway reaches our CDP front door through the control socket,
 * so a sandbox or a desktop behind NAT needs no inbound port. sid → local socket.
 */
const WebSocket = require('ws');
const { MAX_CDP_PAYLOAD } = require('./constants.cjs');

/** Drops `sid` and tells the server its relay closed, with the reason. */
function failRelay({ send, cdpRelays }, sid, error) {
  cdpRelays.delete(sid);
  send({ type: 'cdp_closed', sid, error });
}

/** Opens one relayed CDP socket onto the front door's browser endpoint. */
async function openRelaySocket(relay, sid) {
  const fail = (error) => failRelay(relay, sid, error);
  if (!relay.port) return fail('CDP is off in this browser. Start it with OYA_REMOTE_DEBUGGING_PORT set.');
  try {
    wireRelaySocket(relay, sid, await dialRelay(relay.port, relay.token), fail);
  } catch (e) {
    fail(e.message);
  }
}

/** Connects to the front door's browser target, carrying the relay token. */
async function dialRelay(port, token) {
  const { webSocketDebuggerUrl } = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
  return new WebSocket(`ws://127.0.0.1:${port}${new URL(webSocketDebuggerUrl).pathname}`, {
    headers: { 'X-Oya-Relay': token },
    perMessageDeflate: false,
    maxPayload: MAX_CDP_PAYLOAD,
  });
}

/** Registers the socket under `sid` and forwards its life over the control socket. */
function wireRelaySocket({ send, cdpRelays }, sid, sock, fail) {
  cdpRelays.set(sid, sock);
  sock.on('open', () => send({ type: 'cdp_opened', sid }));
  sock.on('message', (data) => send({ type: 'cdp', sid, data: data.toString() }));
  sock.on('close', () => cdpRelays.delete(sid) && send({ type: 'cdp_closed', sid }));
  sock.on('error', (e) => cdpRelays.has(sid) && fail(e.message));
}

/** Closes every relayed socket. */
function closeRelaySockets(cdpRelays) {
  for (const sock of cdpRelays.values())
    try {
      sock.close();
    } catch {}
  cdpRelays.clear();
}

/** `port` is the CDP front door, `token` proves the relay is ours, `send` writes to the control socket. */
function createCdpRelay({ port, token, send }) {
  const cdpRelays = new Map();
  const relay = { port, token, send, cdpRelays };
  return {
    cdpRelays,
    openCdpRelay: (sid) => openRelaySocket(relay, sid),
    closeCdpRelays: () => closeRelaySockets(cdpRelays),
  };
}

module.exports = { createCdpRelay };
