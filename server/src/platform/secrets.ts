/**
 * Envelope encryption for anything stored at rest that a leak would matter for:
 * profile snapshots, proxy credentials, MFA seeds, recordings.
 *
 * A random data key per record, wrapped by a KEK derived from
 * OYA_PROFILE_SECRET with scrypt. The caller-supplied scope is the AAD on both
 * layers, so a ciphertext copied into another scope, another tenant's
 * namespace, another record type, fails to open rather than decrypting.
 *
 * Rotation: a sealed buffer names the KEK that wrapped it by a short id. The
 * outgoing secret moves to OYA_PROFILE_SECRET_PREVIOUS (its salt, if it had
 * its own, to OYA_PROFILE_SALT_PREVIOUS), records sealed under it still open,
 * rewrap() moves one onto the current key, and new records use the current one.
 */

import { createCipheriv, createDecipheriv, createHash, hkdfSync, randomBytes, scryptSync } from 'crypto';
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { dataPath } from './paths.ts';
import { HttpError } from './errors.ts';
import { Status } from './http-status.ts';
import {
  KEY_BYTES,
  KEY_ID_BYTES,
  NONCE_BYTES,
  SCRYPT_MAXMEM,
  SCRYPT_N,
  SCRYPT_R,
  SEALED_KEYED_VERSION,
  SEALED_VERSION,
  sealKeyIds,
  TAG_BYTES,
} from './constants.ts';

const SECRET_FILE = dataPath('.secret');

/**
 * A self-hoster's first action is onboarding, which stores an LLM key, so a
 * missing OYA_PROFILE_SECRET generates one on disk rather than refusing.
 * An operator-supplied secret is still better (the key then lives somewhere
 * other than next to the ciphertext), but a generated one beats the two
 * alternatives: plaintext credentials, or a product that cannot be set up.
 */
function fileSecret() {
  // Read once: every sealed frame of a recording asks for the key.
  fileSecretRead ??= readSecretFile() ?? generateSecretFile();
  return fileSecretRead;
}

/** The file secret once read, so the disk is not read per record. */
let fileSecretRead: string | null = null;

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

/** The salt every key was derived with until an operator set their own; changing it would orphan existing data. */
const DEFAULT_SALT = 'oya-profile-kek-v1';

/** A key-encryption key and the id sealed buffers name it by. */
type Kek = {
  /** The AES-256 key. */
  key: Buffer;
  /** Its fingerprint, written after the version byte of every buffer it wraps. */
  id: Buffer;
};

/** Derived keys by secret and salt: scrypt is slow on purpose, so each is derived once. */
const derived = new Map<string, Kek>();

/** Always true now, kept because callers guard on it before storing credentials. */
export function haveSecret() {
  try {
    return !!(process.env.OYA_PROFILE_SECRET || fileSecret());
  } catch {
    return false;
  }
}

/** The KEK for a secret and salt, derived once and cached. */
function derive(secret: string, salt: string): Kek {
  const cacheKey = `${salt}\0${secret}`;
  if (!derived.has(cacheKey)) {
    // scrypt is deliberate: the secret may be operator-chosen rather than random.
    // N=2^15,r=8 needs 32MB, exactly Node's default maxmem ceiling, so maxmem is
    // raised explicitly rather than left to trip at runtime.
    const key = scryptSync(secret, salt, KEY_BYTES, { N: SCRYPT_N, r: SCRYPT_R, p: 1, maxmem: SCRYPT_MAXMEM });
    derived.set(cacheKey, { key, id: createHash('sha256').update(key).digest().subarray(0, KEY_ID_BYTES) });
  }
  return derived.get(cacheKey);
}

/** The key new records are sealed under: OYA_PROFILE_SECRET, else the generated file secret. */
function current(): Kek {
  const salt = process.env.OYA_PROFILE_SALT || DEFAULT_SALT;
  return derive(process.env.OYA_PROFILE_SECRET || fileSecret(), salt);
}

/** The outgoing key during a rotation, or null when none is configured. */
function previous(): Kek | null {
  const secret = process.env.OYA_PROFILE_SECRET_PREVIOUS;
  const salt = process.env.OYA_PROFILE_SALT_PREVIOUS || process.env.OYA_PROFILE_SALT || DEFAULT_SALT;
  return secret ? derive(secret, salt) : null;
}

/**
 * A key for one purpose (signing the audit chain, say), derived with HKDF so
 * it never equals a KEK. It comes from the oldest key held, the previous one
 * during a rotation, so moving the secret to OYA_PROFILE_SECRET_PREVIOUS
 * leaves it unchanged. Dropping the previous secret would change it: a
 * deployment that rotates sets an explicit key for that purpose first.
 */
export function derivedKey(purpose: string) {
  return Buffer.from(hkdfSync('sha256', (previous() ?? current()).key, Buffer.alloc(0), purpose, KEY_BYTES));
}

/** Every key a record may have been sealed under, current first. */
const keyring = (): Kek[] => [current(), previous()].filter(Boolean);

/** @returns {Buffer} version 1 | wrapNonce ... or, with OYA_SEAL_KEY_IDS, version 2 | keyId | wrapNonce | wrapTag | wrappedDek | nonce | tag | body */
export function seal(scope, value) {
  return sealBytes(scope, Buffer.from(JSON.stringify(value), 'utf8'));
}

