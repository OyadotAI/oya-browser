/**
 * Unit tests for the Kubernetes objects of one governed browser: a DNS-safe
 * pod name, the Secret holding its credentials, and a locked-down pod that
 * only references that Secret.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { manifests, podName } from '../../../../../src/modules/control/fleet/k8s-manifests.ts';
import { hash } from '../../../../../src/modules/control/service.ts';

describe('podName', () => {
  it('lowercases and replaces what a DNS label cannot hold', () => {
    assert.equal(podName('Browser_1.X'), 'oya-managed-browser-1-x');
    assert.equal(podName('--a--'), 'oya-managed-a');
  });

  it('keeps long names within 63 characters, distinct by a digest of the real id', () => {
    const a = podName('x'.repeat(80) + 'a'),
      b = podName('x'.repeat(80) + 'b');
    assert.ok(a.length <= 63);
    assert.notEqual(a, b);
    assert.match(a, /-[0-9a-f]{12}$/);
  });
});

describe('manifests', () => {
  const [secret, pod] = manifests({
    pod: 'oya-managed-b1',
    ns: 'oya-browsers',
    image: 'img@sha256:abc',
    apiKey: 'key-a',
    browserId: 'b1',
    environment: { OYA_API_KEY: 'oya_cred', OYA_PERSONA: undefined, PORT: 3 },
  });

  it('puts the credentials in a Secret, every value a string', () => {
    assert.equal(secret.kind, 'Secret');
    assert.equal(secret.metadata.name, 'oya-managed-b1-enroll');
    assert.deepEqual(secret.stringData, { OYA_API_KEY: 'oya_cred', OYA_PERSONA: '', PORT: '3' });
  });

  it('gives the pod only a reference to the Secret', () => {
    const [container] = pod.spec.containers;
    assert.deepEqual(container.envFrom, [{ secretRef: { name: 'oya-managed-b1-enroll' } }]);
    assert.equal(JSON.stringify(pod).includes('oya_cred'), false);
  });

  it('never restarts the pod, and gives it no Kubernetes token and no privileges', () => {
    assert.equal(pod.spec.restartPolicy, 'Never');
    assert.equal(pod.spec.automountServiceAccountToken, false);
    assert.deepEqual(pod.spec.containers[0].securityContext, {
      allowPrivilegeEscalation: false,
      capabilities: { drop: ['ALL'] },
      seccompProfile: { type: 'RuntimeDefault' },
    });
  });

  it('records the full owner hash in an annotation and selection labels within 63 characters', () => {
    assert.equal(pod.metadata.annotations['ai.getoya.owner'], hash('key-a'));
    assert.equal(pod.metadata.annotations['ai.getoya.session'], 'b1');
    assert.equal(pod.metadata.labels.app, 'oya-managed-browser');
    for (const value of Object.values(pod.metadata.labels)) assert.ok(String(value).length <= 63);
  });
});
