/** Registration service tests prove resolved identity cannot imply confirmation or bypass product binding. */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { MarketplaceService } from '../../../../src/modules/marketplace/service.ts';

process.env.OYA_PROFILE_SECRET = 'marketplace-service-test-secret-only';
const BUYER = { accountId: '123456789012', licenseArn: 'arn:aws:license:test', productCode: 'p' };
const CONFIG = { productCode: 'p', region: 'us-east-1', origin: 'https://oya.example' };
const NOW = 1_800_000_000_000;

describe('MarketplaceService', () => {
  it('requires explicit linking and reports pending after durable persistence', async () => {
    let linked = false;
    const connect = mock.fn(async () => {
      linked = true;
    });
    const service = new MarketplaceService({
      config: () => CONFIG,
      resolve: async () => BUYER,
      now: () => NOW,
      repository: { connect, registered: async () => linked },
    });
    const cookie = await service.begin('aws-secret-token');
    const context = service.context(cookie);
    assert.equal(context.status, 'ready');
    assert.equal(context.awsAccount, '9012');
    assert.equal((await service.status(cookie, 'alice')).status, 'ready');
    assert.equal(connect.mock.callCount(), 0);
    assert.deepEqual(await service.connect(cookie, 'alice', context.contextId), { ...context, status: 'pending' });
    assert.equal(connect.mock.callCount(), 1);
    assert.deepEqual(connect.mock.calls[0].arguments, [BUYER, 'alice', NOW]);
  });

  it('rejects wrong products, malformed account IDs and missing current license identity', async () => {
    const repository = { connect: mock.fn(), registered: async () => false };
    for (const identity of [
      { ...BUYER, productCode: 'other' },
      { ...BUYER, accountId: 'bad' },
      { ...BUYER, licenseArn: undefined, CustomerIdentifier: 'legacy' },
    ]) {
      const service = new MarketplaceService({
        config: () => CONFIG,
        resolve: async () => identity,
        now: () => NOW,
        repository,
      });
      await assert.rejects(service.begin('token'), { code: 'invalid_setup' });
    }
    assert.equal(repository.connect.mock.callCount(), 0);
  });
});
