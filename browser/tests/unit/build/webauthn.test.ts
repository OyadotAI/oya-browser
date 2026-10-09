/** Release signing and runtime setup must use exactly the same macOS keychain group. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, rm, mkdtemp, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import beforePack, { accessGroup, configuredAccessGroup, keychainEntitlements, configureNativeInfo } from '../../../build/webauthn.cjs';

it('does not invent a keychain group for an unsigned development build', () => {
  assert.equal(accessGroup(''), '');
});
it('rejects malformed teams rather than injecting XML into signing entitlements', () => {
  for (const team of ['short', '<bad/>', 'abcdefghij', 'ABCDEFGHIJK']) assert.throws(() => accessGroup(team));
});
it('uses a stable group bound to the Oya application identifier', () => {
  assert.equal(accessGroup('ABCDEFGHIJ'), 'ABCDEFGHIJ.ai.oya.browser.webauthn');
});
it('adds the matching group only to the main macOS app, preserving helper entitlements', async () => {
  const old = process.env.APPLE_TEAM_ID;
  const previousProfile = process.env.OYA_WEBAUTHN_PROFILE;
  const temp = await mkdtemp(join(tmpdir(), 'oya-profile-test-'));
  process.env.OYA_WEBAUTHN_PROFILE = join(temp, 'fixture.provisionprofile');
  await writeFile(process.env.OYA_WEBAUTHN_PROFILE, 'unit-test placeholder, not a signing profile');
  process.env.APPLE_TEAM_ID = 'ABCDEFGHIJ';
  const options = { entitlements: 'before', entitlementsInherit: 'helpers', provisioningProfile: '' };
  try {
    await beforePack({ electronPlatformName: 'win32', packager: { platformSpecificBuildOptions: options } });
    assert.equal(options.entitlements, 'before');
    await beforePack({ electronPlatformName: 'darwin', packager: { platformSpecificBuildOptions: options } });
    const contents = await readFile(options.entitlements, 'utf8');
    assert.match(
      contents,
      /<key>keychain-access-groups<\/key><array><string>ABCDEFGHIJ.ai.oya.browser.webauthn<\/string><\/array>/,
    );
    assert.match(contents, /com.apple.security.device.audio-input/);
    assert.equal(options.entitlementsInherit, 'helpers');
    assert.equal(options.provisioningProfile, resolve(process.env.OYA_WEBAUTHN_PROFILE));
  } finally {
    if (options.entitlements !== 'before') await rm(dirname(options.entitlements), { recursive: true, force: true });
    await rm(temp, { recursive: true, force: true });
    if (previousProfile === undefined) delete process.env.OYA_WEBAUTHN_PROFILE;
    else process.env.OYA_WEBAUTHN_PROFILE = previousProfile;
    if (old === undefined) delete process.env.APPLE_TEAM_ID;
    else process.env.APPLE_TEAM_ID = old;
  }
});

it('stock builds do not request the experimental browser credential entitlement', () => {
  for (const value of ['', '0', 'true'])
    assert.doesNotMatch(keychainEntitlements('ABCDEFGHIJ.ai.oya.browser.webauthn', value), /public-key-credential/);
});
it('custom-engine opt-in grants browser credentials to the main app entitlement', () => {
  assert.match(
    keychainEntitlements('ABCDEFGHIJ.ai.oya.browser.webauthn', '1'),
    /<key>com.apple.developer.web-browser.public-key-credential<\/key><true\/>/,
  );
});

it('stock builds do not request phone Bluetooth metadata', () => {
  const options = { extendInfo: { existing: 'preserved' } };
  configureNativeInfo(options, '0');
  assert.deepEqual(options.extendInfo, { existing: 'preserved' });
});
it('native phone metadata preserves other platform usage descriptions', () => {
  const options = { extendInfo: { NSMicrophoneUsageDescription: 'Existing microphone consent' } };
  configureNativeInfo(options, '1');
  assert.deepEqual(options.extendInfo, {
    NSMicrophoneUsageDescription: 'Existing microphone consent',
    NSBluetoothAlwaysUsageDescription:
      'Oya Browser uses Bluetooth to verify nearby phones when signing in with passkeys.',
  });
});

it('native opt-in never adds macOS signing or Bluetooth fields to a Windows build', async () => {
  const previous = process.env.OYA_NATIVE_PASSKEYS;
  process.env.OYA_NATIVE_PASSKEYS = '1';
  const options = { entitlements: 'unchanged', extendInfo: { existing: 'unchanged' } };
  try {
    await beforePack({ electronPlatformName: 'win32', packager: { platformSpecificBuildOptions: options } });
    assert.deepEqual(options, { entitlements: 'unchanged', extendInfo: { existing: 'unchanged' } });
  } finally {
    if (previous === undefined) delete process.env.OYA_NATIVE_PASSKEYS;
    else process.env.OYA_NATIVE_PASSKEYS = previous;
  }
});

it('signed stock builds without a profile do not emit a restricted keychain group', () => {
  assert.equal(configuredAccessGroup({ APPLE_TEAM_ID: 'ABCDEFGHIJ' }), '');
});
it('native opt-in fails before packaging when the required provisioning profile is absent', () => {
  assert.throws(() => configuredAccessGroup({ APPLE_TEAM_ID: 'ABCDEFGHIJ', OYA_NATIVE_PASSKEYS: '1' }), /OYA_WEBAUTHN_PROFILE/);
});
it('missing profile files cannot enable native signing', () => {
  assert.throws(() => configuredAccessGroup({ APPLE_TEAM_ID: 'ABCDEFGHIJ', OYA_WEBAUTHN_PROFILE: '/nonexistent/oya-test.provisionprofile' }));
});
