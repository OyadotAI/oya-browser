/**
 * Legacy data files: read once into storage, then set aside as .imported and
 * never deleted, so an upgrade loses nothing and imports nothing twice.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { importLegacyFile } from '../../../../src/platform/storage/index.ts';

/** A path in a fresh directory. */
const fresh = (name: string) => join(mkdtempSync(join(tmpdir(), 'oya-legacy-')), name);

describe('importLegacyFile', () => {
  it('hands the file’s JSON to the loader, then keeps the file as .imported', async () => {
    const path = fresh('personas.json');
    writeFileSync(path, '[{"id":"a"}]');
    const seen = [];
    await importLegacyFile(path, async (data) => void seen.push(data));
    assert.deepEqual(seen, [[{ id: 'a' }]]);
    assert.equal(existsSync(path), false);
    assert.equal(readFileSync(`${path}.imported`, 'utf8'), '[{"id":"a"}]');
  });

  it('does nothing when there is no file', async () => {
    let called = false;
    await importLegacyFile(fresh('none.json'), async () => void (called = true));
    assert.equal(called, false);
  });

  it('leaves the file where it was when the load fails, so the next start tries again', async () => {
    const path = fresh('config.json');
    writeFileSync(path, '{}');
    await assert.rejects(
      importLegacyFile(path, async () => {
        throw new Error('storage down');
      }),
      /storage down/,
    );
    assert.equal(existsSync(path), true);
  });

  it('refuses a file it cannot parse rather than reading it as empty', async () => {
    const path = fresh('broken.json');
    writeFileSync(path, '{not json');
    await assert.rejects(
      importLegacyFile(path, async () => {}),
      SyntaxError,
    );
    assert.equal(existsSync(path), true);
  });
});
