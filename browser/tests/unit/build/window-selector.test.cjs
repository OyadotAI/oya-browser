/** Packaging refuses missing/wrong-architecture native selectors while preserving other platform builds. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

/** Run the exact hook with filesystem/tool seams; no compiler, signing identity or platform dependency. */
function hookFixture() {
  const state = { exists: true, archs: 'x86_64 arm64', symbols: '_napi_register_module_v1', calls: [] };
  const context = { module: { exports: {} }, require: (name) => {
    if (name === 'node:fs') return { existsSync: () => state.exists };
    if (name === 'node:path') return path;
    if (name === 'builder-util') return { Arch: { 1: 'x64', 3: 'arm64', 4: 'universal' } };
    if (name === 'node:child_process') return { execFileSync: (tool, args) => {
      state.calls.push([tool, args]); return tool === 'lipo' ? state.archs : state.symbols;
    } };
    throw Error('Unexpected hook dependency');
  } };
  vm.runInNewContext(fs.readFileSync(path.resolve(__dirname, '../../../build/window-selector.cjs'), 'utf8'), context);
  const build = { electronPlatformName: 'darwin', appOutDir: '/owned/app', arch: 4, packager: { appInfo: { productFilename: 'Oya Browser' } } };
  return { state, build, run: context.module.exports };
}

test('the universal package requires both architecture slices and a Node-API entry point', async () => {
  const f = hookFixture();
  await f.run(f.build);
  assert.equal(f.state.calls[0][0], 'lipo');
  assert.match(f.state.calls[0][1][1], /app\.asar\.unpacked\/node_modules\/@oya\/window-selector\/build\/Release\/window_selector\.node$/);
  assert.equal(f.state.calls[1][0], 'nm');
});
test('a supported macOS package fails rather than silently omitting the selector', async () => {
  const f = hookFixture(); f.state.exists = false;
  await assert.rejects(f.run(f.build), /missing its native window selector/);
});
test('a one-architecture or wrong-entry-point universal binary fails the release gate', async () => {
  const f = hookFixture(); f.state.archs = 'arm64';
  await assert.rejects(f.run(f.build), /architecture is missing/);
  f.state.archs = 'arm64 x86_64'; f.state.symbols = '_unrelated_export';
  await assert.rejects(f.run(f.build), /no Node-API entry point/);
});
test('Windows and Linux packages do not try to inspect or load macOS artifacts', async () => {
  for (const platform of ['win32', 'linux']) {
    const f = hookFixture(); f.build.electronPlatformName = platform; f.state.exists = false;
    await f.run(f.build); assert.deepEqual(f.state.calls, []);
  }
});
test('native packaging composes with WebAuthn and excludes generated per-architecture caches', () => {
  const config = JSON.parse(fs.readFileSync(path.resolve(__dirname, '../../../package.json'), 'utf8')).build;
  assert.equal(config.beforePack, 'build/webauthn.cjs');
  assert.equal(config.afterPack, 'build/window-selector.cjs');
  assert.ok(config.files.includes('!node_modules/@oya/window-selector/bin/**/*'));
  assert.ok(config.asarUnpack.includes('node_modules/@oya/window-selector/build/Release/*.node'));
});
