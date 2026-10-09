/** Build-time keychain group shared by the runtime and macOS signing entitlement. */
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
/** Refuse malformed signing identities instead of generating an unusable keychain group. */
function accessGroup(team = process.env.APPLE_TEAM_ID) {
  if (!team) return '';
  if (!/^[A-Z0-9]{10}$/.test(team)) throw new Error('APPLE_TEAM_ID must be a ten-character Apple team identifier');
  return `${team}.ai.oya.browser.webauthn`;
}
/** Never emit restricted entitlements without an explicitly supplied provisioning profile. */
function configuredAccessGroup(env = process.env) {
  if (!env.OYA_WEBAUTHN_PROFILE) {
    if (env.OYA_NATIVE_PASSKEYS === '1') throw new Error('Native passkeys require OYA_WEBAUTHN_PROFILE with matching Apple capabilities');
    return '';
  }
  if (!fs.statSync(env.OYA_WEBAUTHN_PROFILE).isFile()) throw new Error('OYA_WEBAUTHN_PROFILE must name a provisioning profile file');
  const group = accessGroup(env.APPLE_TEAM_ID);
  if (!group) throw new Error('A WebAuthn provisioning profile also requires APPLE_TEAM_ID');
  return group;
}
/** Experimental custom engines alone request Apple's browser credential entitlement. */
function keychainEntitlements(group, native = process.env.OYA_NATIVE_PASSKEYS) {
  const local = `<key>keychain-access-groups</key><array><string>${group}</string></array>`;
  if (native !== '1') return local;
  return `${local}<key>com.apple.developer.web-browser.public-key-credential</key><true/>`;
}
/** Explain Bluetooth's nearby-phone purpose; macOS still owns permission consent. */
function configureNativeInfo(options, native = process.env.OYA_NATIVE_PASSKEYS) {
  if (native !== '1') return;
  options.extendInfo = {
    ...options.extendInfo,
    NSBluetoothAlwaysUsageDescription:
      'Oya Browser uses Bluetooth to verify nearby phones when signing in with passkeys.',
  };
}
/** Write only the main app's restricted entitlements; helpers retain their ordinary permissions. */
function writeMainEntitlements(group) {
  const source = fs.readFileSync(path.join(__dirname, 'entitlements.mac.plist'), 'utf8');
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-webauthn-signing-'));
  const file = path.join(folder, 'entitlements.plist');
  fs.writeFileSync(file, source.replace('</dict>', `${keychainEntitlements(group)}\n</dict>`));
  return file;
}
/** Use the same team id electron-vite embeds; leave helper entitlements unchanged. */
async function beforePack(context) {
  if (context.electronPlatformName !== 'darwin') return;
  const group = configuredAccessGroup();
  if (!group) return;
  const options = context.packager.platformSpecificBuildOptions;
  options.entitlements = writeMainEntitlements(group);
  options.provisioningProfile = path.resolve(process.env.OYA_WEBAUTHN_PROFILE);
  configureNativeInfo(options);
}
module.exports = beforePack;
module.exports.accessGroup = accessGroup;
module.exports.keychainEntitlements = keychainEntitlements;
module.exports.configureNativeInfo = configureNativeInfo;

module.exports.configuredAccessGroup = configuredAccessGroup;
