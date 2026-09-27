/**
 * Unit tests for moving a playbook between environments: an export carries the
 * playbook and nothing of its history here, and an import refuses what this server
 * could not replay, or a name already taken unless asked to replace it.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const transfer = await import('../../../../src/modules/playbooks/transfer.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');

const FROM = 'transfer-from';
const TO = 'transfer-to';
const PB = {
  name: 'signup',
  prompt: 'Sign up {{email}}',
  steps: [
    { action: 'navigate', url: 'https://a.test/' },
    { action: 'type', text: '{{email}}', el: { name: 'email', tag: 'input' } },
  ],
  defaults: { email: 'ada@x.test' },
  secrets: ['pw'],
  createdAt: '2026-01-01T00:00:00.000Z',
  healedAt: '2026-01-02T00:00:00.000Z',
};

describe('playbook transfer', () => {
  beforeEach(async () => {
    keyConfig.reset();
    await keyConfig.savePlaybook(FROM, 'signup', structuredClone(PB));
  });

  it('exports the playbook itself, without its history here', () => {
    const doc = transfer.exportPlaybook(FROM, 'signup');
    assert.equal(doc.format, 'oya-playbook');
    assert.equal(doc.version, 1);
    assert.deepEqual(doc.playbook, {
      name: 'signup',
      prompt: PB.prompt,
      steps: PB.steps,
      defaults: PB.defaults,
      secrets: ['pw'],
    });
  });

  it('imports an export into another key, under a new name if given', async () => {
    const doc = transfer.exportPlaybook(FROM, 'signup');
    const described = await transfer.importPlaybook(TO, doc, { name: 'signup-prod' });
    assert.deepEqual(described.variables, ['email']);
    assert.equal(keyConfig.getPlaybook(TO, 'signup-prod').steps.length, 2);
  });

  it('refuses to replace a playbook unless asked to', async () => {
    const doc = transfer.exportPlaybook(FROM, 'signup');
    await assert.rejects(transfer.importPlaybook(FROM, doc), { status: 409 });
    await transfer.importPlaybook(FROM, doc, { overwrite: true });
  });

  it('refuses a document that is not an export, or a step this server cannot replay', async () => {
    await assert.rejects(transfer.importPlaybook(TO, { playbook: PB }), {
      status: 400,
      message: /Not an oya-playbook/,
    });
    const doc = transfer.exportPlaybook(FROM, 'signup');
    doc.playbook.steps.push({ action: 'rm_rf' });
    await assert.rejects(transfer.importPlaybook(TO, doc), { status: 400, message: /Step 3 is not a step/ });
  });

  it('refuses an unknown export version and a bad name', async () => {
    const doc = transfer.exportPlaybook(FROM, 'signup');
    await assert.rejects(transfer.importPlaybook(TO, { ...doc, version: 9 }), { status: 400 });
    await assert.rejects(transfer.importPlaybook(TO, doc, { name: '../x' }), { status: 400 });
  });

  it('says so when there is no playbook to export', () => {
    assert.throws(() => transfer.exportPlaybook(FROM, 'nope'), { status: 404 });
  });
});
