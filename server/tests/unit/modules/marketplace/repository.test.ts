/** Real scratch SQLite verifies immutable buyer ownership across licenses, retries and restarts. */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createConnection, type Connection } from '../../../../src/platform/storage/index.ts';
import { MarketplaceRepository } from '../../../../src/modules/marketplace/repository.ts';

const BUYER = { accountId: '123456789012', licenseArn: 'arn:license:first', productCode: 'product-oya' };
const NOW = 1_800_000_000_000;
let db: Connection;
let repository: MarketplaceRepository;
beforeEach(async () => {
  db = createConnection();
  await db.delete('aws_marketplace_registrations', {});
  await db.delete('aws_marketplace_accounts', {});
  repository = new MarketplaceRepository(() => db);
});
afterEach(async () => {
  mock.restoreAll();
  await db.close();
});

describe('MarketplaceRepository', () => {
  it('resolves a two-user race to exactly one immutable owner', async () => {
    const answers = await Promise.allSettled(['alice', 'bob'].map((user) => repository.connect(BUYER, user, NOW)));
    assert.equal(answers.filter((answer) => answer.status === 'fulfilled').length, 1);
    const [row] = await db.select('aws_marketplace_accounts');
    const other = row.user_id === 'alice' ? 'bob' : 'alice';
    await assert.rejects(repository.connect({ ...BUYER, licenseArn: 'arn:license:second' }, other, NOW), {
      code: 'purchase_conflict',
    });
    assert.equal((await db.select('aws_marketplace_registrations')).length, 1);
  });

  it('can finish a partial write only as the original owner', async () => {
    const original = db.upsert.bind(db);
    let fail = true;
    mock.method(db, 'upsert', async (table, rows, options) => {
      if (table === 'aws_marketplace_registrations' && fail) {
        fail = false;
        throw new Error('storage unavailable');
      }
      return original(table, rows, options);
    });
    await assert.rejects(repository.connect(BUYER, 'alice', NOW), /storage unavailable/);
    await assert.rejects(repository.connect(BUYER, 'bob', NOW), { code: 'purchase_conflict' });
    await repository.connect(BUYER, 'alice', NOW);
    await repository.connect(BUYER, 'alice', NOW);
    assert.equal((await db.select('aws_marketplace_registrations')).length, 1);
  });

  it('preserves owner and registration after reopening the database', async () => {
    await repository.connect(BUYER, 'alice', NOW);
    await db.close();
    db = createConnection();
    const restored = new MarketplaceRepository(() => db);
    assert.equal(await restored.registered(BUYER, 'alice'), true);
    await assert.rejects(restored.registered(BUYER, 'bob'), { code: 'purchase_conflict' });
  });

  it('never overwrites a license identity and does not mutate on an unlinked status read', async () => {
    assert.equal(await repository.registered(BUYER, 'alice'), false);
    assert.equal((await db.select('aws_marketplace_accounts')).length, 0);
    await repository.connect(BUYER, 'alice', NOW);
    await assert.rejects(repository.connect({ ...BUYER, productCode: 'other' }, 'alice', NOW), {
      code: 'purchase_conflict',
    });
    assert.equal((await db.select('aws_marketplace_registrations'))[0].product_code, BUYER.productCode);
  });
});
