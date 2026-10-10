/** Select only explicitly supplied, platform-specific Oya engines; never download a stock fallback. */
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { execFileSync } = require('node:child_process');
const { Arch } = require('builder-util');
/** A hung engine probe must fail packaging before artifacts can be published. */
const PROBE_TIMEOUT_MS = 30000;
/** Produce one concrete distribution per target, including both halves of a universal build. */
function distribution(platform, arch, root = process.env.OYA_NATIVE_DISTRIBUTIONS) {
  if (!root) throw Error('Set OYA_NATIVE_DISTRIBUTIONS to validated native Oya distributions before packaging');
  return path.resolve(root, platform + '-' + arch);
}
/** Electron-builder accepts an unpacked distribution, not just the executable within it. */
function executable(platform, directory) {
  const names = { darwin: 'Electron.app/Contents/MacOS/Electron', win32: 'electron.exe', linux: 'electron' };
  if (!Object.hasOwn(names, platform)) throw Error('Unsupported native Oya packaging platform');
  return path.join(directory, names[platform]);
}
/** Run capability checks against the very runtime being packaged, in a fresh temporary profile. */
function verify(platform, arch, directory, run = execFileSync) {
  if (platform !== process.platform) throw Error('Native Oya packaging must run on its target OS');
  const binary = executable(platform, directory);
  if (!fs.statSync(binary).isFile()) throw Error('Native Oya executable is missing');
  const output = probe(binary, run);
  const marker = 'OYA_NATIVE_PROBE ' + platform + ' ' + arch;
  if (!output.split(/\r?\n/).includes(marker)) throw Error('Native engine architecture or capability probe failed');
}
/** Validate before unpacking: electron-builder otherwise catches distribution-hook errors and downloads stock Electron. */
async function beforePack(context) {
  const platform = context.electronPlatformName;
  const arch = Arch[context.arch];
  const directory = distribution(platform, arch);
  verify(platform, arch, directory);
  context.packager.config.electronDist = directory;
  await require('./webauthn.cjs')(context);
}
/** The same selection is used by validation and unpacking; return no optional stock-engine path. */
function electronDist(options) {
  return distribution(options.platformName, typeof options.arch === 'number' ? Arch[options.arch] : options.arch);
}
module.exports = { distribution, executable, verify, beforePack, electronDist };

/** Parent owns cleanup after the engine has exited and released its database handles. */
function probe(binary, run) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-release-probe-'));
  try {
    return run(binary, [path.join(__dirname, 'native-probe.cjs')], {
      encoding: 'utf8', timeout: PROBE_TIMEOUT_MS,
      env: { ...process.env, ELECTRON_RUN_AS_NODE: '', OYA_RELEASE_PROBE_PROFILE: profile },
    });
  } finally { fs.rmSync(profile, { recursive: true, force: true }); }
}
