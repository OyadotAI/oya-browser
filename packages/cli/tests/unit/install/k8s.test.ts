/**
 * Unit tests for the Kubernetes fleet pieces that need no cluster
 * (src/install/k8s.ts): the NetworkPolicy and digest-pinned images.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { networkPolicy, pinImage } from '../../../src/install/k8s.ts';

describe('networkPolicy', () => {
  it('selects the managed browsers and allows only DNS and the control plane', () => {
    const policy = JSON.parse(networkPolicy('browsers', 'oya-governed-egress'));
    assert.deepEqual(policy.metadata, { name: 'oya-governed-egress', namespace: 'browsers' });
    assert.deepEqual(policy.spec.podSelector, { matchLabels: { app: 'oya-managed-browser' } });
    assert.deepEqual(policy.spec.policyTypes, ['Egress']);
    assert.deepEqual(
      policy.spec.egress[0].ports.map((p: { port: number }) => p.port),
      [53, 53],
    );
    assert.deepEqual(policy.spec.egress[1].to[0].podSelector, { matchLabels: { app: 'server' } });
  });
});

describe('pinImage', () => {
  it('leaves an already digest-pinned image alone', async () => {
    const ref = `ghcr.io/x/y@sha256:${'a'.repeat(64)}`;
    assert.equal(await pinImage(ref), ref);
  });
});
