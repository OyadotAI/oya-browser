/**
 * Unit tests for the kubectl runner, against a fake kubectl on PATH: stdout on
 * success, stderr on failure, manifests on stdin, and the configured context.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { kubectl } from '../../../../../src/modules/control/fleet/kubectl.ts';
import { cliCalls, fakeCli } from '../../../support/control.ts';
import { restoreEnv } from '../../../support/data-dir.ts';

let fake;
afterEach(() => {
  fake?.restore();
  restoreEnv('OYA_K8S_CONTEXT', undefined);
});

describe('kubectl', () => {
  it('resolves with stdout', async () => {
    fake = fakeCli('kubectl', `() => ({ stdout: 'namespace/x\\n' })`);
    assert.equal(await kubectl(['get', 'namespace', 'x']), 'namespace/x\n');
  });

  it('writes the manifest to stdin, never to argv', async () => {
    fake = fakeCli('kubectl', `() => ({})`);
    await kubectl(['apply', '-f', '-'], '{"secret":"s"}');
    const [call] = await cliCalls(fake.log);
    assert.deepEqual(call, { args: ['apply', '-f', '-'], stdin: '{"secret":"s"}' });
  });

  it('runs under OYA_K8S_CONTEXT when set', async () => {
    fake = fakeCli('kubectl', `() => ({})`);
    process.env.OYA_K8S_CONTEXT = 'prod';
    await kubectl(['get', 'pods']);
    assert.deepEqual((await cliCalls(fake.log))[0].args, ['--context', 'prod', 'get', 'pods']);
  });

  it('rejects with stderr and the exit code', async () => {
    fake = fakeCli('kubectl', `() => ({ stderr: 'Error: NotFound\\n', code: 1 })`);
    await assert.rejects(kubectl(['get', 'pod', 'x']), (e: any) => {
      assert.equal(e.message, 'Error: NotFound');
      assert.equal(e.code, 1);
      assert.equal(e.stderr, 'Error: NotFound\n');
      return true;
    });
  });

  it('names the exit code when kubectl says nothing', async () => {
    fake = fakeCli('kubectl', `() => ({ code: 3 })`);
    await assert.rejects(kubectl([]), /kubectl exited 3/);
  });

  it('rejects when kubectl cannot be started', async () => {
    const path = process.env.PATH;
    process.env.PATH = '/nonexistent';
    try {
      await assert.rejects(kubectl(['version']), { code: 'ENOENT' });
    } finally {
      process.env.PATH = path;
    }
  });
});
