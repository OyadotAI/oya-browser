/**
 * Unit tests for where the licensing module keeps its values.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-license-store-');
const store = await import('../../../../src/platform/license/repository.ts');

describe('license store', () => {
  it('answers null for a value never stored', async () => {
    assert.equal(await store.get('install_id'), null);
  });

  it('keeps a value and reads it back as it was', async () => {
    await store.set('install_id', 'abc');
    await store.set('revoked:L1', true);
    assert.equal(await store.get('install_id'), 'abc');
    assert.equal(await store.get('revoked:L1'), true);
  });
});
