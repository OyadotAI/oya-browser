/** Verifies inherited product webhooks cannot make hermetic tests post to Slack. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

it('removes an inherited product webhook before non-live tests run', () => {
  const preload = fileURLToPath(new URL('../../support/hermetic.js', import.meta.url));
  const result = spawnSync(
    process.execPath,
    [
      '--import',
      preload,
      '--input-type=module',
      '-e',
      'process.stdout.write(String(process.env.SLACK_OPS_WEBHOOK_PRODUCT))',
    ],
    {
      encoding: 'utf8',
      env: { ...process.env, OYA_TEST_LIVE: '', SLACK_OPS_WEBHOOK_PRODUCT: 'https://hooks.example.test/inherited' },
    },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, 'undefined');
});
