/**
 * TOTP (RFC 6238): the six-digit code an authenticator app shows, computed from
 * the base32 secret printed under its QR code.
 */

import { createHmac } from 'crypto';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import {
  BASE32_BITS,
  BINARY,
  DECIMAL,
  MS_PER_SECOND,
  TOTP_CODE_MASK,
  TOTP_COUNTER_BYTES,
  TOTP_OFFSET_MASK,
} from './constants.ts';

/** The RFC 4648 base32 alphabet. */
const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
/** Whole bytes in a bit string; trailing bits short of a byte are padding. */
const WHOLE_BYTES = /[01]{8}/g;

/** One base32 character's value; anything outside the alphabet is a bad secret. */
function base32Value(ch) {
  const idx = ALPHABET.indexOf(ch);
  if (idx === -1) throw new HttpError(Status.BAD_REQUEST, 'TOTP secret is not valid base32');
  return idx;
}

/** Decode a base32 TOTP secret, tolerating lowercase, spaces, dashes and padding. */
function base32Decode(input) {
  const clean = String(input)
    .toUpperCase()
    .replace(/[\s=-]/g, '');
  const bits = [...clean].map((ch) => base32Value(ch).toString(BINARY).padStart(BASE32_BITS, '0')).join('');
  const out = (bits.match(WHOLE_BYTES) || []).map((byte) => parseInt(byte, BINARY));
  if (!out.length) throw new HttpError(Status.BAD_REQUEST, 'TOTP secret is empty');
  return Buffer.from(out);
}

/** The current unix time in whole seconds. */
const nowSeconds = () => Math.floor(Date.now() / MS_PER_SECOND);

/**
 * @param {string} secret base32, as printed under a QR code
 * @param {number} [at] unix seconds, for testing against known vectors
 */
export function totp(secret, at = nowSeconds(), { digits = 6, period = 30, algorithm = 'sha1' } = {}) {
  const counter = Buffer.alloc(TOTP_COUNTER_BYTES);
  counter.writeBigUInt64BE(BigInt(Math.floor(at / period)));
  const hmac = createHmac(algorithm, base32Decode(secret)).update(counter).digest();
  const offset = hmac[hmac.length - 1] & TOTP_OFFSET_MASK;
  const code = (hmac.readUInt32BE(offset) & TOTP_CODE_MASK) % DECIMAL ** digits;
  return String(code).padStart(digits, '0');
}
