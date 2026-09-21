/**
 * Repository for local workflow drafts: one AES-256-GCM encrypted file per
 * draft, keyed by a secret the OS keychain protects (Electron safeStorage).
 * Nothing is written in the clear; without secure storage, saving refuses.
 */
const fs = require('node:fs');
const path = require('node:path');
const { randomBytes, createCipheriv, createDecipheriv } = require('node:crypto');
const { normalizeDraft } = require('./workflow.cjs');
const { CRYPTO } = require('./constants.cjs');

/** Where the ciphertext starts: after the IV and the auth tag. */
const HEADER_BYTES = CRYPTO.IV_BYTES + CRYPTO.TAG_BYTES;

/** Writes an owner-only file and flushes it to disk before returning. */
function writeSynced(file, data) {
  const fd = fs.openSync(file, 'w', CRYPTO.FILE_MODE);
  try {
    fs.writeFileSync(fd, data);
    fs.fsyncSync(fd);
  } finally {
    fs.closeSync(fd);
  }
}

/** Encrypted drafts in one owner-only folder. */
class DraftStore {
  /** Stores drafts in `directory`, using `safeStorage` to protect the key. */
  constructor(directory, safeStorage) {
    /** The drafts folder. */
    this.dir = directory;
    /** Electron's safeStorage (or a stand-in with the same calls). */
    this.safe = safeStorage;
    fs.mkdirSync(directory, { recursive: true, mode: CRYPTO.DIR_MODE });
  }

  /** The data key: loaded from its keychain-encrypted file, or created on first use. */
  key() {
    if (this.secret) return this.secret;
    this.assertSecure();
    const file = path.join(this.dir, 'key');
    if (fs.existsSync(file)) this.secret = Buffer.from(this.safe.decryptString(fs.readFileSync(file)), 'base64');
    else this.secret = this.createKey(file);
    return this.secret;
  }

  /** Refuses when the OS keychain is unavailable or would store the key as plain text. */
  assertSecure() {
    if (this.safe.isEncryptionAvailable() && this.safe.getSelectedStorageBackend?.() !== 'basic_text') return;
    throw new Error(
      'Secure local storage is unavailable. Recording stays in memory; unlock your OS keychain to save it.',
    );
  }

  /** A fresh random key, written once (never over an existing file) through the keychain. */
  createKey(file) {
    const secret = randomBytes(CRYPTO.KEY_BYTES);
    const sealed = this.safe.encryptString(secret.toString('base64'));
    fs.writeFileSync(file, sealed, { mode: CRYPTO.FILE_MODE, flag: 'wx' });
    return secret;
  }

  /** The file for a draft id; an id that could leave the folder is refused. */
  file(id) {
    if (!/^[\w-]{1,100}$/.test(id)) throw new Error('Invalid draft ID');
    return path.join(this.dir, id + '.enc');
  }

  /** Normalizes, pauses and saves a draft atomically; returns what was saved. */
  save(raw) {
    const draft = normalizeDraft(raw);
    draft.phase = 'paused';
    const iv = randomBytes(CRYPTO.IV_BYTES);
    const cipher = createCipheriv('aes-256-gcm', this.key(), iv);
    const encrypted = Buffer.concat([cipher.update(JSON.stringify(draft)), cipher.final()]);
    this.writeAtomic(this.file(draft.id), Buffer.concat([iv, cipher.getAuthTag(), encrypted]));
    return draft;
  }

  /** Writes to a temporary file, syncs it, then renames it into place. */
  writeAtomic(file, data) {
    const tmp = file + '.tmp';
    writeSynced(tmp, data);
    fs.renameSync(tmp, file);
  }

  /** Decrypts and normalizes a draft; throws when it is missing, tampered with or unreadable. */
  load(id) {
    const data = fs.readFileSync(this.file(id));
    const decipher = createDecipheriv('aes-256-gcm', this.key(), data.subarray(0, CRYPTO.IV_BYTES));
    decipher.setAuthTag(data.subarray(CRYPTO.IV_BYTES, HEADER_BYTES));
    const plain = Buffer.concat([decipher.update(data.subarray(HEADER_BYTES)), decipher.final()]);
    return normalizeDraft(JSON.parse(plain.toString()));
  }

  /** A summary of one stored draft, or a recovery entry when it cannot be read. */
  summary(name) {
    const id = name.slice(0, -'.enc'.length);
    try {
      const d = this.load(id);
      return { id: d.id, name: d.name, updatedAt: d.updatedAt, steps: d.steps.length };
    } catch {
      return { id, name: 'Draft needs recovery', error: true };
    }
  }

  /** Every stored draft, most recently updated first. */
  list() {
    const files = fs.readdirSync(this.dir).filter((name) => name.endsWith('.enc'));
    return files.map((name) => this.summary(name)).sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0));
  }

  /** A stored draft's size on disk, in bytes. */
  size(id) {
    return fs.statSync(this.file(id)).size;
  }

  /** Deletes a draft; a missing one is not an error. */
  remove(id) {
    fs.rmSync(this.file(id), { force: true });
  }
}

module.exports = { DraftStore };
