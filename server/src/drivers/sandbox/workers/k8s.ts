/**
 * Kubernetes sandbox runtime: one pod of the browser image per browser, in the
 * namespace and cluster the server's kubectl reaches (OYA_K8S_CONTEXT).
 * Operator-only: the image, namespace and cluster never come from a key.
 *
 * The browser's credentials sit in a Secret owned by the pod, so deleting the
 * pod, by hand or by us, garbage-collects the Secret with it.
 * activeDeadlineSeconds is the hard lifetime; there is no idle stop.
 */
import { kubectl } from '../../../modules/control/managed.ts';
import { unset } from '../names.ts';
import { SECONDS_PER_MINUTE } from '../../constants.ts';

/** The runtime's name, as OYA_CLOUD_RUNTIME selects it. */
export const id = 'k8s';

/** Runs on the operator's cluster, so a key can never bring its own. */
export const ownAccount = () => false;

/** The namespace when OYA_K8S_NAMESPACE is unset. */
const DEFAULT_NAMESPACE = 'oya-browsers';

/** No privilege escalation, no capabilities, the runtime's default seccomp profile. */
const RESTRICTED = {
  allowPrivilegeEscalation: false,
  capabilities: { drop: ['ALL'] },
  seccompProfile: { type: 'RuntimeDefault' },
};

/** The Kubernetes settings from env, or null unless the image is set. */
export function settings(env) {
  if (!env.OYA_CLOUD_IMAGE) return null;
  return { image: env.OYA_CLOUD_IMAGE, namespace: env.OYA_K8S_NAMESPACE || DEFAULT_NAMESPACE };
}

/** What is unset. */
export const missing = (env) => unset([['OYA_CLOUD_IMAGE', env.OYA_CLOUD_IMAGE]]);

/**
 * Creates the pod, then its Secret owned by the pod. The pod waits for the
 * Secret on its own, and a Secret created first would outlive a failed pod.
 */
export async function create(config, spec) {
  const created = JSON.parse(await apply(config, pod(config, spec), ['-o', 'json']));
  try {
    await apply(config, secret(spec, created.metadata.uid));
  } catch (err) {
    // A pod without its Secret never starts; do not leave it waiting.
    await destroy(config, spec.name).catch(() => {});
    throw err;
  }
  return { id: created.metadata.uid };
}

/** Creates one object in the namespace, the manifest on stdin so no credential reaches argv. */
const apply = (config, manifest, extra: string[] = []) =>
  kubectl(['create', '-n', config.namespace, ...extra, '-f', '-'], JSON.stringify(manifest));

/**
 * Label values allow no spaces, so labels carry only what selects the pod;
 * every label, the display name included, is also an annotation, which is
 * what ownership is read back from.
 */
const metadata = (spec) => ({
  name: spec.name,
  labels: { 'oya-browser': 'true', 'oya-owner': spec.labels['oya-owner'] },
  annotations: spec.labels,
});

/** The browser pod. */
const pod = (config, spec) => ({
  apiVersion: 'v1',
  kind: 'Pod',
  metadata: metadata(spec),
  spec: podSpec(config, spec),
});

/** Never restarted, no API token, a memory-backed /dev/shm, and a hard deadline. */
const podSpec = (config, spec) => ({
  restartPolicy: 'Never',
  automountServiceAccountToken: false,
  enableServiceLinks: false,
  activeDeadlineSeconds: spec.lifetimeMinutes * SECONDS_PER_MINUTE,
  volumes: [{ name: 'dev-shm', emptyDir: { medium: 'Memory', sizeLimit: '512Mi' } }],
  containers: [browserContainer(config, spec)],
});

/** The browser container, its environment from the Secret. */
const browserContainer = (config, spec) => ({
  name: 'browser',
  image: config.image,
  envFrom: [{ secretRef: { name: spec.name } }],
  volumeMounts: [{ name: 'dev-shm', mountPath: '/dev/shm' }],
  resources: { requests: { memory: '512Mi', cpu: '250m' }, limits: { memory: '2Gi' } },
  securityContext: RESTRICTED,
});

/** The Secret holding the browser's environment, garbage-collected with its pod. */
const secret = (spec, podUid) => ({
  apiVersion: 'v1',
  kind: 'Secret',
  type: 'Opaque',
  metadata: { ...metadata(spec), ownerReferences: [{ apiVersion: 'v1', kind: 'Pod', name: spec.name, uid: podUid }] },
  stringData: spec.env,
});

/** The pod of this name, or null when the namespace has none. */
export async function find(config, name) {
  try {
    const info = JSON.parse(await kubectl(['get', 'pod', name, '-n', config.namespace, '-o', 'json']));
    return { ...found(info), destroy: () => destroy(config, name) };
  } catch (err) {
    if (/NotFound|not found/i.test(err.stderr || err.message)) return null;
    throw err;
  }
}

/** Deletes the pod; its Secret goes with it. Already gone is fine. */
async function destroy(config, name) {
  await kubectl(['delete', 'pod', name, '-n', config.namespace, '--ignore-not-found', '--wait=false']);
}

/** Every browser pod labelled for this owner, finished ones included. */
export async function list(config, owner) {
  const selector = `oya-browser=true,oya-owner=${owner}`;
  const pods = JSON.parse(await kubectl(['get', 'pods', '-n', config.namespace, '-l', selector, '-o', 'json']));
  return (pods.items || []).map(found);
}

/** A pod's labels (from its annotations) and state. */
const found = (info) => ({
  labels: info.metadata?.annotations || {},
  state: String(info.status?.phase || 'unknown').toLowerCase(),
});
