/**
 * Kubernetes fleet runtime: one pod per session.
 *
 * The Docker runtime proves its image carries the governance hooks by reading the
 * ai.getoya.governance label off a local image. kubectl cannot do that — the image
 * lives in a registry and is resolved by the kubelet, not by us. So this runtime
 * demands the stronger, locally checkable property instead: the image reference
 * must be pinned to a digest. A tag can be moved under you between verification
 * and scheduling; a digest cannot.
 *
 * Isolation is a NetworkPolicy the operator supplies, standing in for Docker's
 * internal bridge: we require it to exist and to actually select these pods,
 * because a policy that matches nothing is indistinguishable from no policy.
 */

import { prepareSession, hash, fault } from './common.ts';
import { kubectl } from './kubectl.ts';
import { manifests, podName } from './k8s-manifests.ts';
import { Status } from '../../../platform/http-status.ts';
import { CAPABILITIES, DEFAULT_NAMESPACE, POD_LABEL } from './constants.ts';

export { podName };

/** The runtime's name, as OYA_FLEET_RUNTIME selects it. */
export const id = 'k8s';

/** The namespace managed pods run in. */
const namespace = () => process.env.OYA_K8S_NAMESPACE || DEFAULT_NAMESPACE;
/** Why governance is refused before the runtime is set up. */
const UNCONFIGURED = 'Configure the managed Kubernetes runtime before requesting governance';
/** Why an image reference by tag is refused. */
const UNPINNED =
  'On Kubernetes OYA_MANAGED_IMAGE must be pinned to a digest (image@sha256:…), so the governed image cannot be swapped after verification';
/** Marks a pod that no longer exists. */
const GONE = Symbol('gone');

/** Whether the network policy, image, control URL and proxy URL are all set. */
export function configured() {
  return !!(
    process.env.OYA_MANAGED_NETWORK &&
    process.env.OYA_MANAGED_IMAGE &&
    process.env.OYA_MANAGED_CONTROL_URL &&
    process.env.OYA_MANAGED_PROXY_URL
  );
}

/**
 * Refuse unless the image is pinned to a digest, the namespace is reachable and the
 * NetworkPolicy selects these pods and restricts egress. Returns the runtime identity
 * sessions are pinned to, including the cluster's kube-system UID.
 */
export async function verifyRuntime() {
  if (!configured()) throw fault('runtime_unavailable', UNCONFIGURED, Status.UNPROCESSABLE);
  const image = pinnedImage();
  const ns = namespace();
  await requireNamespace(ns);
  requireEgressPolicy(await networkPolicy(ns));
  return runtimeIdentity(await clusterUid(), image, ns);
}

/** The managed image, refused unless it is pinned to a digest. */
function pinnedImage() {
  const image = process.env.OYA_MANAGED_IMAGE;
  if (!/@sha256:[0-9a-f]{64}$/.test(image)) throw fault('unsupported_image', UNPINNED, Status.UNPROCESSABLE);
  return image;
}

/** Refuses unless the namespace is reachable. */
async function requireNamespace(ns) {
  try {
    await kubectl(['get', 'namespace', ns, '-o', 'name']);
  } catch (e) {
    throw fault('runtime_unavailable', `Namespace ${ns} is not reachable: ${e.message}`, Status.UNPROCESSABLE);
  }
}

/**
 * The NetworkPolicy is the only thing keeping a governed browser off the rest
 * of the cluster, so an absent or non-selecting policy is a hard failure.
 */
async function networkPolicy(ns) {
  try {
    return JSON.parse(await kubectl(['get', 'networkpolicy', process.env.OYA_MANAGED_NETWORK, '-n', ns, '-o', 'json']));
  } catch {
    const message = `NetworkPolicy ${process.env.OYA_MANAGED_NETWORK} not found in ${ns}; managed browsers require egress restriction`;
    throw fault('unsafe_network', message, Status.UNPROCESSABLE);
  }
}

