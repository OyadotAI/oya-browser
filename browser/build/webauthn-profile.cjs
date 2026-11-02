/** Fail an experimental browser-passkey build before signing when its Apple profile cannot authorize the app. */
const fs = require('node:fs');
const { execFileSync } = require('node:child_process');
/** Public provisioning metadata still has a bounded decoding cost. */
const PROFILE_MAX_BYTES = 1_048_576;
/** A keychain/security-tool failure must not leave packaging waiting indefinitely. */
const PROFILE_DECODE_TIMEOUT_MS = 10_000;
/** Only this restricted capability enables Apple's browser-specific credential provider. */
const BROWSER_CAPABILITY = 'com.apple.developer.web-browser.public-key-credential';

/** Keep raw security-tool output, certificate data and filenames out of failure messages. */
function requireProfile(condition, message) {
  if (!condition) throw new Error(message);
}
/** The requested group must belong to Oya's exact signing identity, never another app or wildcard. */
function identity(group) {
  requireProfile(/^[A-Z0-9]{10}\.ai\.oya\.browser\.webauthn$/.test(group), 'Invalid Oya WebAuthn signing group');
  const team = group.split('.')[0];
  return { team, appId: `${team}.ai.oya.browser` };
}
/** Decode only public metadata required for validation; never return the certificate payload. */
function readBrowserProfile(file, run = execFileSync) {
  try {
    const xml = run('/usr/bin/security', ['cms', '-D', '-i', file], toolOptions());
    return profileFields(xml, run);
  } catch {
    throw new Error('Could not verify the Apple browser-passkey profile. Supply a valid matching provisioning profile.');
  }
}
/** Extract selected fields because full plist-to-JSON conversion need not support certificate data values. */
function profileFields(xml, run) {
  return {
    entitlements: JSON.parse(field(xml, 'Entitlements', 'json', run)),
    teams: JSON.parse(field(xml, 'TeamIdentifier', 'json', run)),
    platforms: JSON.parse(field(xml, 'Platform', 'json', run)),
    expires: Date.parse(field(xml, 'ExpirationDate', 'raw', run).trim()),
  };
}
/** Native tools consume the decoded bytes on stdin, with no shell interpolation or temporary plaintext profile. */
function field(xml, key, format, run) {
  return run('/usr/bin/plutil', ['-extract', key, format, '-o', '-', '-'], { ...toolOptions(), input: xml });
}
/** Bound stdout and runtime for both native tools and keep process execution shell-free. */
function toolOptions() {
  return { encoding: 'utf8', timeout: PROFILE_DECODE_TIMEOUT_MS, maxBuffer: PROFILE_MAX_BYTES };
}
/** A profile's identity, capability and expiration must all authorize the same final app. */
function assertBrowserProfile(profile, group, now = Date.now()) {
  const expected = identity(group);
  requireProfile(profile && Number.isFinite(profile.expires) && profile.expires > now, 'The Apple browser-passkey profile has expired or has no valid expiration');
  requireProfile(Array.isArray(profile.teams) && profile.teams.length === 1 && profile.teams[0] === expected.team, 'The Apple browser-passkey profile belongs to a different team');
  requireProfile(Array.isArray(profile.platforms) && profile.platforms.includes('OSX'), 'The Apple browser-passkey profile does not authorize macOS');
  assertEntitlements(profile.entitlements, expected, group);
}
/** A valid Developer ID certificate or ordinary keychain profile does not grant browser credential access. */
function assertEntitlements(entitlements, expected, group) {
  requireProfile(entitlements && !Array.isArray(entitlements), 'The Apple browser-passkey profile has no valid entitlements');
  requireProfile(entitlements['com.apple.application-identifier'] === expected.appId && entitlements['com.apple.developer.team-identifier'] === expected.team, 'The Apple browser-passkey profile does not match Oya Browser');
  requireProfile(entitlements[BROWSER_CAPABILITY] === true, 'The Apple profile does not authorize the browser public-key-credential capability');
  requireProfile(groupAllowed(entitlements['keychain-access-groups'], group, expected.team), 'The Apple browser-passkey profile does not authorize the Oya keychain group');
}
/** Apple may authorize the exact group or the signing team's wildcard, never an unrestricted or other-team wildcard. */
function groupAllowed(groups, group, team) {
  return Array.isArray(groups) && groups.some((item) => item === group || item === `${team}.*`);
}
/** Only the explicitly opted-in macOS path invokes this gate; stock builds retain device-local Touch ID behavior. */
function validateBrowserProfileFile(file, group, read = readBrowserProfile, now = Date.now(), platform = process.platform) {
  requireProfile(platform === 'darwin', 'Experimental browser-passkey profile validation requires macOS');
  const stat = fs.statSync(file);
  requireProfile(stat.isFile() && stat.size > 0 && stat.size <= PROFILE_MAX_BYTES, 'The Apple browser-passkey profile is empty, oversized or not a regular file');
  assertBrowserProfile(read(file), group, now);
}
module.exports = { assertBrowserProfile, readBrowserProfile, validateBrowserProfileFile };
