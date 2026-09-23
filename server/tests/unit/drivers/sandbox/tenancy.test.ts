/**
 * Unit tests for whose runtime a key's cloud browsers run on: the deployment's
 * by default, the one the key picks, and the key's own account only when it
 * brings that runtime's credentials, with none of the operator's mixed in.
 */
import { describe, it, before, beforeEach, after } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-tenancy-');
/** The deployment's own settings: Daytona by default, and an ECS cluster of its own. */
const HOST = {
  OYA_CLOUD_API_KEY: 'operator-daytona',
  OYA_CLOUD_SNAPSHOT: 'snap',
  OYA_CLOUD_IMAGE: 'oya/browser:1',
  OYA_PUBLIC_WS_URL: 'wss://oya.example/ws',
  OYA_ECS_CLUSTER: 'operator-cluster',
  AWS_SECRET_ACCESS_KEY: 'operator-aws-secret',
};
const saved = Object.fromEntries(Object.keys(HOST).map((name) => [name, process.env[name]]));
after(() => Object.entries(saved).forEach(([name, value]) => restoreEnv(name, value)));

const keyConfig = await import('../../../../src/modules/config/service.ts');
const { SANDBOX_RUNTIMES } = await import('../../../../src/modules/config/fields.ts');
const { sandboxEnv } = await import('../../../../src/drivers/sandbox/tenancy.ts');
const { WORKERS } = await import('../../../../src/drivers/sandbox/worker.ts');
const { settings } = await import('../../../../src/drivers/sandbox/config.ts');

const { ecsExternalId } = await import('../../../../src/drivers/sandbox.ts');

/** A key's own ECS setting, on an access key. */
const TENANT_ECS = {
  cluster: 'tenant-cluster',
  taskDefinition: 'oya-browser',
  subnets: ['subnet-01'],
  region: 'eu-west-1',
  auth: { type: 'iam', accessKeyId: 'AKIAABCDEFGHIJKLMNOP', secretAccessKey: 'tenant-secret' },
};

before(() => Object.assign(process.env, HOST));
beforeEach(() => keyConfig.reset());

describe('sandboxEnv', () => {
  it('runs a key that chose nothing on the deployment’s runtime', () => {
    assert.equal(settings(sandboxEnv('k1'))!.runtime, 'daytona');
    assert.equal(sandboxEnv(), process.env);
  });

  it('runs a key that picks Docker on the operator’s Docker, as the operator set it up', async () => {
    await keyConfig.set('k1', { sandbox_runtime: 'docker' });
    assert.deepEqual(settings(sandboxEnv('k1'))!.image, 'oya/browser:1');
  });

  it('runs a key with its own AWS keys on its own account, with none of the operator’s ECS or AWS settings', async () => {
    await keyConfig.set('k1', { sandbox_runtime: 'ecs', ecs: TENANT_ECS });
    const env = sandboxEnv('k1');
    assert.equal(env.OYA_ECS_CLUSTER, 'tenant-cluster', 'the operator’s cluster is never used with the tenant’s keys');
    assert.equal(env.OYA_CLOUD_API_KEY, undefined);
    assert.equal(env.OYA_PUBLIC_WS_URL, HOST.OYA_PUBLIC_WS_URL);
    assert.deepEqual(settings(env)!.auth, {
      type: 'iam',
      accessKeyId: 'AKIAABCDEFGHIJKLMNOP',
      secretAccessKey: 'tenant-secret',
    });
  });

  it('assumes a key’s role with Oya’s ExternalId for that key, never another', async () => {
    const auth = { type: 'role', roleArn: 'arn:aws:iam::123456789012:role/oya' };
    await keyConfig.set('k1', { sandbox_runtime: 'ecs', ecs: { ...TENANT_ECS, auth } });
    await keyConfig.set('k2', { sandbox_runtime: 'ecs', ecs: { ...TENANT_ECS, auth } });
    const [one, two] = [settings(sandboxEnv('k1'))!.auth, settings(sandboxEnv('k2'))!.auth];
    assert.equal(one.externalId, ecsExternalId('k1'));
    assert.notEqual(one.externalId, two.externalId);
    assert.equal(keyConfig.get('k1').ecs.externalId, ecsExternalId('k1'), 'shown, so the trust policy can name it');
  });

  it('runs a key’s own account on the deployment’s runtime without it choosing one', async () => {
    process.env.OYA_CLOUD_RUNTIME = 'ecs';
    try {
      await keyConfig.set('k1', { ecs: TENANT_ECS });
      assert.equal(sandboxEnv('k1').OYA_ECS_CLUSTER, 'tenant-cluster');
    } finally {
      delete process.env.OYA_CLOUD_RUNTIME;
    }
  });

  it('never runs a key on the operator’s AWS account with the key’s cluster', async () => {
    await keyConfig.set('k1', { sandbox_runtime: 'ecs' });
    assert.equal(sandboxEnv('k1').OYA_ECS_CLUSTER, 'operator-cluster');
  });

  it('uses the runtime a sandbox was made on over the key’s current choice', async () => {
    await keyConfig.set('k1', { sandbox_runtime: 'docker' });
    assert.equal(settings(sandboxEnv('k1', 'daytona'))!.runtime, 'daytona');
  });
});

describe('the ecs setting', () => {
  it('takes JSON text too, and refuses text that is not', async () => {
    await keyConfig.set('k1', { ecs: JSON.stringify(TENANT_ECS) });
    assert.equal(keyConfig.envFor('k1', {}).OYA_ECS_CLUSTER, 'tenant-cluster');
    await assert.rejects(keyConfig.set('k1', { ecs: '{nope' }), /ecs must be an object/);
  });

  it('reads back masked, keeps the stored secret when that is sent back, and clears with null', async () => {
    await keyConfig.set('k1', { ecs: TENANT_ECS });
    const view = keyConfig.get('k1').ecs;
    assert.ok(!JSON.stringify(view).includes('tenant-secret'));
    await keyConfig.set('k1', { ecs: { ...view, cluster: 'renamed' } });
    assert.equal(keyConfig.envFor('k1', {}).AWS_SECRET_ACCESS_KEY, 'tenant-secret');
    await keyConfig.set('k1', { ecs: null });
    assert.deepEqual(keyConfig.get('k1').ecs, { externalId: ecsExternalId('k1') });
  });
});

describe('sandbox_runtime', () => {
  it('offers exactly the runtimes there are', () => {
    assert.deepEqual(SANDBOX_RUNTIMES, Object.keys(WORKERS));
  });

  it('refuses a runtime that does not exist', async () => {
    await assert.rejects(keyConfig.set('k1', { sandbox_runtime: 'lambda' }), /sandbox_runtime must be one of/);
  });

  it('reads the tenant’s cloud credentials back masked', async () => {
    await keyConfig.set('k1', { ecs: TENANT_ECS, daytona_api_key: 'tenant-daytona' });
    const view = keyConfig.get('k1');
    assert.ok(!JSON.stringify(view).includes('tenant-secret'));
    assert.ok(!JSON.stringify(view).includes('tenant-daytona'));
  });
});
