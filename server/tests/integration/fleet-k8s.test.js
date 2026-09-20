/**
 * Kubernetes fleet runtime, without a cluster.
 *
 * A fake kubectl is put on PATH rather than stubbing modules, so the real spawn
 * path runs and the manifest is asserted exactly as kubectl would receive it —
 * including that credentials arrive on stdin and never in argv, where `ps` would
 * expose them to every process on the node.
 */
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const work = await mkdtemp(join(tmpdir(), 'oya-k8s-test-'));
const bin = join(work, 'bin');
const log = join(work, 'calls.jsonl');
await mkdtemp(join(tmpdir(), 'x')).then(() => {});
await (await import('node:fs/promises')).mkdir(bin, { recursive: true });

const FAKE = `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
let stdin = '';
try { stdin = fs.readFileSync(0, 'utf8'); } catch {}
fs.appendFileSync(process.env.OYA_FAKE_LOG, JSON.stringify({ tool: 'kubectl', args, stdin }) + '\\n');
const has = (v) => args.includes(v);
const die = (msg) => { process.stderr.write(msg); process.exit(1); };

if (has('networkpolicy')) {
  if (process.env.OYA_FAKE_NP === 'missing') die('Error from server (NotFound): networkpolicies "x" not found');
  process.stdout.write(process.env.OYA_FAKE_NP || JSON.stringify({
    spec: { podSelector: { matchLabels: { app: 'oya-managed-browser' } }, policyTypes: ['Egress'] },
  }));
} else if (has('kube-system')) {
  process.stdout.write(process.env.OYA_FAKE_CLUSTER || 'cluster-uid-1');
} else if (args[0] === 'get' && args[1] === 'namespace') {
  process.stdout.write('namespace/' + args[2]);
} else if (args[0] === 'get' && args[1] === 'pod') {
  if (!process.env.OYA_FAKE_POD) die('Error from server (NotFound): pods "x" not found');
  process.stdout.write(process.env.OYA_FAKE_POD);
} else if (args[0] === 'apply') {
  process.stdout.write('created');
} else if (args[0] === 'delete') {
  process.stdout.write('deleted');
} else die('fake kubectl: unhandled ' + args.join(' '));
`;
await writeFile(join(bin, 'kubectl'), FAKE, { mode: 0o755 });

// A fake docker too, so cleanup routing can be observed rather than assumed.
const FAKE_DOCKER = `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
fs.appendFileSync(process.env.OYA_FAKE_LOG, JSON.stringify({ tool: 'docker', args }) + '\\n');
if (args[0] === 'info') process.stdout.write('fake-daemon-id');
else if (args[0] === 'inspect') { process.stderr.write('Error: No such object: ' + args[1]); process.exit(1); }
else process.stdout.write('ok');
`;
await writeFile(join(bin, 'docker'), FAKE_DOCKER, { mode: 0o755 });

Object.assign(process.env, {
  PATH: `${bin}:${process.env.PATH}`,
  OYA_FAKE_LOG: log,
  OYA_DATA_DIR: join(work, 'data'),
  API_KEYS: 'k8s-fleet-test',
  OYA_PROFILE_SECRET: 'k8s-fleet-test-secret',
  OYA_FLEET_RUNTIME: 'k8s',
  OYA_K8S_NAMESPACE: 'oya-browsers',
  OYA_MANAGED_NETWORK: 'oya-governed-egress',
  OYA_MANAGED_IMAGE: 'ghcr.io/oyadotai/oya-browser@sha256:' + 'a'.repeat(64),
  OYA_MANAGED_CONTROL_URL: 'ws://oya-server:3100/ws',
  OYA_MANAGED_PROXY_URL: 'http://oya-server:3128',
  OYA_MANAGED_REGION: 'local',
});
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
delete process.env.DATABASE_URL;

const { control, hash } = await import('../../src/modules/control/service.ts');
const { createManaged, removeManaged, managedConfigured, verifyRuntime } =
  await import('../../src/modules/control/managed.ts');
const k8s = await import('../../src/modules/control/fleet/k8s.ts');

const KEY = 'k8s-fleet-test';
const calls = async () =>
  (await readFile(log, 'utf8'))
    .trim()
    .split('\n')
    .filter(Boolean)
    .map((l) => JSON.parse(l));
const reset = async () => writeFile(log, '');

let passed = 0;
// Awaited: an async assertion passed to a synchronous runner would reject
// unobserved, and the case would pass without ever having checked anything.
const check = async (label, fn) => {
  await fn();
  passed++;
  console.log(`  ✅ ${label}`);
};

