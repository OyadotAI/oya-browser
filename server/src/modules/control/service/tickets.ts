/** Connection tickets: single-use, one-minute stand-ins for a credential on one session's connection URL. */
import { randomBytes } from 'node:crypto';
import { sealText, openText } from '../../../platform/secrets.ts';
import { Status } from '../../../platform/http-status.ts';
import { fault, hash, projectId, stamp } from './model.ts';
import { TICKET_TTL_MS, TOKEN_BYTES } from './constants.ts';

/** The stored ticket: which session it opens, and the sealed credential it stands for. */
function ticketRow(key, sessionId, authToken) {
  const auth = sealText('connection-ticket', authToken);
  return { project: projectId(key), sessionId, auth, expiresAt: stamp() + TICKET_TTL_MS };
}

/** A single-use, one-minute ticket that stands in for the credential on one session's connection URL. */
export async function ticket(store, key, sessionId, authToken = key) {
  const token = randomBytes(TOKEN_BYTES).toString('base64url');
  await store.transact(async (tx) => {
    tx.put('ticket', hash(token), ticketRow(key, sessionId, authToken));
  });
  return token;
}

/** Consume a ticket for this session and return the credential it stands for. */
export async function redeem(store, token, sessionId) {
  return store.transact(async (tx) => {
    const t = await tx.get('ticket', hash(token));
    if (!t || t.expiresAt < stamp() || t.sessionId !== sessionId)
      throw fault('invalid_ticket', 'Invalid connection ticket', Status.UNAUTHORIZED);
    await tx.delete('ticket', hash(token));
    return openText('connection-ticket', t.auth);
  });
}
