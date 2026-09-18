/** The Kubernetes objects for one governed browser: its pod and the Secret holding its credentials. */
import { createHash } from 'node:crypto';
import { hash } from './common.ts';
import { K8S_NAME_MAX, POD_DIGEST_CHARS, POD_LABEL, POD_SLUG_KEEP } from './constants.ts';

/** No privilege escalation, no capabilities, the runtime's default seccomp profile. */
const RESTRICTED = {
  allowPrivilegeEscalation: false,
  capabilities: { drop: ['ALL'] },
  seccompProfile: { type: 'RuntimeDefault' },
};

/** Pod names are DNS-1123 labels: lowercase alphanumeric and '-', at most 63 chars. */
export function podName(browserId) {
  const slug = String(browserId)
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, '-')
    .replace(/^-+|-+$/g, '');
  const candidate = `oya-managed-${slug}`;
  if (candidate.length <= K8S_NAME_MAX) return candidate;
  // Truncating alone could collide, so the tail is a digest of the real id.
  const digest = createHash('sha256').update(String(browserId)).digest('hex').slice(0, POD_DIGEST_CHARS);
  return `${candidate.slice(0, POD_SLUG_KEEP)}-${digest}`;
}

/** Secret carries the credentials; the pod only references it. */
export function manifests({ pod, ns, image, apiKey, browserId, environment }) {
  const common = metadata(ns, apiKey, browserId);
  return [secretManifest(pod, common, environment), podManifest(pod, common, image)];
}

/** Namespace, labels and ownership annotations shared by both objects. */
function metadata(ns, apiKey, browserId) {
  const owner = hash(apiKey);
  return {
    namespace: ns,
    labels: selectionLabels(owner, browserId),
    annotations: { 'ai.getoya.owner': owner, 'ai.getoya.session': String(browserId) },
  };
}

/**
 * Label values cap at 63 characters and the owner hash is 64, so the label
 * is for selection only; ownership is verified against the annotation.
 */
const selectionLabels = (owner, browserId) => ({
  app: POD_LABEL,
  'oya.owner': owner.slice(0, K8S_NAME_MAX),
  'oya.session': podName(browserId).slice(0, K8S_NAME_MAX),
});

/** The enrollment Secret, one key per environment variable. */
const secretManifest = (pod, common, environment) => ({
  apiVersion: 'v1',
  kind: 'Secret',
  type: 'Opaque',
  metadata: { name: `${pod}-enroll`, ...common },
  stringData: Object.fromEntries(Object.entries(environment).map(([k, v]) => [k, String(v ?? '')])),
});

/** The browser pod. */
const podManifest = (pod, common, image) => ({
  apiVersion: 'v1',
  kind: 'Pod',
  metadata: { name: pod, ...common },
  spec: podSpec(pod, image),
});

/** One browser container, never restarted, with no Kubernetes API token. */
const podSpec = (pod, image) => ({
  // A spent enrollment token cannot enrol twice, so a restart would just
  // produce a pod that never connects.
  restartPolicy: 'Never',
  // A browser has no business holding a Kubernetes API token.
  automountServiceAccountToken: false,
  enableServiceLinks: false,
  volumes: [{ name: 'dev-shm', emptyDir: { medium: 'Memory', sizeLimit: '512Mi' } }],
  containers: [browserContainer(pod, image)],
});

/** The browser container: environment from the Secret, a memory-backed /dev/shm, restricted privileges. */
const browserContainer = (pod, image) => ({
  name: 'browser',
  image,
  envFrom: [{ secretRef: { name: `${pod}-enroll` } }],
  volumeMounts: [{ name: 'dev-shm', mountPath: '/dev/shm' }],
  resources: { requests: { memory: '512Mi', cpu: '250m' }, limits: { memory: '2Gi' } },
  securityContext: RESTRICTED,
});
