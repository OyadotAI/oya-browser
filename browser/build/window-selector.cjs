/** Fail a macOS package missing the exact-architecture native selector, after Electron's dependency rebuild. */
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { Arch } = require('builder-util');

/** Node-API binary stays unpacked so macOS can load and sign its nested Mach-O code. */
const BINARY = 'node_modules/@oya/window-selector/build/Release/window_selector.node';
/** Electron-builder names differ from Apple's architecture spelling. */
const ARCHITECTURES = { arm64: ['arm64'], x64: ['x86_64'], universal: ['arm64', 'x86_64'] };

/** Run after packaging/rebuild; beforePack remains the existing WebAuthn entitlement hook. */
async function afterPack(context) {
  if (context.electronPlatformName !== 'darwin') return;
  const app = `${context.packager.appInfo.productFilename}.app`;
  const file = path.join(context.appOutDir, app, 'Contents/Resources/app.asar.unpacked', BINARY);
  const architectures = ARCHITECTURES[Arch[context.arch]];
  if (!architectures || !fs.existsSync(file)) throw Error('macOS package is missing its native window selector');
  const present = execFileSync('lipo', ['-archs', file], { encoding: 'utf8' }).trim().split(/\s+/);
  if (architectures.some((arch) => !present.includes(arch))) throw Error('Window selector architecture is missing');
  const symbols = execFileSync('nm', ['-gU', file], { encoding: 'utf8' });
  if (!symbols.includes('_napi_register_module_v1')) throw Error('Window selector has no Node-API entry point');
}
module.exports = afterPack;
