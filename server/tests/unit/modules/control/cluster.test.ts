/**
 * Unit tests for cluster membership: this replica's routable origin, and the
 * instance record it advertises and renews; plus which replica owns a session.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-control-cluster-');
const { clusterOrigin, heartbeatInstance, ownerFor } = await import('../../../../src/modules/control/cluster.ts');
const { control, instanceId } = await import('../../../../src/modules/control/service.ts');
const { patchRow, putRow, readySession } = await import('../../support/control.ts');

const NOW = 1_700_000_000_000;
let saved;
beforeEach(() => {
  saved = { url: process.env.OYA_INSTANCE_URL, secret: process.env.OYA_CLUSTER_SECRET };
  process.env.OYA_CLUSTER_SECRET = 's';
  mock.timers.enable({ apis: ['Date'], now: NOW });
});
afterEach(() => {
  restoreEnv('OYA_INSTANCE_URL', saved.url);
  restoreEnv('OYA_CLUSTER_SECRET', saved.secret);
  mock.timers.reset();
});

describe('clusterOrigin', () => {
  it('is null for a replica running alone', () => {
    delete process.env.OYA_INSTANCE_URL;
    assert.equal(clusterOrigin(), null);
  });

  it('is the origin of OYA_INSTANCE_URL', () => {
    process.env.OYA_INSTANCE_URL = 'https://replica-1:8443/some/path';
    assert.equal(clusterOrigin(), 'https://replica-1:8443');
  });

  it('refuses a non-HTTP URL, embedded credentials, or a missing cluster secret', () => {
    for (const url of ['ws://replica-1', 'https://u:p@replica-1']) {
      process.env.OYA_INSTANCE_URL = url;
      assert.throws(() => clusterOrigin(), /requires an HTTP\(S\) URL and cluster secret/);
    }
    process.env.OYA_INSTANCE_URL = 'https://replica-1';
    delete process.env.OYA_CLUSTER_SECRET;
    assert.throws(() => clusterOrigin(), /cluster secret/);
  });
});

describe('heartbeatInstance', () => {
  /** One heartbeat, then the stored instance record. */
  const beat = async () => {
    await control().store.transact((tx) => heartbeatInstance(tx));
    return control().store.get('instance', instanceId);
  };

  it('advertises nothing when running alone', async () => {
    delete process.env.OYA_INSTANCE_URL;
    assert.equal(await beat(), null);
  });

  it('advertises this replica’s origin with a 30-second lease', async () => {
    process.env.OYA_INSTANCE_URL = 'https://replica-1';
    assert.deepEqual(await beat(), { id: instanceId, url: 'https://replica-1', leaseUntil: NOW + 30_000 });
  });

  it('writes nothing while the lease is fresh, and renews it once it runs low', async () => {
    process.env.OYA_INSTANCE_URL = 'https://replica-1';
    await beat();
    mock.timers.tick(5000);
    assert.equal((await beat()).leaseUntil, NOW + 30_000);
    mock.timers.tick(6000);
    assert.equal((await beat()).leaseUntil, NOW + 41_000);
  });
});

describe('ownerFor', () => {
  let n = 0,
    mine;
  beforeEach(async () => {
    mine = `mine-${n++}`;
    await readySession(control(), 'key-a', mine);
    await patchRow(control(), 'session', mine, { instance: 'replica-2', leaseUntil: NOW + 10_000 });
    await putRow(control(), 'instance', 'replica-2', { id: 'replica-2', url: 'https://r2', leaseUntil: NOW + 10_000 });
  });

  it('names the live replica that owns the key’s session', async () => {
    assert.equal((await ownerFor(mine, 'key-a')).url, 'https://r2');
  });

  it('serves here what is unknown, another project’s, or ours', async () => {
    assert.equal(await ownerFor('missing', 'key-a'), null);
    assert.equal(await ownerFor(mine, 'key-b'), null);
    await patchRow(control(), 'session', mine, { instance: instanceId });
    assert.equal(await ownerFor(mine, 'key-a'), null);
  });

  it('serves here a terminal session, one whose lease lapsed, or one whose owner is gone', async () => {
    mock.timers.tick(10_001);
    assert.equal(await ownerFor(mine, 'key-a'), null);
    mock.timers.setTime(NOW);
    await patchRow(control(), 'instance', 'replica-2', { leaseUntil: NOW - 1 });
    assert.equal(await ownerFor(mine, 'key-a'), null);
    await patchRow(control(), 'session', mine, { state: 'stopped' });
    assert.equal(await ownerFor(mine, 'key-a'), null);
  });

  it('routes a gateway attachment the same way', async () => {
    await putRow(control(), 'attachment', 'gw', {
      id: 'gw',
      project: control().projectIdFor('key-a'),
      instance: 'replica-2',
      state: 'ready',
      leaseUntil: NOW + 10_000,
    });
    assert.equal((await ownerFor('gw', 'key-a')).url, 'https://r2');
  });
});
