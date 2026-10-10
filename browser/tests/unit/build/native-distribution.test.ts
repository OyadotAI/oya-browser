/** Packaging must select and verify the patched engine, with no stock-runtime fallback. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { Arch } from 'builder-util';
import { distribution, executable, verify, electronDist, beforePack } from '../../../build/native-distribution.cjs';

it('refuses an unspecified native distribution root', () => {
  assert.throws(() => distribution('darwin', 'arm64', ''), /OYA_NATIVE_DISTRIBUTIONS/);
});
it('isolates both macOS architectures and Windows distributions', () => {
  for (const [platform, arch] of [['darwin', 'arm64'], ['darwin', 'x64'], ['win32', 'x64'], ['linux', 'x64']])
    assert.equal(distribution(platform, arch, '/native'), path.resolve('/native', platform + '-' + arch));
});
it('resolves the actual executable rather than mistaking an app bundle for a binary', () => {
  assert.equal(executable('darwin', '/native'), path.join('/native', 'Electron.app/Contents/MacOS/Electron'));
  assert.equal(executable('win32', '/native'), path.join('/native', 'electron.exe'));
  assert.equal(executable('linux', '/native'), path.join('/native', 'electron'));
  assert.throws(() => executable('unknown', '/native'), /Unsupported/);
});
it('refuses unverified cross-OS packaging', () => {
  const other = process.platform === 'win32' ? 'darwin' : 'win32';
  assert.throws(() => verify(other, 'x64', '/missing'), /target OS/);
});
it('rejects a missing engine before spawning a probe', () => {
  assert.throws(() => verify(process.platform, process.arch, '/missing-oya-native-engine'), /ENOENT/);
});
it('requires the exact native success marker and architecture from the selected engine', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-dist-test-'));
  const binary = executable(process.platform, directory);
  fs.mkdirSync(path.dirname(binary), { recursive: true });
  fs.writeFileSync(binary, 'hermetic executable stand-in, never launched');
  try {
    const marker = 'OYA_NATIVE_PROBE ' + process.platform + ' ' + process.arch;
    verify(process.platform, process.arch, directory, (file, args, options) => {
      assert.equal(file, binary);
      assert.ok(args[0].endsWith('native-probe.cjs'));
      assert.ok(options.timeout > 0);
      return marker + '\n';
    });
    for (const output of ['', marker + '-wrong', 'OYA_NATIVE_PROBE other x64'])
      assert.throws(() => verify(process.platform, process.arch, directory, () => output), /probe failed/);
    assert.throws(() => verify(process.platform, process.arch, directory, () => { throw Error('timeout'); }), /timeout/);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});
it('before-pack fails before the builder can swallow a distribution-hook error', async () => {
  const previous = process.env.OYA_NATIVE_DISTRIBUTIONS;
  delete process.env.OYA_NATIVE_DISTRIBUTIONS;
  try {
    await assert.rejects(beforePack({ electronPlatformName: 'darwin', arch: Arch.arm64 }), /OYA_NATIVE_DISTRIBUTIONS/);
    assert.throws(() => electronDist({ platformName: process.platform, arch: process.arch }), /OYA_NATIVE_DISTRIBUTIONS/);
  } finally {
    if (previous !== undefined) process.env.OYA_NATIVE_DISTRIBUTIONS = previous;
  }
});
it('refuses Linux desktop packaging while retaining the Linux cloud engine probe', async () => {
  await assert.rejects(beforePack({ electronPlatformName: 'linux', arch: Arch.x64 }), /only macOS and Windows/);
  assert.equal(executable('linux', '/native'), path.join('/native', 'electron'));
});
