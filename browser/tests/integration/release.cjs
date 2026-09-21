/**
 * The release wait guard (k8s/scripts/wait-release.sh): it waits through a
 * draft that is still uploading, and refuses to ship older downloads.
 */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-release-test-'));
try {
  fs.writeFileSync(
    path.join(dir, 'gh'),
    `#!/bin/bash
n=$(cat "$TEST_COUNT" 2>/dev/null || echo 0)
n=$((n+1)); echo "$n" > "$TEST_COUNT"
[ "$TEST_MODE" = missing ] && exit 1
if [ "$n" -lt 3 ]; then echo true; else echo false; fi
`,
    { mode: 0o755 },
  );
  const run = (mode) =>
    spawnSync('bash', [path.join(__dirname, '../../../k8s/scripts/wait-release.sh'), 'v1.0.81'], {
      encoding: 'utf8',
      env: {
        ...process.env,
        PATH: `${dir}:${process.env.PATH}`,
        TEST_COUNT: path.join(dir, mode),
        TEST_MODE: mode,
        RELEASE_WAIT_ATTEMPTS: '3',
        RELEASE_WAIT_SECONDS: '0',
      },
    });
  const published = run('published');
  assert.equal(published.status, 0, published.stderr);
  assert.equal(fs.readFileSync(path.join(dir, 'published'), 'utf8').trim(), '3', 'waits through the uploading draft');
  const missing = run('missing');
  assert.equal(missing.status, 1, 'unpublished release must fail rather than carry old binaries forward');
  assert.match(missing.stdout, /Refusing to ship older downloads/);
  console.log('release publication guard: ok');
} finally {
  fs.rmSync(dir, { recursive: true, force: true });
}
