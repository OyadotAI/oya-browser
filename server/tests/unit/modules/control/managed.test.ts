/**
 * Unit tests for runtime selection: OYA_FLEET_RUNTIME picks the governed fleet's
 * runtime, an unknown one is refused, and cleanup follows the runtime recorded
 * on the session rather than whatever is configured now.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-control-managed-');
const { managedConfigured, removeManaged, verifyRuntime } = await import('../../../../src/modules/control/managed.ts');
const { cliCalls, fakeCli } = await import('../../support/control.ts');

const ENV = [
  'OYA_FLEET_RUNTIME',
  'OYA_MANAGED_NETWORK',
  'OYA_MANAGED_IMAGE',
  'OYA_MANAGED_CONTROL_URL',
  'OYA_MANAGED_PROXY_URL',
];
let saved;
beforeEach(() => {
  saved = Object.fromEntries(ENV.map((name) => [name, process.env[name]]));
  for (const name of ENV) delete process.env[name];
});
afterEach(() => {
  for (const name of ENV) restoreEnv(name, saved[name]);
});

describe('managed runtime selection', () => {
  it('is not configured until the runtime has its settings', () => {
    assert.equal(managedConfigured(), false);
    Object.assign(process.env, {
      OYA_MANAGED_NETWORK: 'n',
      OYA_MANAGED_IMAGE: 'i',
      OYA_MANAGED_CONTROL_URL: 'http://c',
      OYA_MANAGED_PROXY_URL: 'http://p',
    });
    assert.equal(managedConfigured(), true);
  });

  it('reports an unknown runtime as unconfigured, and refuses to verify it', () => {
    process.env.OYA_FLEET_RUNTIME = 'ecs';
    assert.equal(managedConfigured(), false);
    assert.throws(() => verifyRuntime(), { status: 422, code: 'runtime_unavailable', message: /known: docker, k8s/ });
  });

  it('verifies the Docker runtime by default', async () => {
    await assert.rejects(verifyRuntime(), { message: /managed Docker runtime/ });
    process.env.OYA_FLEET_RUNTIME = 'k8s';
    await assert.rejects(verifyRuntime(), { message: /managed Kubernetes runtime/ });
  });

  it('removes on the runtime the session was created on, Docker for old descriptors', async () => {
    process.env.OYA_FLEET_RUNTIME = 'k8s';
    const fake = fakeCli('docker', `() => ({ stderr: 'No such container', code: 1 })`, { readStdin: false });
    try {
      await removeManaged('c1', 'key-a', 'b1');
      assert.deepEqual((await cliCalls(fake.log))[0].args, ['inspect', 'c1']);
    } finally {
      fake.restore();
    }
  });
});
