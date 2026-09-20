/**
 * Unit tests for gateway WebSocket forwarding: an upgrade is left to this
 * replica unless another live replica owns the session, and a forged hop is
 * refused. The frame bridge itself needs real sockets and is not covered here.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../../support/data-dir.ts';

ownDataDir('oya-control-forward-gw-');
const { forwardGateway } = await import('../../../../../src/modules/control/cluster.ts');

let saved;
beforeEach(() => {
  saved = { url: process.env.OYA_INSTANCE_URL, secret: process.env.OYA_CLUSTER_SECRET };
  process.env.OYA_CLUSTER_SECRET = 'cluster-secret';
});
afterEach(() => {
  restoreEnv('OYA_INSTANCE_URL', saved.url);
  restoreEnv('OYA_CLUSTER_SECRET', saved.secret);
});

const upgrade = (headers = {}) => ({ headers, method: 'GET', url: '/gateway/g1' });

describe('forwardGateway', () => {
  it('leaves every upgrade to a replica running alone', async () => {
    delete process.env.OYA_INSTANCE_URL;
    assert.equal(await forwardGateway(upgrade(), null, null, 'token', 'key-a', 'g1'), false);
  });

  it('leaves an upgrade no other replica owns to this one', async () => {
    process.env.OYA_INSTANCE_URL = 'https://replica-1';
    assert.equal(await forwardGateway(upgrade(), null, null, 'token', 'key-a', 'g1'), false);
  });

  it('refuses a forged hop', async () => {
    process.env.OYA_INSTANCE_URL = 'https://replica-1';
    await assert.rejects(
      forwardGateway(upgrade({ 'x-oya-hop': `${Date.now()}.forged` }), null, null, 't', 'key-a', 'g1'),
      { status: 403, code: 'invalid_hop' },
    );
  });
});
