/**
 * Gateway WebSocket forwarding: an upgrade for a session another replica owns
 * is bridged, frame for frame, to that owner.
 */
import { WebSocket, WebSocketServer } from 'ws';
import { ownerFor } from './owner.ts';
import { hopHeader, refuseRehop, retarget, verifyHop } from './hop.ts';
import { GOING_AWAY, HANDSHAKE_TIMEOUT_MS, MAX_BRIDGE_PAYLOAD, UNAVAILABLE_UPGRADE } from './constants.ts';

/** Accepts the client side of bridged upgrades. */
const bridge = new WebSocketServer({ noServer: true, perMessageDeflate: false, maxPayload: MAX_BRIDGE_PAYLOAD });

/** Bridges a gateway WebSocket upgrade to the replica that owns the session; false when this replica should take it. */
export async function forwardGateway(req, socket, head, token, key, id) {
  if (!process.env.OYA_INSTANCE_URL) return false;
  verifyHop(req);
  const owner = await ownerFor(id, key);
  if (!owner) return false;
  refuseRehop(req, 'Session ownership changed');
  bridgeTo(req, socket, head, dialOwner(owner.url, req.url, token));
  return true;
}

/** Opens the upstream WebSocket to the owner, signed as a hop. */
function dialOwner(ownerUrl, path, token) {
  const target = retarget(ownerUrl, path, ['ticket', 'token']);
  target.protocol = target.protocol === 'https:' ? 'wss:' : 'ws:';
  return new WebSocket(target, {
    headers: { Authorization: `Bearer ${token}`, 'X-Oya-Hop': hopHeader('GET', target.pathname + target.search) },
    handshakeTimeout: HANDSHAKE_TIMEOUT_MS,
    maxPayload: MAX_BRIDGE_PAYLOAD,
  });
}

/** Completes the client's upgrade once upstream opens; answers 503 if it never does. */
function bridgeTo(req, socket, head, upstream) {
  socket.once('close', () => upstream.terminate());
  upstream.once('error', () => {
    if (!socket.destroyed) socket.end(UNAVAILABLE_UPGRADE);
  });
  upstream.once('open', () => bridge.handleUpgrade(req, socket, head, (client) => pipe(client, upstream)));
}

/** Copies frames both ways and ties the two sockets' lifetimes together. */
function pipe(client, upstream) {
  upstream.on('message', (data, binary) => send(client, data, binary));
  client.on('message', (data, binary) => send(upstream, data, binary));
  client.on('close', () => upstream.close());
  upstream.on('close', () => client.close(GOING_AWAY, 'Session connection ended'));
  client.on('error', () => upstream.terminate());
}

/** Sends a frame if the socket is still open. */
function send(to, data, binary) {
  if (to.readyState === WebSocket.OPEN) to.send(data, { binary });
}
