const fs = require('node:fs');
const path = require('node:path');
const { randomBytes, createCipheriv, createDecipheriv } = require('node:crypto');
const { normalizeDraft } = require('./workflow.cjs');
class DraftStore {
  constructor(directory, safeStorage) { this.dir = directory; this.safe = safeStorage; fs.mkdirSync(directory, { recursive: true, mode: 0o700 }); }
  key() {
    if (this.secret) return this.secret;
    if (!this.safe.isEncryptionAvailable() || this.safe.getSelectedStorageBackend?.() === 'basic_text') throw new Error('Secure local storage is unavailable. Recording stays in memory; unlock your OS keychain to save it.');
    const file = path.join(this.dir, 'key');
    if (fs.existsSync(file)) this.secret = Buffer.from(this.safe.decryptString(fs.readFileSync(file)), 'base64');
    else { this.secret = randomBytes(32); fs.writeFileSync(file, this.safe.encryptString(this.secret.toString('base64')), { mode: 0o600, flag: 'wx' }); }
    return this.secret;
  }
  file(id) { if (!/^[\w-]{1,100}$/.test(id)) throw new Error('Invalid draft ID'); return path.join(this.dir, id + '.enc'); }
  save(raw) {
    const draft = normalizeDraft(raw); draft.phase = 'paused';
    const iv = randomBytes(12), cipher = createCipheriv('aes-256-gcm', this.key(), iv);
    const encrypted = Buffer.concat([cipher.update(JSON.stringify(draft)), cipher.final()]);
    const file = this.file(draft.id), tmp = file + '.tmp';
    const fd = fs.openSync(tmp, 'w', 0o600);
    try { fs.writeFileSync(fd, Buffer.concat([iv, cipher.getAuthTag(), encrypted])); fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
    fs.renameSync(tmp, file); return draft;
  }
  load(id) {
    const data = fs.readFileSync(this.file(id)), decipher = createDecipheriv('aes-256-gcm', this.key(), data.subarray(0, 12));
    decipher.setAuthTag(data.subarray(12, 28));
    return normalizeDraft(JSON.parse(Buffer.concat([decipher.update(data.subarray(28)), decipher.final()]).toString()));
  }
  list() { return fs.readdirSync(this.dir).filter(name => name.endsWith('.enc')).map(name => { try { const d = this.load(name.slice(0, -4)); return { id: d.id, name: d.name, updatedAt: d.updatedAt, steps: d.steps.length }; } catch { return { id: name.slice(0, -4), name: 'Draft needs recovery', error: true }; } }).sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0)); }
  remove(id) { fs.rmSync(this.file(id), { force: true }); }
}
module.exports = { DraftStore };
