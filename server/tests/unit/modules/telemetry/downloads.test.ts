/**
 * Unit tests for counting /downloads: a person's installer, the app's update
 * check with its version, and the app fetching its update each count once the
 * response went out whole; a failed, partial or HEAD response and any other
 * file count nothing, and who asked is a fingerprint, never an address.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { trackDownloads } from '../../../../src/modules/telemetry/downloads.ts';
import { track } from '../../../../src/modules/telemetry/service.ts';
import { FakeResponse, fakeRequest } from '../../support/browsers.ts';

const BROWSER_UA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/140 Safari/537.36';
const UPDATER_UA = 'electron-builder';

/** What one request to /downloads counted: [event, visitor, props] per count. */
function served(path: string, { status = 200, method = 'GET', headers = {} as Record<string, string> } = {}) {
  const counted: unknown[][] = [];
  mock.method(track, 'downloadServed', (visitor, props) => counted.push(['download_served', visitor, props]));
  mock.method(track, 'updateChecked', (visitor, props) => counted.push(['update_checked', visitor, props]));
  const req = fakeRequest({
    key: null,
    headers: { 'x-forwarded-for': '203.0.113.9', ...headers },
    extra: { method, path },
  });
  const res = new FakeResponse();
  let passedOn = false;
  trackDownloads(req, res as any, () => (passedOn = true));
  res.statusCode = status;
  res.emit('finish');
  assert.equal(passedOn, true, 'the static files still answer');
  return counted;
}

describe('trackDownloads', () => {
  beforeEach(() => mock.restoreAll());
  afterEach(() => mock.restoreAll());

  it('counts a person downloading an installer, by platform and release', () => {
    const [[event, , props]] = served('/Oya.Browser-1.0.115-universal.dmg', { headers: { 'user-agent': BROWSER_UA } });
    assert.equal(event, 'download_served');
    assert.deepEqual(props, { platform: 'mac', version: '1.0.115', file_type: 'installer', via: 'web' });
    assert.equal(served('/Oya.Browser-1.0.115-x64.exe')[0][2].platform, 'windows');
    assert.equal(served('/Oya.Browser-1.0.115-x64.AppImage')[0][2].platform, 'linux');
  });

  it('counts the app fetching its own update as an update the updater asked for', () => {
    const [[, , props]] = served('/Oya.Browser-1.0.116-universal.zip', { headers: { 'user-agent': UPDATER_UA } });
    assert.deepEqual(props, { platform: 'mac', version: '1.0.116', file_type: 'update', via: 'updater' });
  });

  it('counts an update check with the version asking, and an old app as unknown', () => {
    const [[event, , props]] = served('/latest-mac.yml', { headers: { 'x-oya-version': '1.0.114' } });
    assert.deepEqual([event, props], ['update_checked', { platform: 'mac', from_version: '1.0.114' }]);
    assert.equal(served('/latest.yml')[0][2].from_version, 'unknown');
    assert.equal(
      served('/latest-linux.yml', { headers: { 'x-oya-version': '<script>' } })[0][2].from_version,
      'unknown',
    );
  });

  it('counts nothing for a missing file, a later range of a resumed download, or a HEAD', () => {
    assert.deepEqual(served('/Oya.Browser-1.0.115-universal.dmg', { status: 404 }), []);
    assert.deepEqual(
      served('/Oya.Browser-1.0.115-universal.dmg', { status: 206, headers: { range: 'bytes=5000-' } }),
      [],
    );
    assert.equal(
      served('/Oya.Browser-1.0.115-universal.dmg', { status: 206, headers: { range: 'bytes=0-' } }).length,
      1,
    );
    assert.deepEqual(served('/Oya.Browser-1.0.115-universal.dmg', { method: 'HEAD' }), []);
  });

  it('counts nothing for other files or a path that is not valid percent-encoding', () => {
    assert.deepEqual(served('/Oya.Browser-1.0.115-universal.dmg.blockmap'), []);
    assert.deepEqual(served('/notes.txt'), []);
    assert.deepEqual(served('/%E0%A4%A'), []);
  });

  it('names who asked by a fingerprint, the same for the same person, never their address', () => {
    const [[, first]] = served('/Oya.Browser-1.0.115-x64.exe', { headers: { 'user-agent': BROWSER_UA } });
    const [[, again]] = served('/Oya.Browser-1.0.115-x64.exe', { headers: { 'user-agent': BROWSER_UA } });
    assert.equal(first, again);
    assert.match(String(first), /^dl-[0-9a-f]+$/);
    assert.ok(!String(first).includes('203.0.113.9'));
  });
});
