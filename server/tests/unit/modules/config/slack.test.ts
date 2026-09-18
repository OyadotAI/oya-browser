/**
 * Unit tests for a key's stored Slack install: sealed, replaced on save,
 * forgotten on clear, and absent when it no longer unseals.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { getSlack, saveSlack, clearSlack } = await import('../../../../src/modules/config/slack.ts');
const { store, seal } = await import('../../../../src/modules/config/store.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

const KEY = 'config-slack-key';

describe('stored Slack install', () => {
  beforeEach(() => store.clear());

  it('has no install until one is saved', () => {
    assert.equal(getSlack(KEY), null);
  });

  it('saves an install sealed and reads it back', async () => {
    await saveSlack(KEY, { botToken: 'xoxb-secret', channelId: 'C1' });
    assert.deepEqual(getSlack(KEY), { botToken: 'xoxb-secret', channelId: 'C1' });
    assert.ok(!JSON.stringify(store.get(fingerprint(KEY))).includes('xoxb-secret'));
  });

  it('replaces an earlier install', async () => {
    await saveSlack(KEY, { botToken: 'a' });
    await saveSlack(KEY, { botToken: 'b' });
    assert.equal(getSlack(KEY).botToken, 'b');
  });

  it('forgets the install on clear', async () => {
    await saveSlack(KEY, { botToken: 'a' });
    await clearSlack(KEY);
    assert.equal(getSlack(KEY), null);
  });

  it('reads an install that no longer unseals as absent', () => {
    store.set(fingerprint(KEY), { _slack: seal('someone-else', { botToken: 'x' }) });
    assert.equal(getSlack(KEY), null);
  });
});