/** Seals raw bytes (a screenshot, a file) under the scope, with no JSON round trip; open with openBytes. */
export function sealBytes(scope: string, bytes: Buffer) {
  const dek = randomBytes(KEY_BYTES);
  const aad = Buffer.from(scope);
  const inner = encrypt(dek, aad, bytes);
  return Buffer.concat([wrapDek(current(), aad, dek), inner.nonce, inner.tag, inner.body]);
}

/** The header: version (and key id in the keyed format), and the data key wrapped under `kek`. */
function wrapDek(kek: Kek, aad: Buffer, dek: Buffer) {
  const outer = encrypt(kek.key, aad, dek);
  const version = sealKeyIds() ? [Buffer.from([SEALED_KEYED_VERSION]), kek.id] : [Buffer.from([SEALED_VERSION])];
  return Buffer.concat([...version, outer.nonce, outer.tag, outer.body]);
}

/** AES-256-GCM under `k` with the scope as AAD: a fresh nonce, the tag and the ciphertext. */
function encrypt(k, aad, plaintext) {
  const nonce = randomBytes(NONCE_BYTES);
  const c = createCipheriv('aes-256-gcm', k, nonce);
  c.setAAD(aad);
  const body = Buffer.concat([c.update(plaintext), c.final()]);
  return { nonce, tag: c.getAuthTag(), body };
}

/** Byte lengths of the sealed fields after the header, in order; the body is the rest. */
const SEALED_LAYOUT = [NONCE_BYTES, TAG_BYTES, KEY_BYTES, NONCE_BYTES, TAG_BYTES];

/** Whether a buffer is in a sealed format rather than plaintext written before sealing (a JPEG, JSON). */
export const isSealed = (buf: Buffer) => buf[0] === SEALED_VERSION || buf[0] === SEALED_KEYED_VERSION;

/** Opens a sealed buffer under the same scope; throws if the scope differs or the data was altered. */
export function open(scope, buf) {
  return JSON.parse(openBytes(scope, buf).toString('utf8'));
}

/** Opens a sealBytes buffer to its bytes; throws if the scope differs, the key is unknown or the data was altered. */
export function openBytes(scope: string, buf: Buffer) {
  const aad = Buffer.from(scope);
  const p = unpack(buf);
  const dek = unwrapDek(p, aad);
  return decrypt(dek, aad, p.nonce, p.tag, p.body);
}

/** The data key, unwrapped by the key the buffer names, or by each held key for the unkeyed format. */
function unwrapDek(p, aad: Buffer) {
  const candidates = p.keyId ? keyring().filter((k) => k.id.equals(p.keyId)) : keyring();
  if (!candidates.length) throw new Error('Sealed under a key this server does not hold');
  return firstOpening(candidates, (kek) => decrypt(kek.key, aad, p.wrapNonce, p.wrapTag, p.wrapped));
}

/** The first key's answer that opens; an unkeyed record from before a rotation may be under the previous key. */
function firstOpening(keys: Kek[], unwrap: (kek: Kek) => Buffer) {
  for (const kek of keys.slice(0, -1)) {
    try {
      return unwrap(kek);
    } catch {
      // Not this key; try the next.
    }
  }
  return unwrap(keys.at(-1));
}

/** Split a sealed buffer into its fields; the key id is null for the unkeyed format. */
function unpack(buf: Buffer) {
  if (!isSealed(buf)) throw new Error('Unsupported sealed format');
  const keyed = buf[0] === SEALED_KEYED_VERSION;
  let o = keyed ? 1 + KEY_ID_BYTES : 1;
  const take = (n) => buf.subarray(o, (o += n));
  const [wrapNonce, wrapTag, wrapped, nonce, tag] = SEALED_LAYOUT.map(take);
  const keyId = keyed ? buf.subarray(1, 1 + KEY_ID_BYTES) : null;
  return { keyId, wrapNonce, wrapTag, wrapped, nonce, tag, body: buf.subarray(o) };
}

/** Open one AES-256-GCM layer; throws when the tag does not verify. */
function decrypt(k, aad, nonce, tag, data) {
  const d = createDecipheriv('aes-256-gcm', k, nonce);
  d.setAAD(aad);
  d.setAuthTag(tag);
  return Buffer.concat([d.update(data), d.final()]);
}

/**
 * The same record with its data key re-wrapped under the current key, for a
 * rotation: only the small outer layer is redone, the body is copied as is.
 * A record already under the current key comes back unchanged.
 */
export function rewrap(scope: string, buf: Buffer) {
  const p = unpack(buf);
  const kek = current();
  if (p.keyId?.equals(kek.id)) return buf;
  const aad = Buffer.from(scope);
  return Buffer.concat([wrapDek(kek, aad, unwrapDek(p, aad)), p.nonce, p.tag, p.body]);
}

/** Base64 for records that live in JSON rather than as raw files. */
export const sealText = (scope, value) => seal(scope, value).toString('base64');
/** Opens a sealText value. */
export const openText = (scope, text) => open(scope, Buffer.from(text, 'base64'));
/** rewrap() for a sealText value. */
export const rewrapText = (scope: string, text: string) =>
  rewrap(scope, Buffer.from(text, 'base64')).toString('base64');
