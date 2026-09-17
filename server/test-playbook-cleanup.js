import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { cleanPlaybook } from './src/playbook-cleanup.js';

const original = {
  name: 'legacy', createdAt: '2026-09-01', prompt: 'Use {{promptOnly}}',
  labels: ['continue', 'shared', 'missing', 'secretLabel'], secrets: ['password', 'secretLabel'],
  defaults: { continue: 'Continue', shared: 'Shared value', hidden: 'background',
    patient: 'Ada', promptOnly: 'kept', orphan: 'unused', secretLabel: 'do not inline' },
  steps: [
    { action: 'navigate', url: 'https://example.com' },
    { action: 'type', el: { tag: 'input', type: 'hidden' }, text: '{{hidden}}' },
    { action: 'type', el: { tag: 'input', type: 'input', domId: 'ctl00BodycontentContentplaceholder1Ucpat', visible: false }, text: '{{patient}}' },
    { action: 'type', el: { tag: 'input', type: 'input' }, text: '{{shared}}' },
    { action: 'type', el: { tag: 'input', type: 'input' }, text: '{{password}}' },
    { action: 'click', el: { text: '{{continue}}', domId: 'submit' } },
    { action: 'click', el: { text: '{{shared}}' } },
    { action: 'click', el: { text: '{{missing}}' } },
    { action: 'click', el: { text: '{{secretLabel}}' } },
    { action: 'click', el: { text: '{{intentional}}' } },
  ],
};
const before = structuredClone(original);
const cleaned = cleanPlaybook(original);
assert.deepEqual(original, before, 'cleanup does not mutate its input');
assert.equal(cleaned.steps.length, original.steps.length - 1);
assert(cleaned.steps.some(step => step.el?.domId?.startsWith('ctl00')), 'opaque IDs and offscreen flags are never grounds for deletion');
assert.equal(cleaned.steps.find(step => step.el?.domId === 'submit').el.text, 'Continue');
assert.deepEqual(cleaned.defaults, { shared: 'Shared value', patient: 'Ada', promptOnly: 'kept', secretLabel: 'do not inline' });
assert.deepEqual(cleaned.labels, ['missing', 'secretLabel']);
assert.deepEqual(cleaned.secrets, ['password', 'secretLabel']);
assert(cleaned.steps.some(step => step.el?.text === '{{intentional}}'), 'explicit data-driven clicks without a legacy label marker survive');
assert.deepEqual(cleanPlaybook(cleaned), cleaned, 'cleanup is idempotent');
assert.deepEqual(cleanPlaybook({ name: 'malformed' }), { name: 'malformed' });
const draft = { ...original, name: 'legacy:draft', healedFrom: 3, healedAt: '2026-09-02' };
assert.equal(cleanPlaybook(draft).healedFrom, 2, 'draft progress accounts for removed steps');

const dir = await mkdtemp(join(tmpdir(), 'oya-playbook-cleanup-'));
process.env.OYA_DATA_DIR = dir;
process.env.OYA_PROFILE_SECRET = 'playbook-cleanup-test-secret';
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
const config = await import('./src/key-config.js');
const { openText } = await import('./src/secrets.js');
const { fingerprint } = await import('./src/audit.js');
try {
  await config.savePlaybook('alice', original.name, original);
  await config.savePlaybook('alice', draft.name, draft);
  await config.savePlaybook('bob', original.name, { ...original, defaults: { ...original.defaults, patient: 'Bob' } });
  const file = join(dir, 'key-settings.json');
  const rows = JSON.parse(await readFile(file, 'utf8'));
  rows[0][1]['_playbook:unreadable'] = 'unreadable sealed record';
  await writeFile(file, JSON.stringify(rows));
  config.reset();
  await config.restore();
  assert.deepEqual(config.getPlaybook('alice', 'legacy'), cleaned);
  assert.equal(config.getPlaybook('alice', 'legacy:draft').healedFrom, 2);
  assert.equal(config.getPlaybook('bob', 'legacy').defaults.patient, 'Bob');
  assert.equal(config.listPlaybooks('alice').length, 2, 'backups are not listed as playbooks');
  const saved = await readFile(file, 'utf8');
  const alice = new Map(JSON.parse(saved)).get(fingerprint('alice'));
  assert.equal(alice['_playbook:unreadable'], 'unreadable sealed record');
  assert.deepEqual(JSON.parse(openText(`key-settings:${fingerprint('alice')}`, alice['_playbook-backup:v1:legacy'])), original);
  assert(!saved.includes('Shared value'), 'both cleaned records and backups stay encrypted');
  assert.deepEqual(await config.cleanupPlaybooks(), { updated: 0, unreadable: 1 });
  assert.equal(await readFile(file, 'utf8'), saved, 'repeated migration does not rewrite records or backups');
  config.reset();
  await config.restore();
  assert.deepEqual(config.getPlaybook('alice', 'legacy'), cleaned, 'cleanup survives restart');
  console.log('playbook cleanup: legacy labels, proven hidden inputs, preserved uncertain fields, drafts, tenant isolation, encrypted backup and restart passed');
} finally {
  config.reset();
  await rm(dir, { recursive: true, force: true });
}
