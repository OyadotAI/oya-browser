/**
 * Legacy playbook cleanup: cleanPlaybook drops proven-dead steps, labels and
 * defaults without touching uncertain ones, is pure and idempotent, and keeps
 * draft progress in step. Then the stored migration: encrypted backups,
 * unreadable records left alone, per-tenant isolation, survival across a
 * restart, and playbook rename.
 */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { cleanPlaybook } from '../../src/modules/playbooks/cleanup.ts';

const original = {
  name: 'legacy',
  createdAt: '2026-09-01',
  prompt: 'Use {{promptOnly}}',
  labels: ['continue', 'shared', 'missing', 'secretLabel'],
  secrets: ['password', 'secretLabel'],
  defaults: {
    continue: 'Continue',
    shared: 'Shared value',
    hidden: 'background',
    patient: 'Ada',
    promptOnly: 'kept',
    orphan: 'unused',
    secretLabel: 'do not inline',
  },
  steps: [
    { action: 'navigate', url: 'https://example.com' },
    { action: 'type', el: { tag: 'input', type: 'hidden' }, text: '{{hidden}}' },
    {
      action: 'type',
      el: { tag: 'input', type: 'input', domId: 'ctl00BodycontentContentplaceholder1Ucpat', visible: false },
      text: '{{patient}}',
    },
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
assert(
  cleaned.steps.some((step) => step.el?.domId?.startsWith('ctl00')),
  'opaque IDs and offscreen flags are never grounds for deletion',
);
assert.equal(cleaned.steps.find((step) => step.el?.domId === 'submit').el.text, 'Continue');
assert.deepEqual(cleaned.defaults, {
  shared: 'Shared value',
  patient: 'Ada',
  promptOnly: 'kept',
  secretLabel: 'do not inline',
});
assert.deepEqual(cleaned.labels, ['missing', 'secretLabel']);
assert.deepEqual(cleaned.secrets, ['password', 'secretLabel']);
assert(
  cleaned.steps.some((step) => step.el?.text === '{{intentional}}'),
  'explicit data-driven clicks without a legacy label marker survive',
);
assert.deepEqual(cleanPlaybook(cleaned), cleaned, 'cleanup is idempotent');
assert.deepEqual(cleanPlaybook({ name: 'malformed' }), { name: 'malformed' });
const draft = { ...original, name: 'legacy:draft', healedFrom: 3, healedAt: '2026-09-02' };
assert.equal(cleanPlaybook(draft).healedFrom, 2, 'draft progress accounts for removed steps');

const dir = await mkdtemp(join(tmpdir(), 'oya-playbook-cleanup-'));
process.env.OYA_DATA_DIR = dir;
process.env.OYA_PROFILE_SECRET = 'playbook-cleanup-test-secret';
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
const config = await import('../../src/modules/config/service.ts');
const { openText } = await import('../../src/platform/secrets.ts');
const { fingerprint } = await import('../../src/platform/audit.ts');
const { getConnection } = await import('../../src/platform/storage/index.ts');
/** Every stored settings row, in a fixed order, as one string to compare and search. */
const storedRows = async () =>
  JSON.stringify(
    (await getConnection().select('key_settings'))
      .map((r) => [r.owner, r.key, r.value, r.updated_at])
      .sort((a, b) => `${a[0]}|${a[1]}`.localeCompare(`${b[0]}|${b[1]}`)),
  );
try {
  await config.savePlaybook('alice', original.name, original);
  await config.savePlaybook('alice', draft.name, draft);
  await config.savePlaybook('bob', original.name, { ...original, defaults: { ...original.defaults, patient: 'Bob' } });
  await config.drain();
  await getConnection().upsert('key_settings', [
    { owner: fingerprint('alice'), key: '_playbook:unreadable', value: 'unreadable sealed record' },
  ]);
  config.reset();
  await config.restore();
  assert.deepEqual(config.getPlaybook('alice', 'legacy'), cleaned);
  assert.equal(config.getPlaybook('alice', 'legacy:draft').healedFrom, 2);
  assert.equal(config.getPlaybook('bob', 'legacy').defaults.patient, 'Bob');
  assert.equal(config.listPlaybooks('alice').length, 2, 'backups are not listed as playbooks');
  await config.drain();
  const saved = await storedRows();
  const aliceRows = await getConnection().select('key_settings', { owner: fingerprint('alice') });
  const alice = Object.fromEntries(aliceRows.map((r) => [r.key, r.value]));
  assert.equal(alice['_playbook:unreadable'], 'unreadable sealed record');
  assert.deepEqual(
    JSON.parse(openText(`key-settings:${fingerprint('alice')}`, alice['_playbook-backup:v1:legacy'])),
    original,
  );
  assert(!saved.includes('Shared value'), 'both cleaned records and backups stay encrypted');
  assert.deepEqual(await config.cleanupPlaybooks(), { updated: 0, unreadable: 1 });
  await config.drain();
  assert.equal(await storedRows(), saved, 'repeated migration does not rewrite records or backups');
  config.reset();
  await config.restore();
  assert.deepEqual(config.getPlaybook('alice', 'legacy'), cleaned, 'cleanup survives restart');

  // Rename carries the healed draft along and never lands on a name in use.
  const playbooks = await import('../../src/modules/playbooks/service.ts');
  await playbooks.rename('alice', 'legacy', 'renamed');
  assert.equal(config.getPlaybook('alice', 'legacy'), null);
  assert.equal(config.getPlaybook('alice', 'legacy:draft'), null);
  assert.equal(config.getPlaybook('alice', 'renamed').name, 'renamed');
  assert.equal(config.getPlaybook('alice', 'renamed:draft').healedFrom, 2);
  assert.equal(
    config.getPlaybook('bob', 'legacy').defaults.patient,
    'Bob',
    'one tenant renaming leaves the other alone',
  );
  await assert.rejects(() => playbooks.rename('alice', 'renamed', 'not a name'), /1-64 letters/);
  await assert.rejects(() => playbooks.rename('alice', 'gone', 'whatever'), /No playbook named gone/);
  await config.savePlaybook('alice', 'taken', cleaned);
  await assert.rejects(() => playbooks.rename('alice', 'renamed', 'taken'), /already exists/);
  assert.equal(config.getPlaybook('alice', 'renamed').name, 'renamed', 'a refused rename changes nothing');

  console.log(
    'playbook cleanup: rename, legacy labels, proven hidden inputs, preserved uncertain fields, drafts, tenant isolation, encrypted backup and restart passed',
  );
} finally {
  config.reset();
  await rm(dir, { recursive: true, force: true });
}
