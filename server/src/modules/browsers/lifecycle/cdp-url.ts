/**
 * The CDP URL handed to callers: our gateway, never the vendor's.
 */
import { getKey } from '../../../app/http.ts';
import { control } from '../../control/service.ts';
import { DEFAULT_PORT } from '../constants.ts';

/** A single-use CDP WebSocket URL for a browser, authorised by a fresh ticket. */
export async function browserCdpUrl(req, id) {
  const host = req.headers.host || `localhost:${process.env.PORT || DEFAULT_PORT}`;
  const scheme = req.secure || String(req.headers['x-forwarded-proto'] || '').includes('https') ? 'wss' : 'ws';
  const ticket = await control().ticket(getKey(req), id, req.authToken || getKey(req));
  return `${scheme}://${host}/connect?ticket=${encodeURIComponent(ticket)}&browser=${encodeURIComponent(id)}`;
}
