/** A local experimental engine must not be signed with an expired, unrelated or device-local-only Apple profile. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { assertBrowserProfile, readBrowserProfile, validateBrowserProfileFile } from '../../../build/webauthn-profile.cjs';
import { configuredAccessGroup } from '../../../build/webauthn.cjs';

/** Only public profile fields are represented; no real Apple certificate or credential is used. */
const profile = () => ({
  expires: Date.parse('2040-01-01T00:00:00Z'), teams: ['ABCDEFGHIJ'], platforms: ['OSX'],
  entitlements: {
    'com.apple.application-identifier': 'ABCDEFGHIJ.ai.oya.browser',
    'com.apple.developer.team-identifier': 'ABCDEFGHIJ',
    'com.apple.developer.web-browser.public-key-credential': true,
    'keychain-access-groups': ['ABCDEFGHIJ.*'],
  },
});
const group = 'ABCDEFGHIJ.ai.oya.browser.webauthn', now = Date.parse('2026-11-02T00:00:00Z');

it('accepts only the matching macOS browser capability and exact or team-authorized keychain group', () => {
  const value = profile();
  assert.doesNotThrow(() => assertBrowserProfile(value, group, now));
  value.entitlements['keychain-access-groups'] = [group];
  assert.doesNotThrow(() => assertBrowserProfile(value, group, now));
});

it('ordinary Touch ID profiles and string-shaped capability flags cannot authorize a native browser build', () => {
  for (const capability of [undefined, false, 'true', 1]) {
    const value = profile();
    (value.entitlements as any)['com.apple.developer.web-browser.public-key-credential'] = capability;
    assert.throws(() => assertBrowserProfile(value, group, now), /browser public-key-credential/);
  }
});

it('rejects expired profiles, wrong apps or teams, non-macOS and unrelated keychain access', () => {
  const edits = [
    (value: any) => { value.expires = now; },
    (value: any) => { value.expires = NaN; },
    (value: any) => { value.teams = ['KLMNOPQRST']; },
    (value: any) => { value.platforms = ['iOS']; },
    (value: any) => { value.entitlements['com.apple.application-identifier'] = 'ABCDEFGHIJ.ai.oya.other'; },
    (value: any) => { value.entitlements['com.apple.developer.team-identifier'] = 'KLMNOPQRST'; },
    (value: any) => { value.entitlements['keychain-access-groups'] = ['*', 'KLMNOPQRST.*']; },
  ];
  for (const edit of edits) { const value = profile(); edit(value); assert.throws(() => assertBrowserProfile(value, group, now)); }
});

it('decodes only required public metadata with bounded shell-free native tools and never exposes raw errors', () => {
  const calls: unknown[][] = [], value = profile();
  const outputs = ['<plist>fixture</plist>', JSON.stringify(value.entitlements), JSON.stringify(value.teams), JSON.stringify(value.platforms), '2040-01-01T00:00:00Z'];
  const decoded = readBrowserProfile('/fixture/profile', (...args: unknown[]) => { calls.push(args); return outputs.shift(); });
  assert.deepEqual(decoded, value);
  assert.deepEqual((calls[0][1] as string[]), ['cms', '-D', '-i', '/fixture/profile']);
  assert.equal((calls[0][2] as any).timeout > 0, true);
  assert.equal((calls[1][2] as any).input, '<plist>fixture</plist>');
  assert.throws(() => readBrowserProfile('/fixture/profile', () => { throw Error('fixture-sensitive-detail'); }),
    (error: unknown) => error instanceof Error && !error.message.includes('fixture-sensitive-detail'));
});

it('native opt-in validates its supplied profile before returning a signing group while stock builds do not', (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'oya-browser-profile-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const file = join(directory, 'fixture.profile');
  writeFileSync(file, 'public fixture');
  let validations = 0;
  const validate = (path: string, expected: string) => { validations++; assert.equal(path, file); assert.equal(expected, group); };
  const env = { APPLE_TEAM_ID: 'ABCDEFGHIJ', OYA_WEBAUTHN_PROFILE: file };
  assert.equal(configuredAccessGroup(env, validate), group);
  assert.equal(validations, 0);
  assert.equal(configuredAccessGroup({ ...env, OYA_NATIVE_PASSKEYS: '1' }, validate), group);
  assert.equal(validations, 1);
  assert.doesNotThrow(() => validateBrowserProfileFile(file, group, profile, now, 'darwin'));
  assert.throws(() => validateBrowserProfileFile(file, group, profile, now, 'linux'), /requires macOS/);
});
