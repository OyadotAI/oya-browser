/**
 * Record tables: sealed values by id, stored through the configured driver,
 * replaced by id and removed by id.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-storage-records-');
const { RecordTable } = await import('../../../../src/platform/storage/index.ts');

describe('RecordTable', () => {
  it('stores records, replaces one by id, and loads them all', async () => {
    const table = new RecordTable('mfa_factors');
    await table.put([
      ['a', 'sealed-1'],
      ['b', 'sealed-2'],
    ]);
    await table.put([['a', 'sealed-3']]);
    assert.deepEqual([...(await table.load())].sort(), [
      ['a', 'sealed-3'],
      ['b', 'sealed-2'],
    ]);
  });

  it('removes only the given ids', async () => {
    const table = new RecordTable('persona_credentials');
    await table.put([
      ['p|a.com', 'x'],
      ['p|b.com', 'y'],
    ]);
    await table.remove(['p|a.com']);
    assert.deepEqual([...(await table.load()).keys()], ['p|b.com']);
  });

  it('writes nothing for no records', async () => {
    const table = new RecordTable('persona_logins');
    await table.put([]);
    assert.equal((await table.load()).size, 0);
  });
});