/** Refuses a policy that does not select managed pods or does not restrict egress. */
function requireEgressPolicy(policy) {
  const name = process.env.OYA_MANAGED_NETWORK;
  const selector = policy?.spec?.podSelector?.matchLabels || {};
  const selectsAll = Object.keys(selector).length === 0;
  if (!selectsAll && selector.app !== POD_LABEL)
    throw fault('unsafe_network', `NetworkPolicy ${name} does not select app=${POD_LABEL} pods`, Status.UNPROCESSABLE);
  if (!(policy.spec.policyTypes || []).includes('Egress'))
    throw fault('unsafe_network', `NetworkPolicy ${name} must restrict Egress`, Status.UNPROCESSABLE);
}

/**
 * The cluster's identity, so cleanup cannot be aimed at a different cluster
 * that happens to have a pod of the same name. kube-system's UID is stable.
 */
const clusterUid = async () =>
  (await kubectl(['get', 'namespace', 'kube-system', '-o', 'jsonpath={.metadata.uid}'])).trim();

/** What a session records about the runtime it was started on. */
const runtimeIdentity = (clusterId, image, ns) => ({
  id: 'k8s',
  daemonId: clusterId,
  networkId: process.env.OYA_MANAGED_NETWORK,
  imageId: image,
  region: process.env.OYA_MANAGED_REGION || ns,
  capabilities: [...CAPABILITIES],
  verifiedAt: Date.now(),
});

/** Apply the session's pod and its enrollment Secret in the configured namespace. */
export async function create({ apiKey, browserId, persona, name, policies = [] }) {
  const runtime = await verifyRuntime();
  const ns = namespace();
  const pod = podName(browserId);
  const session = { apiKey, browserId, persona, name, policies, runtime, fallbackName: pod };
  const { environment } = await prepareSession({ ...session, cleanup: podCleanup(pod, ns, runtime) });
  const docs = manifests({ pod, ns, image: runtime.imageId, apiKey, browserId, environment });
  await kubectl(['apply', '-n', ns, '-f', '-'], JSON.stringify({ apiVersion: 'v1', kind: 'List', items: docs }));
  return { browserId, runtime };
}

/** What cleanup deletes: the pod, in the namespace and on the cluster it was created in. */
const podCleanup = (pod, ns, runtime) => ({
  kind: 'docker',
  runtime: 'k8s',
  container: pod,
  namespace: ns,
  daemonId: runtime.daemonId,
});

/** Delete the session's pod and enrollment Secret, only on the cluster that made it and only if its annotations match the owner and session. Already gone is fine. */
export async function remove(pod, apiKey, browserId, clusterId = null, storedNamespace = null) {
  // The namespace the pod was created in, not the one configured now: after an
  // operator changes OYA_K8S_NAMESPACE, every in-flight pod and its -enroll
  // Secret (which carries OYA_API_KEY and the egress proxy password) would
  // otherwise be orphaned, while the delete is aimed at a namespace that never
  // held them.
  const ns = storedNamespace || namespace();
  if (clusterId && (await clusterUid()) !== clusterId)
    throw fault('runtime_unavailable', 'Cleanup requires the original Kubernetes cluster', Status.UNAVAILABLE);
  const info = await getPod(pod, ns);
  if (info === GONE) return;
  requireOwner(info, apiKey, browserId);
  await kubectl(['delete', 'pod', pod, 'secret', `${pod}-enroll`, '-n', ns, '--ignore-not-found', '--wait=false']);
}

/** The pod, or GONE (after deleting its Secret) when it no longer exists. */
async function getPod(pod, ns) {
  try {
    return JSON.parse(await kubectl(['get', 'pod', pod, '-n', ns, '-o', 'json']));
  } catch (e) {
    if (!/NotFound|not found/i.test(e.stderr || e.message)) throw e;
    // The pod is gone; its Secret must not outlive it.
    await kubectl(['delete', 'secret', `${pod}-enroll`, '-n', ns, '--ignore-not-found']).catch(() => {});
    return GONE;
  }
}

/** Refuses unless the pod's annotations name this owner and session. */
function requireOwner(info, apiKey, browserId) {
  const annotations = info?.metadata?.annotations || {};
  if (annotations['ai.getoya.owner'] !== hash(apiKey) || annotations['ai.getoya.session'] !== String(browserId)) {
    throw fault('ownership_mismatch', 'Managed pod ownership mismatch');
  }
}
