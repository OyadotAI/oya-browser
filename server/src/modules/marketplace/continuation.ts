/** Encrypted short-lived buyer context survives authentication without retaining AWS tokens. */
import { randomUUID } from 'node:crypto';
import { sealText, openText } from '../../platform/secrets.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { invalidSetup, validIdentity, identifier, type BuyerIdentity, type SetupContext } from './model.ts';
import { CONTINUATION_MS, CONTINUATION_SCOPE, CONTINUATION_VERSION, MAX_COOKIE } from './constants.ts';

/** Seals trusted resolved identity; AWS's original token is deliberately absent. */
export function sealContext(identity: BuyerIdentity, now: number) {
  const value = { ...identity, audience: CONTINUATION_SCOPE, version: CONTINUATION_VERSION };
  return sealText(CONTINUATION_SCOPE, { ...value, nonce: randomUUID(), expiresAt: now + CONTINUATION_MS });
}

/** Authenticates the entire context before using any identity or deadline. */
function decryptContext(value: string): SetupContext {
  try {
    if (!value || value.length > MAX_COOKIE) throw invalidSetup();
    return openText(CONTINUATION_SCOPE, value);
  } catch {
    throw invalidSetup();
  }
}

/** Rejects tampering, other audiences/products and expired sessions before linking. */
export function openContext(value: string, productCode: string, now: number): SetupContext {
  const context = decryptContext(value);
  if (!validContext(context, productCode)) throw invalidSetup();
  if (context.expiresAt <= now)
    throw new HttpError(Status.GONE, 'This setup session has expired', { code: 'setup_expired' });
  return context;
}

/** Every authority-bearing field is checked after authenticated decryption. */
function validContext(context: SetupContext, productCode: string) {
  return (
    validIdentity(context, productCode) &&
    context.audience === CONTINUATION_SCOPE &&
    context.version === CONTINUATION_VERSION &&
    identifier(context.nonce) &&
    Number.isFinite(context.expiresAt)
  );
}