try {
  assert.equal(managedConfigured(), true, 'k8s runtime reports configured');

  // ── The pod manifest carries the security invariants ──────────────────────
  await reset();
  const session = await control().reserve(KEY, { provider: 'oya-selfhosted' });
  await createManaged({ apiKey: KEY, browserId: session.id, persona: 'default', name: 'test', policies: [] });

  const applied = (await calls()).find((c) => c.args[0] === 'apply');
  assert.ok(applied, 'kubectl apply was invoked');
  const items = JSON.parse(applied.stdin).items;
  const pod = items.find((i) => i.kind === 'Pod');
  const secret = items.find((i) => i.kind === 'Secret');
  const container = pod.spec.containers[0];

  await check('manifest goes in on stdin, never argv', () => {
    assert.ok(applied.args.includes('-f') && applied.args.includes('-'));
    const argv = applied.args.join(' ');
    assert.ok(!/OYA_ENROLLMENT_TOKEN|OYA_API_KEY/.test(argv), 'no credential names in argv');
  });
  await check('no credential appears in any kubectl argv', async () => {
    for (const c of [applied]) {
      assert.ok(
        !c.args.some((a) => a.includes(secret.stringData.OYA_ENROLLMENT_TOKEN)),
        'enrollment token not in argv',
      );
      assert.ok(!c.args.some((a) => a.includes(secret.stringData.OYA_API_KEY)), 'session credential not in argv');
    }
  });
  await check('all capabilities dropped', () => assert.deepEqual(container.securityContext.capabilities.drop, ['ALL']));
  await check('privilege escalation disallowed', () =>
    assert.equal(container.securityContext.allowPrivilegeEscalation, false),
  );
  await check('seccomp is the runtime default', () =>
    assert.equal(container.securityContext.seccompProfile.type, 'RuntimeDefault'),
  );
  await check('no Kubernetes API token is mounted', () => assert.equal(pod.spec.automountServiceAccountToken, false));
  await check('a spent enrollment token cannot be reused by a restart', () =>
    assert.equal(pod.spec.restartPolicy, 'Never'),
  );
  await check('memory is capped', () => assert.equal(container.resources.limits.memory, '2Gi'));
  await check('/dev/shm is a sized memory volume, as Chrome needs', () => {
    const vol = pod.spec.volumes.find((v) => v.name === 'dev-shm');
    assert.equal(vol.emptyDir.medium, 'Memory');
    assert.equal(vol.emptyDir.sizeLimit, '512Mi');
    assert.ok(container.volumeMounts.some((m) => m.mountPath === '/dev/shm'));
  });
  await check('credentials reach the pod only through the Secret', () => {
    assert.deepEqual(container.envFrom, [{ secretRef: { name: `${pod.metadata.name}-enroll` } }]);
    assert.equal(container.env, undefined, 'nothing inline in the pod spec');
    assert.ok(secret.stringData.OYA_ENROLLMENT_TOKEN, 'enrollment token is in the Secret');
    assert.notEqual(secret.stringData.OYA_API_KEY, KEY, 'the project key is never handed to the browser');
  });
  await check('ownership is recorded where it is not truncated', () => {
    assert.equal(pod.metadata.annotations['ai.getoya.owner'], hash(KEY));
    assert.equal(pod.metadata.annotations['ai.getoya.session'], session.id);
    assert.ok(pod.metadata.labels['oya.owner'].length <= 63, 'label values stay within the 63-char limit');
  });
  await check('the image the pod runs is the digest-pinned one', () =>
    assert.equal(container.image, process.env.OYA_MANAGED_IMAGE),
  );

  // ── verifyRuntime refuses unsafe configuration ────────────────────────────
  const image = process.env.OYA_MANAGED_IMAGE;
  process.env.OYA_MANAGED_IMAGE = 'ghcr.io/oyadotai/oya-browser:latest';
  await assert.rejects(verifyRuntime(), { code: 'unsupported_image' }, 'a mutable tag is refused');
  passed++;
  console.log('  ✅ a mutable image tag is refused; only a digest is accepted');
  process.env.OYA_MANAGED_IMAGE = image;

  process.env.OYA_FAKE_NP = 'missing';
  await assert.rejects(verifyRuntime(), { code: 'unsafe_network' }, 'a missing NetworkPolicy is refused');
  passed++;
  console.log('  ✅ a missing NetworkPolicy is refused');

  process.env.OYA_FAKE_NP = JSON.stringify({
    spec: { podSelector: { matchLabels: { app: 'something-else' } }, policyTypes: ['Egress'] },
  });
  await assert.rejects(verifyRuntime(), { code: 'unsafe_network' }, 'a policy selecting other pods is refused');
  passed++;
  console.log('  ✅ a NetworkPolicy that does not select these pods is refused');

  process.env.OYA_FAKE_NP = JSON.stringify({ spec: { podSelector: {}, policyTypes: ['Ingress'] } });
  await assert.rejects(verifyRuntime(), { code: 'unsafe_network' }, 'an ingress-only policy is refused');
  passed++;
  console.log('  ✅ a NetworkPolicy that does not restrict Egress is refused');
  delete process.env.OYA_FAKE_NP;

  // ── Cleanup verifies ownership and the cluster ────────────────────────────
  const podName = k8s.podName(session.id);
  process.env.OYA_FAKE_POD = JSON.stringify({
    metadata: { annotations: { 'ai.getoya.owner': hash('someone-else'), 'ai.getoya.session': session.id } },
  });
  await assert.rejects(
    removeManaged(podName, KEY, session.id, 'cluster-uid-1', 'k8s'),
    { code: 'ownership_mismatch' },
    "another tenant's pod is not deleted",
  );
  passed++;
  console.log('  ✅ cleanup refuses a pod owned by another key');

  process.env.OYA_FAKE_POD = JSON.stringify({
    metadata: { annotations: { 'ai.getoya.owner': hash(KEY), 'ai.getoya.session': 'a-different-session' } },
  });
  await assert.rejects(
    removeManaged(podName, KEY, session.id, 'cluster-uid-1', 'k8s'),
    { code: 'ownership_mismatch' },
    'a pod from another session is not deleted',
  );
  passed++;
  console.log('  ✅ cleanup refuses a pod belonging to another session');

  process.env.OYA_FAKE_POD = JSON.stringify({
    metadata: { annotations: { 'ai.getoya.owner': hash(KEY), 'ai.getoya.session': session.id } },
  });
  process.env.OYA_FAKE_CLUSTER = 'a-different-cluster';
  await assert.rejects(
    removeManaged(podName, KEY, session.id, 'cluster-uid-1', 'k8s'),
    { code: 'runtime_unavailable' },
    'a pod in the wrong cluster is not deleted',
  );
  passed++;
  console.log('  ✅ cleanup refuses a different cluster than the one that created the pod');
  process.env.OYA_FAKE_CLUSTER = 'cluster-uid-1';

  await reset();
  await removeManaged(podName, KEY, session.id, 'cluster-uid-1', 'k8s');
  const deleted = (await calls()).find((c) => c.args[0] === 'delete');
  await check('a matching pod is deleted together with its Secret', () => {
    assert.ok(deleted.args.includes('pod') && deleted.args.includes(podName));
    assert.ok(deleted.args.includes(`${podName}-enroll`), 'the Secret does not outlive the pod');
  });

  await reset();
  delete process.env.OYA_FAKE_POD;
  await removeManaged(podName, KEY, session.id, 'cluster-uid-1', 'k8s');
  await check('an already-absent pod still has its Secret collected', async () => {
    assert.ok((await calls()).some((c) => c.args[0] === 'delete' && c.args.includes('secret')));
  });

  // ── Names stay legal ─────────────────────────────────────────────────────
  await check('pod names are valid DNS-1123 labels', () => {
    const long = k8s.podName(`APIKEY_${'x'.repeat(90)}/weird`);
    assert.ok(long.length <= 63, `got ${long.length}`);
    assert.match(long, /^[a-z0-9]([-a-z0-9]*[a-z0-9])?$/);
    assert.notEqual(k8s.podName('a'.repeat(80)), k8s.podName('b' + 'a'.repeat(79)), 'truncation does not collide');
  });

  // ── Cleanup routing honours the descriptor, not the current setting ───────
  // OYA_FLEET_RUNTIME is still 'k8s' here, so this proves the stored descriptor
  // wins: a browser started on Docker must be torn down on Docker even after the
  // operator moves the fleet to Kubernetes, or it is left running forever.
  await reset();
  await removeManaged('oya-managed-legacy', KEY, 'legacy-session', null, 'docker');
  const routed = await calls();
  await check('a docker-created session cleans up via docker after a switch to k8s', () => {
    assert.ok(routed.length, 'something was invoked');
    assert.ok(
      routed.every((c) => c.tool === 'docker'),
      `expected docker, saw ${routed.map((c) => c.tool).join(',')}`,
    );
  });

  await reset();
  await removeManaged('oya-managed-old', KEY, 'old-session', null, undefined);
  await check('a descriptor written before runtimes existed is treated as docker', async () => {
    assert.ok((await calls()).every((c) => c.tool === 'docker'));
  });

  // ── ...and the namespace comes from the descriptor too ───────────────────
  // Change OYA_K8S_NAMESPACE with sessions in flight and re-deriving it here
  // leaves the pod — and its -enroll Secret, which carries OYA_API_KEY and the
  // egress proxy password — running in the old namespace forever.
  await reset();
  const before = process.env.OYA_K8S_NAMESPACE;
  process.env.OYA_K8S_NAMESPACE = 'moved-since';
  delete process.env.OYA_FAKE_POD;
  await removeManaged(podName, KEY, session.id, 'cluster-uid-1', 'k8s', 'oya-browsers');
  if (before === undefined) delete process.env.OYA_K8S_NAMESPACE;
  else process.env.OYA_K8S_NAMESPACE = before;
  await check('cleanup targets the namespace the pod was created in', async () => {
    const del = (await calls()).find((c) => c.args[0] === 'delete');
    assert.ok(del, 'a delete was issued');
    assert.equal(del.args[del.args.indexOf('-n') + 1], 'oya-browsers');
  });

  console.log(`\n  ${passed} passed, 0 failed`);
} finally {
  await rm(work, { recursive: true, force: true });
}
