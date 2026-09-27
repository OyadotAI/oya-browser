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

  it('refuses a self-hosted server’s sixth cloud browser at once without a license, with who to write to', async () => {
    const { registry } = await import('../../../../../src/modules/browsers/registry.ts');
    const ids = ['c1', 'c2', 'c3', 'c4', 'c5'];
    ids.forEach((id) => registry.add(id, { apiKey: 'other', provider: 'oya-cloud', clientType: 'oya', name: id }));
    try {
      await assert.rejects(runMiddleware(cloudNeedsPerson('oya-cloud'), start('k'.repeat(32))), {
        status: 402,
        code: 'license_required',
        contact: 'sales@getoya.ai',
      });
      assert.equal((await runMiddleware(cloudNeedsPerson(), start('k'.repeat(32), 'cdp'))).passed, true);
    } finally {
      ids.forEach((id) => registry.remove(id));
    }
  });

  it('counts a provision’s whole batch against the cap', async () => {
    const batch = fakeRequest({
      method: 'POST',
      path: '/browsers/provision',
      headers: { authorization: `Bearer ${'k'.repeat(32)}` },
      body: { count: 6 },
    });
    await assert.rejects(runMiddleware(cloudNeedsPerson('oya-cloud'), batch), { code: 'license_required' });
  });

  it('lets a key that is not an unclaimed agent key start an Oya Cloud browser', async () => {
    assert.equal((await runMiddleware(cloudNeedsPerson(), start('k'.repeat(32), 'oya-cloud'))).passed, true);
    assert.equal((await runMiddleware(cloudNeedsPerson('oya-cloud'), start('k'.repeat(32)))).passed, true);
  });
});
