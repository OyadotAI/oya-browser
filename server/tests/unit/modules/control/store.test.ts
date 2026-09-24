/**
 * Unit tests for the control store entry point: with no Postgres
 * configured, one process-wide store on SQLite in the data directory.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-control-store-');
const { controlStore, SqliteBackend } = await import('../../../../src/modules/control/store.ts');

describe('controlStore', () => {
  it('opens one SQLite store, locked, in the data directory', () => {
    const store = controlStore();
    assert.equal(store, controlStore());
    assert.ok(store.backend instanceof SqliteBackend);
    assert.equal(store.serial, true);
    assert.ok(existsSync(join(dir, 'control.sqlite')));
    assert.ok(existsSync(join(dir, 'control.sqlite.lock')));
  });
});
