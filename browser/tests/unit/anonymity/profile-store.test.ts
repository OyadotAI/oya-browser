/**
 * Unit tests for src/anonymity/profile-store.ts: profiles on disk, the active id,
 * and ids that could escape the folder.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { ProfileStore } from '../../../src/anonymity/profile-store.ts';

describe('ProfileStore', () => {
  let dir: string;
  beforeEach(() => (dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-profiles-'))));
  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  it('creates its folder and round-trips a saved profile', () => {
    const store = new ProfileStore(dir);
    assert.ok(fs.existsSync(path.join(dir, 'profiles')));
    const profile = { id: 'p_1', seed: 'x' };
    assert.equal(store.save(profile), profile);
    assert.deepEqual(store.get('p_1'), profile);
  });

  it('answers null for a missing or corrupt profile', () => {
    const store = new ProfileStore(dir);
    assert.equal(store.get('missing'), null);
    fs.writeFileSync(path.join(dir, 'profiles', 'bad.json'), '{');
    assert.equal(store.get('bad'), null);
  });

  it('refuses an id that could leave the profiles folder', () => {
    const store = new ProfileStore(dir);
    assert.throws(() => store.get('../secret'), /Invalid profile id/);
    assert.throws(() => store.save({ id: 'a/b' }), /Invalid profile id/);
  });

  it('remembers the active profile, null until one is set', () => {
    const store = new ProfileStore(dir);
    assert.equal(store.getActiveId(), null);
    store.setActiveId('p_1');
    assert.equal(new ProfileStore(dir).getActiveId(), 'p_1');
  });
});
