/**
 * Unit tests for the stable download addresses: each platform redirects to its
 * newest installer, compared by version number rather than by name; a folder
 * with none, a missing folder and an unknown platform answer 404; the redirect
 * stays under /downloads so the file is still counted.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import express from 'express';
import { downloadRoutes, latestInstaller } from '../../../../src/modules/downloads/index.ts';

const dirs: string[] = [];

/** A scratch downloads folder holding empty files with these names. */
function folder(...names: string[]) {
  const dir = mkdtempSync(join(tmpdir(), 'oya-downloads-'));
  dirs.push(dir);
  for (const name of names) writeFileSync(join(dir, name), '');
  return dir;
}

/** GETs /download/<platform> from a throwaway app over `dir`; answers the status and Location. */
async function get(dir: string, platform: string) {
  const server = express().use('/download', downloadRoutes(dir)).listen(0);
  try {
    const port = (server.address() as any).port;
    const res = await fetch(`http://127.0.0.1:${port}/download/${platform}`, { redirect: 'manual' });
    return { status: res.status, location: res.headers.get('location') };
  } finally {
    server.close();
  }
}

describe('stable download addresses', () => {
  afterEach(() => dirs.splice(0).forEach((dir) => rmSync(dir, { recursive: true, force: true })));

  it('redirects each platform to its newest installer under /downloads', async () => {
    const dir = folder('Oya.Browser-1.0.222-universal.dmg', 'Oya.Browser-1.0.223-universal.dmg', 'Oya.Browser-1.0.223-x64.exe');
    assert.deepEqual(await get(dir, 'mac'), { status: 302, location: '/downloads/Oya.Browser-1.0.223-universal.dmg' });
    assert.deepEqual(await get(dir, 'windows'), { status: 302, location: '/downloads/Oya.Browser-1.0.223-x64.exe' });
  });

  it('compares versions as numbers, not as text', async () => {
    const dir = folder('Oya.Browser-1.0.9-universal.dmg', 'Oya.Browser-1.0.223-universal.dmg', 'Oya.Browser-1.0.30-universal.dmg');
    assert.equal(await latestInstaller(dir, 'mac'), 'Oya.Browser-1.0.223-universal.dmg');
  });

  it('ignores update archives, feeds and other platforms', async () => {
    const dir = folder('Oya.Browser-1.0.300-universal.zip', 'latest-mac.yml', 'Oya.Browser-1.0.1-universal.dmg');
    assert.equal(await latestInstaller(dir, 'mac'), 'Oya.Browser-1.0.1-universal.dmg');
    assert.equal(await latestInstaller(dir, 'windows'), null);
  });

  it('answers 404 when there is no installer, no folder, or no such platform', async () => {
    assert.equal((await get(folder(), 'mac')).status, 404);
    assert.equal((await get(join(tmpdir(), 'oya-no-such-folder'), 'windows')).status, 404);
    assert.equal((await get(folder('Oya.Browser-1.0.1-universal.dmg'), 'linux')).status, 404);
  });
});
