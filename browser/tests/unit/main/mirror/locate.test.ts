/**
 * Unit tests for src/main/mirror/locate.ts parsing: the https default browser is read
 * out of the LaunchServices dump, and profiles come out of Local State with
 * the last-used one flagged, falling back to a plain Default when unreadable.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { bundleForHttps, profilesOf } from '../../../../src/main/mirror/locate.ts';

describe('bundleForHttps', () => {
  it('picks the handler that claims the https scheme', () => {
    const raw = `(
      { LSHandlerRoleAll = "com.apple.safari"; LSHandlerURLScheme = mailto; },
      { LSHandlerRoleAll = "com.google.chrome"; LSHandlerURLScheme = https; }
    )`;
    assert.equal(bundleForHttps(raw), 'com.google.chrome');
  });

  it('returns null when no handler claims https', () => {
    assert.equal(bundleForHttps('( { LSHandlerURLScheme = ftp; } )'), null);
  });
});

describe('profilesOf', () => {
  /** Writes a Local State file into a fresh temp user-data dir and returns the dir. */
  const withLocalState = (state: object) => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-locate-'));
    fs.writeFileSync(path.join(dir, 'Local State'), JSON.stringify(state));
    return dir;
  };

  it('lists every profile with its name and flags the last-used one', () => {
    const dir = withLocalState({
      profile: { last_used: 'Profile 1', info_cache: { Default: { name: 'You' }, 'Profile 1': { name: 'Work' } } },
    });
    assert.deepEqual(profilesOf(dir), [
      { dir: 'Default', name: 'You', lastUsed: false },
      { dir: 'Profile 1', name: 'Work', lastUsed: true },
    ]);
  });

  it('falls back to a single Default profile when Local State is missing', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-locate-'));
    assert.deepEqual(profilesOf(dir), [{ dir: 'Default', name: 'Default', lastUsed: true }]);
  });
});
