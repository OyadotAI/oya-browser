/** Engine provisioning must reject missing pins and failed verification before publishing an executable. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, writeFileSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

/** Fake only external installation tools, so no test downloads or executes a browser. */
function fixture(t, overrides = {}) {
  const root = mkdtempSync(join(tmpdir(), 'oya-engine-setup-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const bin = join(root, 'bin');
  mkdirSync(bin);
  const output = join(root, 'env');
  const trace = join(root, 'trace');
  writeFileSync(output, '');
  writeFileSync(trace, '');
  for (const [name, body] of Object.entries({
    curl: 'exit 0',
    sha256sum: 'cat >/dev/null; exit "${CHECKSUM_STATUS:-0}"',
    tar: 'for arg in "$@"; do if [[ "$arg" == */distribution ]]; then touch "$arg/electron"; chmod +x "$arg/electron"; fi; done',
    'xvfb-run': 'exit "${PROBE_STATUS:-0}"',
  })) {
    writeFileSync(join(bin, name), '#!/bin/bash\necho ' + name + ' >> "$TRACE"\n' + body + '\n', { mode: 0o755 });
  }
  const result = spawnSync('bash', ['.github/actions/setup-native-engine/install.sh'], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: bin + ':' + process.env.PATH,
      RUNNER_OS: 'Linux',
      RUNNER_ARCH: 'X64',
      RUNNER_TEMP: root,
      GITHUB_ENV: output,
      TRACE: trace,
      OYA_ENGINE_URL: 'https://example.invalid/oya.tar.gz',
      OYA_ENGINE_SHA256: 'a'.repeat(64),
      ...overrides,
    },
  });
  return { ...result, exported: readFileSync(output, 'utf8'), calls: readFileSync(trace, 'utf8') };
}

test('missing pins, insecure URLs and wrong platforms fail before downloading', (t) => {
  for (const overrides of [
    { OYA_ENGINE_URL: '' },
    { OYA_ENGINE_URL: 'http://example.invalid/engine' },
    { OYA_ENGINE_SHA256: '' },
    { OYA_ENGINE_SHA256: 'not-a-digest' },
    { RUNNER_OS: 'macOS' },
    { RUNNER_ARCH: 'ARM64' },
  ]) {
    const result = fixture(t, overrides);
    assert.notEqual(result.status, 0);
    assert.equal(result.calls, '');
    assert.equal(result.exported, '');
  }
});

test('a checksum mismatch prevents extraction and native execution', (t) => {
  const result = fixture(t, { CHECKSUM_STATUS: '1' });
  assert.notEqual(result.status, 0);
  assert.equal(result.calls, 'curl\nsha256sum\n');
  assert.equal(result.exported, '');
});

test('a failed native capability probe never publishes an executable', (t) => {
  const result = fixture(t, { PROBE_STATUS: '1' });
  assert.notEqual(result.status, 0);
  assert.equal(result.calls, 'curl\nsha256sum\ntar\nxvfb-run\n');
  assert.equal(result.exported, '');
});

test('only successful checksum and capability verification publishes the engine', (t) => {
  const result = fixture(t);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.exported, /^OYA_NATIVE_ENGINE=.+\/distribution\/electron\n$/);
});
