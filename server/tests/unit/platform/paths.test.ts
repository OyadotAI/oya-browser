/**
 * Unit tests for the path helpers: persistent state follows OYA_DATA_DIR, and
 * the repo folders are resolved from the server root.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { BROWSER_DIR, SERVER_ROOT, dataPath } from '../../../src/platform/paths.ts';

describe('paths', () => {
  it('puts state under OYA_DATA_DIR when it is set', () => {
    assert.equal(dataPath('a', 'b.json'), join(process.env.OYA_DATA_DIR, 'a', 'b.json'));
  });

  it('falls back to server/data when OYA_DATA_DIR is unset', () => {
    const saved = process.env.OYA_DATA_DIR;
    delete process.env.OYA_DATA_DIR;
    try {
      assert.equal(dataPath('x'), join(SERVER_ROOT, 'data', 'x'));
    } finally {
      process.env.OYA_DATA_DIR = saved;
    }
  });

  it('finds browser/ next to server/', () => {
    assert.equal(BROWSER_DIR, join(SERVER_ROOT, '..', 'browser'));
  });
});
