/** Keep the resolved graph outside the reviewed security advisories, including nested dev dependencies. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

/** Fixed releases for the November 2026 Dependabot snapshot and the additional npm Zod advisory. */
const patched = {
  minimatch: '11.0.1',
  '@hono/node-server': '2.1.3',
  hono: '4.13.11',
  qs: '6.16.1',
  'fast-uri': '3.1.9',
  'ip-address': '10.7.4',
  electron: '44.9.1',
  vite: '8.3.3',
  zod: '4.6.6',
};
/** Undici maintains separate supported major release lines. */
const undiciPatched = { 6: '6.29.1', 7: '7.30.1', 8: '8.12.2' };
/** The four independent installation roots checked by GitHub and CI. */
const directories = ['', 'server/', 'ui/', 'browser/'];
/** Only releases are accepted; a prerelease cannot silently satisfy a fixed security floor. */
const atLeast = (version, floor) =>
  /^\d+\.\d+\.\d+$/.test(version) && version.localeCompare(floor, 'en', { numeric: true }) >= 0;

for (const directory of directories) {
  test(`${directory || 'root/'} lockfile contains no affected security dependency versions`, () => {
    const { packages } = JSON.parse(readFileSync(new URL(`../${directory}package-lock.json`, import.meta.url)));
    for (const [location, entry] of Object.entries(packages)) {
      const name = location.split('node_modules/').at(-1);
      const major = entry.version?.split('.')[0];
      const floor = name === 'undici' ? undiciPatched[major] : patched[name];
      if (floor) assert.ok(atLeast(entry.version, floor), `${directory}${location}: ${entry.version} < ${floor}`);
      if (name === 'glob' && Number(major) >= 9) assert.ok(atLeast(entry.version, '14.0.2'), location);
    }
  });
}

test('every Mars TLS consumer and the actual vendor package use patched Rustls', () => {
  for (const file of ['Cargo.toml', 'crates/relay/Cargo.toml', 'crates/link/Cargo.toml']) {
    const manifest = readFileSync(new URL(`../mars/${file}`, import.meta.url), 'utf8');
    assert.match(manifest, /rustls = \{ version = "=0\.23\.46"/);
  }
  const vendor = readFileSync(new URL('../mars/vendor/rustls/Cargo.toml', import.meta.url), 'utf8');
  const lock = readFileSync(new URL('../mars/Cargo.lock', import.meta.url), 'utf8');
  assert.match(vendor, /name = "rustls"\nversion = "0\.23\.46"/);
  assert.match(lock, /name = "rustls"\nversion = "0\.23\.46"/);
});
