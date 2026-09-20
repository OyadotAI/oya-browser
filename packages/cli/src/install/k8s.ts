/**
 * The Kubernetes browser fleet: a digest-pinned image, the browsers'
 * namespace, and the NetworkPolicy that is a governed pod's only isolation.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { capture, pipeTo } from './shell.ts';
import type { Answers } from './types.ts';
import { DIGEST_PREVIEW, DNS_PORT, JSON_INDENT } from './constants.ts';

/** The NetworkPolicy's name, which the runtime looks for. */
const POLICY_NAME = 'oya-governed-egress';

/** The error for a tag that could not be resolved, with how to do it by hand. */
function unpinnable(reference: string): Error {
  return new Error(
    `Kubernetes browsers must run a digest-pinned image, and ${reference} is a tag.\n` +
      `  Get the digest with:  docker manifest inspect --verbose ${reference}\n` +
      `  then re-run with the image written as ${reference.split(':')[0]}@sha256:<digest>`,
  );
}

/** The digest `docker manifest inspect` reports for a tag, or nothing. */
async function resolveDigest(reference: string): Promise<string | null | undefined> {
  const raw = await capture('docker', ['manifest', 'inspect', '--verbose', reference]);
  return raw && /"digest":\s*"(sha256:[0-9a-f]{64})"/.exec(raw)?.[1];
}

/**
 * The runtime refuses a mutable tag, so resolve one to a digest here rather than
 * making the operator go and find it. `docker manifest inspect` reads the registry
 * without pulling; if Docker is not around, say exactly what to run.
 */
export async function pinImage(reference: string): Promise<string> {
  if (/@sha256:[0-9a-f]{64}$/.test(reference)) return reference;
  process.stdout.write(`  resolving ${reference} to a digest… `);
  const digest = await resolveDigest(reference);
  console.log(digest ? digest.slice(0, DIGEST_PREVIEW) + '…' : 'could not');
  if (!digest) throw unpinnable(reference);
  return `${reference.split('@')[0].replace(/:[^:/]+$/, '')}@${digest}`;
}

/** The egress rules: DNS, and the control plane's egress proxy. */
const EGRESS = [
  // DNS only, to the cluster resolver.
  {
    ports: [
      { protocol: 'UDP', port: DNS_PORT },
      { protocol: 'TCP', port: DNS_PORT },
    ],
  },
  // Everything else must go through the control plane's egress proxy, which
  // is what makes per-session egress policy and budgets enforceable.
  // `app: server` is the label k8s/base/server.yaml gives the control plane;
  // the empty namespaceSelector lets the browsers live in their own namespace.
  // A control plane OUTSIDE the cluster is not matched by any pod selector —
  // add an ipBlock rule here for that case, or the pods will resolve DNS and
  // then fail to enrol.
  { to: [{ namespaceSelector: {}, podSelector: { matchLabels: { app: 'server' } } }] },
];

/**
 * A governed pod's only isolation is this NetworkPolicy — the Kubernetes stand-in
 * for Docker's internal bridge — so the runtime verifies it exists, selects these
 * pods and restricts Egress. It is written to disk before being applied: it is the
 * operator's security boundary and they should be able to read and tighten it.
 */
export function networkPolicy(namespace: string, name: string): string {
  const spec = {
    podSelector: { matchLabels: { app: 'oya-managed-browser' } },
    policyTypes: ['Egress'],
    egress: EGRESS,
  };
  const policy = { apiVersion: 'networking.k8s.io/v1', kind: 'NetworkPolicy', metadata: { name, namespace }, spec };
  return JSON.stringify(policy, null, JSON_INDENT);
}

/** Writes the NetworkPolicy under k8s/fleet/ for the operator to read, and returns its path. */
function savePolicy(root: string, namespace: string): string {
  const policyPath = join(root, 'k8s', 'fleet', 'networkpolicy.json');
  mkdirSync(join(root, 'k8s', 'fleet'), { recursive: true });
  writeFileSync(policyPath, networkPolicy(namespace, POLICY_NAME) + '\n');
  return policyPath;
}

/** Applies a manifest with kubectl, fed on stdin. */
const apply = (manifest: string, root: string) => pipeTo('kubectl', ['apply', '-f', '-'], manifest, root);

/** Pins the image and applies the namespace and NetworkPolicy. Returns the pinned image. */
export async function provisionK8sFleet(root: string, a: Answers): Promise<string> {
  const fleet = a.k8sFleet!;
  const pinned = await pinImage(fleet.image);
  // apply, not create: re-running an install must not fail on an existing namespace.
  await apply(JSON.stringify({ apiVersion: 'v1', kind: 'Namespace', metadata: { name: fleet.namespace } }), root);
  const policyPath = savePolicy(root, fleet.namespace);
  await apply(networkPolicy(fleet.namespace, POLICY_NAME), root);
  console.log(`  applied namespace ${fleet.namespace} and NetworkPolicy ${POLICY_NAME}`);
  console.log(`  policy written to ${policyPath} — review it; it is your egress boundary`);
  return pinned;
}
