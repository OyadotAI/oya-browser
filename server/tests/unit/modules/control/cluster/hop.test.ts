/**
 * Unit tests for signed hops between replicas: an HMAC over time, method and
 * path under the cluster secret, fresh for 30 seconds, never forwarded twice.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { hopHeader, refuseRehop, retarget, verifyHop } from '../../../../../src/modules/control/cluster/hop.ts';
import { restoreEnv } from '../../../support/data-dir.ts';

const NOW = 1_700_000_000_000;
let saved;
beforeEach(() => {
  saved = process.env.OYA_CLUSTER_SECRET;
  process.env.OYA_CLUSTER_SECRET = 'cluster-secret';
  mock.timers.enable({ apis: ['Date'], now: NOW });
});
afterEach(() => {
  restoreEnv('OYA_CLUSTER_SECRET', saved);
  mock.timers.reset();
});

/** A request carrying `hop` for GET `url`. */
const request = (hop, url = '/api/browsers/b1', method = 'GET') => ({ headers: { 'x-oya-hop': hop }, method, url });

describe('verifyHop', () => {
  it('accepts a hop signed by a peer for this method and path', () => {
    assert.doesNotThrow(() => verifyHop(request(hopHeader('GET', '/api/browsers/b1'))));
  });

  it('lets a request without a hop header through', () => {
    assert.doesNotThrow(() => verifyHop({ headers: {} }));
  });

  it('refuses a hop older than 30 seconds, or from the future', () => {
    const hop = hopHeader('GET', '/api/browsers/b1');
    mock.timers.tick(30_001);
    assert.throws(() => verifyHop(request(hop)), { status: 403, message: 'Expired cluster request' });
    assert.throws(() => verifyHop(request(`${NOW + 120_000}.x`)), { message: 'Expired cluster request' });
    assert.throws(() => verifyHop(request('soon.x')), { message: 'Expired cluster request' });
  });

  it('refuses a hop signed for another path, another method, or another secret', () => {
    const hop = hopHeader('GET', '/api/browsers/b1');
    assert.throws(() => verifyHop(request(hop, '/api/browsers/b2')), { status: 403, code: 'invalid_hop' });
    assert.throws(() => verifyHop(request(hop, '/api/browsers/b1', 'POST')), { code: 'invalid_hop' });
    process.env.OYA_CLUSTER_SECRET = 'other';
    assert.throws(() => verifyHop(request(hop)), { code: 'invalid_hop' });
    assert.throws(() => verifyHop(request(`${NOW}`)), { code: 'invalid_hop' });
  });

  it('prefers the original URL when Express rewrote it', () => {
    const req = { ...request(hopHeader('GET', '/api/x?y=1'), '/x'), originalUrl: '/api/x?y=1' };
    assert.doesNotThrow(() => verifyHop(req));
  });
});

describe('hopHeader', () => {
  it('refuses to sign without a cluster secret', () => {
    delete process.env.OYA_CLUSTER_SECRET;
    assert.throws(() => hopHeader('GET', '/'), { status: 503, code: 'cluster_unconfigured' });
  });
});

describe('refuseRehop', () => {
  it('refuses to forward an already-forwarded request', () => {
    assert.throws(() => refuseRehop(request('1.x'), 'moved'), { status: 503, code: 'owner_changed', message: 'moved' });
    assert.doesNotThrow(() => refuseRehop({ headers: {} }, 'moved'));
  });
});

describe('retarget', () => {
  it('points the same path and query at the owner, minus the named credentials', () => {
    const target = retarget('https://replica-2:8443/base', '/api/live/b1?key=k&ticket=t&frame=2', ['key', 'ticket']);
    assert.equal(target.href, 'https://replica-2:8443/api/live/b1?frame=2');
  });
});
