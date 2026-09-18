/**
 * Unit tests for the Kubernetes fleet runtime, against a fake kubectl on PATH:
 * the image must be pinned to a digest, the namespace reachable and the
 * NetworkPolicy must select managed pods and restrict egress; pods are created
 * with their Secret and deleted only on the cluster that made them, only when
 * their annotations match the owner and session.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../../support/data-dir.ts';

ownDataDir('oya-control-fleet-k8s-');
const { control, hash } = await import('../../../../../src/modules/control/service.ts');
const k8s = await import('../../../../../src/modules/control/fleet/k8s.ts');
const { cliCalls, fakeCli } = await import('../../../support/control.ts');

const DIGEST = `img@sha256:${'a'.repeat(64)}`;
/** The fake kubectl: answers from FAKE_* variables the test sets. */
const KUBECTL = `(args) => {
  const e = process.env, a = args.join(' ');
  if (a.startsWith('get namespace kube-system')) return { stdout: e.FAKE_UID || 'uid-1' };
  if (a.startsWith('get namespace')) return e.FAKE_NO_NS ? { stderr: 'forbidden', code: 1 } : { stdout: 'namespace/x' };
  if (a.startsWith('get networkpolicy')) return e.FAKE_POLICY ? { stdout: e.FAKE_POLICY } : { stderr: 'NotFound', code: 1 };
  if (a.startsWith('get pod')) return e.FAKE_POD ? { stdout: e.FAKE_POD } : { stderr: 'Error from server (NotFound)', code: 1 };
  return {};
}`;
const ENV = [
  'OYA_MANAGED_NETWORK',
  'OYA_MANAGED_IMAGE',
  'OYA_MANAGED_CONTROL_URL',
  'OYA_MANAGED_PROXY_URL',
  'OYA_K8S_NAMESPACE',
  'FAKE_UID',
  'FAKE_NO_NS',
  'FAKE_POLICY',
  'FAKE_POD',
];
/** A policy that selects managed pods and restricts egress. */
const goodPolicy = { spec: { podSelector: { matchLabels: { app: 'oya-managed-browser' } }, policyTypes: ['Egress'] } };

let saved, fake;
beforeEach(() => {
  saved = Object.fromEntries(ENV.map((name) => [name, process.env[name]]));
  Object.assign(process.env, {
    OYA_MANAGED_NETWORK: 'browsers-egress',
    OYA_MANAGED_IMAGE: DIGEST,
    OYA_MANAGED_CONTROL_URL: 'http://control',
    OYA_MANAGED_PROXY_URL: 'http://egress:3128',
    FAKE_POLICY: JSON.stringify(goodPolicy),
  });
  fake = fakeCli('kubectl', KUBECTL);
});
afterEach(() => {
  fake.restore();
  for (const name of ENV) restoreEnv(name, saved[name]);
});

describe('k8s verifyRuntime', () => {
  it('returns the identity sessions are pinned to, including the cluster’s UID', async () => {
    const identity = await k8s.verifyRuntime();
    assert.equal(identity.id, 'k8s');
    assert.equal(identity.daemonId, 'uid-1');
    assert.equal(identity.imageId, DIGEST);
    assert.equal(identity.region, 'oya-browsers');
  });

  it('refuses an unconfigured runtime', async () => {
    delete process.env.OYA_MANAGED_NETWORK;
    assert.equal(k8s.configured(), false);
    await assert.rejects(k8s.verifyRuntime(), { status: 422, code: 'runtime_unavailable' });
  });

  it('refuses an image referenced by tag', async () => {
    process.env.OYA_MANAGED_IMAGE = 'img:latest';
    await assert.rejects(k8s.verifyRuntime(), { code: 'unsupported_image' });
  });

  it('refuses an unreachable namespace', async () => {
    process.env.FAKE_NO_NS = '1';
    await assert.rejects(k8s.verifyRuntime(), { code: 'runtime_unavailable', message: /Namespace oya-browsers/ });
  });

  it('refuses a missing policy, one that selects other pods, and one that leaves egress open', async () => {
    delete process.env.FAKE_POLICY;
    await assert.rejects(k8s.verifyRuntime(), { code: 'unsafe_network', message: /not found/ });
    process.env.FAKE_POLICY = JSON.stringify({ spec: { podSelector: { matchLabels: { app: 'web' } } } });
    await assert.rejects(k8s.verifyRuntime(), { code: 'unsafe_network', message: /does not select/ });
    process.env.FAKE_POLICY = JSON.stringify({ spec: { podSelector: {}, policyTypes: ['Ingress'] } });
    await assert.rejects(k8s.verifyRuntime(), { code: 'unsafe_network', message: /must restrict Egress/ });
  });
});

describe('k8s create', () => {
  it('applies the pod and its Secret in one List on stdin, after recording the cleanup', async () => {
    await control().reserve('key-a', { id: 'kb1', provider: 'oya-selfhosted', managed: true });
    await k8s.create({ apiKey: 'key-a', browserId: 'kb1', persona: 'p', name: 'n' });
    const apply = (await cliCalls(fake.log)).find((c) => c.args[0] === 'apply');
    const list = JSON.parse(apply.stdin);
    assert.deepEqual(
      list.items.map((d) => d.kind),
      ['Secret', 'Pod'],
    );
    assert.equal(apply.args.join(' ').includes('oya_'), false, 'no credential in argv');
    assert.deepEqual((await control().store.get('session', 'kb1')).cleanup, {
      kind: 'docker',
      runtime: 'k8s',
      container: 'oya-managed-kb1',
      namespace: 'oya-browsers',
      daemonId: 'uid-1',
    });
  });
});

describe('k8s remove', () => {
  /** A pod annotated for `apiKey` and session b1. */
  const pod = (apiKey = 'key-a') =>
    JSON.stringify({ metadata: { annotations: { 'ai.getoya.owner': hash(apiKey), 'ai.getoya.session': 'b1' } } });

  it('deletes the pod and its Secret in the namespace it was created in', async () => {
    process.env.FAKE_POD = pod();
    process.env.OYA_K8S_NAMESPACE = 'moved';
    await k8s.remove('oya-managed-b1', 'key-a', 'b1', 'uid-1', 'original');
    const del = (await cliCalls(fake.log)).find((c) => c.args[0] === 'delete');
    assert.deepEqual(del.args, [
      'delete',
      'pod',
      'oya-managed-b1',
      'secret',
      'oya-managed-b1-enroll',
      '-n',
      'original',
      '--ignore-not-found',
      '--wait=false',
    ]);
  });

  it('refuses to clean up on a different cluster', async () => {
    await assert.rejects(k8s.remove('p', 'key-a', 'b1', 'uid-other'), { status: 503, code: 'runtime_unavailable' });
  });

  it('refuses a pod another owner or session made', async () => {
    process.env.FAKE_POD = pod('key-b');
    await assert.rejects(k8s.remove('p', 'key-a', 'b1'), { code: 'ownership_mismatch' });
  });

  it('treats a missing pod as gone, deleting its Secret so it cannot outlive it', async () => {
    await k8s.remove('oya-managed-b1', 'key-a', 'b1');
    const calls = (await cliCalls(fake.log)).map((c) => c.args.slice(0, 3).join(' '));
    assert.ok(calls.includes('delete secret oya-managed-b1-enroll'));
    assert.equal(
      calls.some((c) => c.startsWith('delete pod')),
      false,
    );
  });
});
