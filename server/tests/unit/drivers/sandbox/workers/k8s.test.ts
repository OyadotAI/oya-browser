/**
 * Unit tests for the Kubernetes sandbox runtime against a fake kubectl:
 * the pod and the pod-owned Secret it creates, that the API key reaches the
 * cluster only on stdin, and finding, deleting and listing pods.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { fakeCli } from '../../../support/fake-cli.ts';
import * as k8s from '../../../../../src/drivers/sandbox/workers/k8s.ts';

const cli = fakeCli(
  'kubectl',
  `
const pod = { metadata: { uid: 'pod-uid', annotations: { 'oya-browser-id': 'b-1' } }, status: { phase: 'Running' } };
if (args[0] === 'create' && process.env.FAKE_SECRET_FAILS && stdin.includes('"Secret"')) fail('secrets is forbidden');
if (args[0] === 'create') reply(stdin.includes('"Pod"') ? pod : 'secret/created');
if (args[0] === 'get' && args[1] === 'pod') args[2] === 'missing' ? fail('Error from server (NotFound): pods "missing" not found') : args[2] === 'broken' ? fail('connection refused') : reply(pod);
if (args[0] === 'get' && args[1] === 'pods') reply({ items: [pod, { metadata: {}, status: {} }] });
if (args[0] === 'delete') reply('deleted');
fail('unhandled ' + args.join(' '));
`,
);

/** A configured deployment. */
const CONFIG = { image: 'oya/browser@sha256:abc', namespace: 'browsers' };
/** What the facade hands every runtime. */
const SPEC = {
  name: 'oya-browser-b-1',
  labels: { 'oya-browser': 'true', 'oya-owner': 'owner-tag', 'oya-browser-id': 'b-1', 'oya-name': 'My browser' },
  env: { OYA_API_KEY: 'secret-key', OYA_BROWSER_ID: 'b-1' },
  ttlMinutes: 60,
  lifetimeMinutes: 70,
};

beforeEach(() => cli.reset());

describe('k8s settings', () => {
  it('needs only the image, the namespace defaulting', () => {
    assert.equal(k8s.settings({}), null);
    assert.deepEqual(k8s.missing({}), ['OYA_CLOUD_IMAGE']);
    assert.deepEqual(k8s.settings({ OYA_CLOUD_IMAGE: 'i' }), { image: 'i', namespace: 'oya-browsers' });
    assert.equal(k8s.settings({ OYA_CLOUD_IMAGE: 'i', OYA_K8S_NAMESPACE: 'x' })!.namespace, 'x');
  });

  it('is never configured by a key', () => {
    assert.equal(k8s.ownAccount(), false);
  });
});

describe('k8s create', () => {
  it('creates a locked-down pod with a hard deadline, then a Secret the pod owns', async () => {
    assert.deepEqual(await k8s.create(CONFIG, SPEC), { id: 'pod-uid' });
    const [podCall, secretCall] = cli.calls();
    const pod = JSON.parse(podCall.stdin);
    assert.equal(pod.spec.activeDeadlineSeconds, 70 * 60);
    assert.equal(pod.spec.restartPolicy, 'Never');
    assert.equal(pod.spec.automountServiceAccountToken, false);
    assert.deepEqual(
      pod.metadata.labels,
      { 'oya-browser': 'true', 'oya-owner': 'owner-tag' },
      'no spaces in label values',
    );
    assert.equal(pod.metadata.annotations['oya-name'], 'My browser');
    const secret = JSON.parse(secretCall.stdin);
    assert.deepEqual(secret.metadata.ownerReferences[0], {
      apiVersion: 'v1',
      kind: 'Pod',
      name: SPEC.name,
      uid: 'pod-uid',
    });
    assert.equal(secret.stringData.OYA_API_KEY, 'secret-key');
  });

  it('keeps the API key off argv and out of the pod manifest', async () => {
    await k8s.create(CONFIG, SPEC);
    const [podCall, secretCall] = cli.calls();
    assert.ok(![...podCall.args, ...secretCall.args].join(' ').includes('secret-key'));
    assert.ok(!podCall.stdin.includes('secret-key'));
  });

  it('deletes the pod when its Secret cannot be created', async () => {
    process.env.FAKE_SECRET_FAILS = '1';
    try {
      await assert.rejects(k8s.create(CONFIG, SPEC), /forbidden/);
    } finally {
      delete process.env.FAKE_SECRET_FAILS;
    }
    assert.deepEqual(cli.calls().at(-1)!.args.slice(0, 3), ['delete', 'pod', SPEC.name]);
  });
});

describe('k8s find, destroy and list', () => {
  it('reads ownership from annotations and deletes the pod', async () => {
    const found = await k8s.find(CONFIG, 'oya-browser-b-1');
    assert.deepEqual([found!.labels['oya-browser-id'], found!.state], ['b-1', 'running']);
    await found!.destroy();
    assert.deepEqual(cli.calls().at(-1)!.args.slice(0, 5), ['delete', 'pod', 'oya-browser-b-1', '-n', 'browsers']);
  });

  it('answers null for a pod that does not exist, and passes on other failures', async () => {
    assert.equal(await k8s.find(CONFIG, 'missing'), null);
    await assert.rejects(k8s.find(CONFIG, 'broken'), /connection refused/);
  });

  it('lists the owner’s pods by label selector', async () => {
    const rows = await k8s.list(CONFIG, 'owner-tag');
    assert.deepEqual(
      rows.map((row) => row.state),
      ['running', 'unknown'],
    );
    assert.ok(cli.calls()[0].args.includes('oya-browser=true,oya-owner=owner-tag'));
  });
});
