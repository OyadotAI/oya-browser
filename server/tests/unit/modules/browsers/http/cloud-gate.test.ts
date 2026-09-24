/**
 * Unit tests for the cloud gate without a database: with no database there are
 * no agent keys, so every key passes to the start, whichever provider it
 * asks for. (The refusal needs an agent row, and is checked against a real
 * database.)
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';
import { fakeRequest, runMiddleware } from '../../../support/auth.ts';

ownDataDir('oya-cloud-gate-');
const { cloudNeedsPerson } = await import('../../../../../src/modules/browsers/http/cloud-gate.ts');

/** A start request from `key` asking for `provider`. */
const start = (key: string, provider?: string) =>
  fakeRequest({
    method: 'POST',
    path: '/browsers/start',
    headers: { authorization: `Bearer ${key}` },
    body: { provider },
  });

describe('cloudNeedsPerson', () => {
  it('lets any key start a browser it brings itself', async () => {
    assert.equal((await runMiddleware(cloudNeedsPerson(), start('k'.repeat(32), 'cdp'))).passed, true);
  });

  it('lets a key that is not an unclaimed agent key start an Oya Cloud browser', async () => {
    assert.equal((await runMiddleware(cloudNeedsPerson(), start('k'.repeat(32), 'oya-cloud'))).passed, true);
    assert.equal((await runMiddleware(cloudNeedsPerson('oya-cloud'), start('k'.repeat(32)))).passed, true);
  });
});
