/**
 * Envelope encryption for anything stored at rest that a leak would matter for:
 * profile snapshots, proxy credentials, MFA seeds.
 *
 * A random data key per record, wrapped by a KEK derived from
 * OYA_PROFILE_SECRET with scrypt. The caller-supplied scope is the AAD on both
 * layers, so a ciphertext copied into another scope, another tenant's
 * namespace, another record type, fails to open rather than decrypting.
 */

import { createCipheriv, createDecipheriv, randomBytes, scryptSync } from 'crypto';
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { dataPath } from './paths.ts';
import { HttpError } from './errors.ts';
import { Status } from './http-status.ts';
import { KEY_BYTES, NONCE_BYTES, SCRYPT_MAXMEM, SCRYPT_N, SCRYPT_R, SEALED_VERSION, TAG_BYTES } from './constants.ts';

const SECRET_FILE = dataPath('.secret');

/**
 * A self-hoster's first action is onboarding, which stores an LLM key, so a
 * missing OYA_PROFILE_SECRET generates one on disk rather than refusing.
 * An operator-supplied secret is still better (the key then lives somewhere
 * other than next to the ciphertext), but a generated one beats the two
 * alternatives: plaintext credentials, or a product that cannot be set up.
 */
function fileSecret() {
  return readSecretFile() ?? generateSecretFile();
}

/** The secret on disk, or null when there is none to read. */
function readSecretFile() {
  try {
    return readFileSync(SECRET_FILE, 'utf8').trim();
  } catch {
    return null;
  }
}

/** Generate a secret and store it; if that fails, use one another worker stored first. */
function generateSecretFile() {
  const generated = randomBytes(KEY_BYTES).toString('hex');
  try {
    return writeSecretFile(generated);
  } catch (e) {
    // Lost a race with another worker, or the directory is read-only.
    return readSecretFile() ?? cannotStore(e);
  }
}

/** Write a new secret file (never over an existing one) and warn that it needs backing up. */
function writeSecretFile(generated) {
  mkdirSync(dirname(SECRET_FILE), { recursive: true });
  writeFileSync(SECRET_FILE, generated, { mode: 0o600, flag: 'wx' });
  console.warn(
    `[secrets] OYA_PROFILE_SECRET not set, generated one at ${SECRET_FILE}. ` +
      'Back it up: losing it makes stored credentials unrecoverable.',
  );
  return generated;
}

/** No secret configured and none can be stored. */
function cannotStore(e): never {
  throw new HttpError(Status.CONFLICT, `Cannot store secrets: set OYA_PROFILE_SECRET (${e.message})`);
}

const SECRET = process.env.OYA_PROFILE_SECRET || '';
const SALT = Buffer.from(process.env.OYA_PROFILE_SALT || 'oya-profile-kek-v1');

let kek = null;

/** Always true now, kept because callers guard on it before storing credentials. */
export function haveSecret() {
  try {
    return !!(SECRET || fileSecret());
  } catch {
    return false;
  }
}

/** The key-encryption key, derived once from the secret and cached. */
function key() {
  if (kek) return kek;
  const secret = SECRET || fileSecret();
  // scrypt is deliberate: the secret may be operator-chosen rather than random.
  // N=2^15,r=8 needs 32MB, exactly Node's default maxmem ceiling, so maxmem is
  // raised explicitly rather than left to trip at runtime.
  kek = scryptSync(secret, SALT, KEY_BYTES, { N: SCRYPT_N, r: SCRYPT_R, p: 1, maxmem: SCRYPT_MAXMEM });
  return kek;
}

/** @returns {Buffer} version | wrapNonce | wrapTag | wrappedDek | nonce | tag | body */
export function seal(scope, value) {
  const dek = randomBytes(KEY_BYTES);
  const aad = Buffer.from(scope);
  const inner = encrypt(dek, aad, JSON.stringify(value));
  const outer = encrypt(key(), aad, dek);
  const version = Buffer.from([SEALED_VERSION]);
  return Buffer.concat([version, outer.nonce, outer.tag, outer.body, inner.nonce, inner.tag, inner.body]);
}

/** AES-256-GCM under `k` with the scope as AAD: a fresh nonce, the tag and the ciphertext. */
function encrypt(k, aad, plaintext) {
  const nonce = randomBytes(NONCE_BYTES);
  const c = createCipheriv('aes-256-gcm', k, nonce);
  c.setAAD(aad);
  const body = Buffer.concat([c.update(plaintext, 'utf8'), c.final()]);
  return { nonce, tag: c.getAuthTag(), body };
}

/** Byte lengths of the sealed fields after the version byte, in order; the body is the rest. */
const SEALED_LAYOUT = [NONCE_BYTES, TAG_BYTES, KEY_BYTES, NONCE_BYTES, TAG_BYTES];

/** Opens a sealed buffer under the same scope; throws if the scope differs or the data was altered. */
export function open(scope, buf) {
  if (buf[0] !== SEALED_VERSION) throw new Error('Unsupported sealed format');
  const aad = Buffer.from(scope);
  const p = unpack(buf);
  const dek = decrypt(key(), aad, p.wrapNonce, p.wrapTag, p.wrapped);
  return JSON.parse(decrypt(dek, aad, p.nonce, p.tag, p.body).toString('utf8'));
}

/** Split a sealed buffer into its fields. */
function unpack(buf) {
  let o = 1;
  const take = (n) => buf.subarray(o, (o += n));
  const [wrapNonce, wrapTag, wrapped, nonce, tag] = SEALED_LAYOUT.map(take);
  return { wrapNonce, wrapTag, wrapped, nonce, tag, body: buf.subarray(o) };
}

/** Open one AES-256-GCM layer; throws when the tag does not verify. */
function decrypt(k, aad, nonce, tag, data) {
  const d = createDecipheriv('aes-256-gcm', k, nonce);
  d.setAAD(aad);
  d.setAuthTag(tag);
  return Buffer.concat([d.update(data), d.final()]);
}

/** Base64 for records that live in JSON rather than as raw files. */
export const sealText = (scope, value) => seal(scope, value).toString('base64');
/** Opens a sealText value. */
export const openText = (scope, text) => open(scope, Buffer.from(text, 'base64'));
