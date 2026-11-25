/** Build the small stock-Electron native input adapter with Node headers and system AppKit only. */
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
/** N-API headers ship alongside the Node distribution installed by setup-node/Homebrew. */
function headers() {
  const folder = path.resolve(path.dirname(process.execPath), '../include/node');
  if (!fs.existsSync(path.join(folder, 'node_api.h'))) throw Error('Node N-API headers are required to build native browser input');
  return folder;
}
/** A universal addon is identical in both app slices; electron-builder signs its unpacked binary. */
function buildOwnedPointer() {
  if (process.platform !== 'darwin') return;
  const output = path.resolve(__dirname, '../out/native');
  fs.mkdirSync(output, { recursive: true });
  execFileSync('xcrun', ['clang++', '-arch', 'arm64', '-arch', 'x86_64', '-std=c++17', '-mmacosx-version-min=12.0', '-bundle', '-undefined', 'dynamic_lookup', '-fobjc-arc', '-framework', 'AppKit', '-I', headers(), path.resolve(__dirname, '../native/owned-pointer.mm'), '-o', path.join(output, 'owned-pointer.node')], { stdio: 'inherit' });
}
buildOwnedPointer();
