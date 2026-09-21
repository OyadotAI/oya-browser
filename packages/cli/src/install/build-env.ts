/**
 * Turns the answers and secrets into .env values: generated secrets (reusing a
 * previous install's), the LLM, the fleet's settings (one entry per fleet in a
 * command map), and optional services.
 */
import { randomBytes } from 'node:crypto';
import type { Answers, Secrets } from './types.ts';
import { DEFAULT_PORT, EGRESS_PORT, TOKEN_BYTES } from './constants.ts';

/** .env values by variable name. */
type Env = Record<string, string>;

/** What buildEnv produces. */
export interface BuiltEnv {
  /** The values to write. */
  values: Env;
  /** Lines appended after them, commented out. */
  extra: string[];
  /** The tenant key to hand the operator. */
  apiKey: string;
}

/** A fresh secret. */
export const token = () => randomBytes(TOKEN_BYTES).toString('base64url');

/** What a fleet's settings are computed from. */
interface FleetInput {
  /** The answers. */
  a: Answers;
  /** The previous install's values. */
  existing: Env;
  /** The control socket's public URL. */
  wsUrl: string;
}

/** Sets a fleet's variables. */
type FleetSettings = (v: Env, input: FleetInput) => void;

/** The egress proxy a governed fleet routes through. */
const egress = (v: Env) => Object.assign(v, { OYA_EGRESS_PORT: EGRESS_PORT, OYA_EGRESS_HOST: '0.0.0.0' });

/** Oya Cloud sandboxes dial the public socket. */
const oyaCloud: FleetSettings = (v, { existing, wsUrl }) => {
  v.OYA_BROWSER_PROVIDER = 'oya-cloud';
  v.OYA_PUBLIC_WS_URL = wsUrl;
  // A .env written before the rename still carries the old spelling.
  v.OYA_CLOUD_SNAPSHOT = existing.OYA_CLOUD_SNAPSHOT || existing.DAYTONA_SNAPSHOT || 'oya-browser';
};

/** Governed Docker: one container per session on an internal bridge. */
const governedDocker: FleetSettings = (v, { a, wsUrl }) => {
  const inCompose = a.host === 'docker';
  Object.assign(v, { OYA_BROWSER_PROVIDER: 'oya-selfhosted', OYA_FLEET_RUNTIME: 'docker' });
  Object.assign(v, { OYA_MANAGED_NETWORK: 'oya-browsers', OYA_MANAGED_IMAGE: 'oya-browser:local' });
  v.OYA_MANAGED_CONTROL_URL = inCompose ? `ws://server:${DEFAULT_PORT}/ws` : wsUrl;
  egress(v);
  v.OYA_MANAGED_PROXY_URL = inCompose ? `http://server:${EGRESS_PORT}` : `http://127.0.0.1:${EGRESS_PORT}`;
  v.OYA_MANAGED_REGION = 'local';
};

/** The Kubernetes fleet's own values. */
function kubeSettings(v: Env, k: NonNullable<Answers['k8sFleet']>): void {
  const runtime = { OYA_BROWSER_PROVIDER: 'oya-selfhosted', OYA_FLEET_RUNTIME: 'k8s', OYA_K8S_NAMESPACE: k.namespace };
  Object.assign(v, runtime);
  v.OYA_MANAGED_NETWORK = 'oya-governed-egress';
  v.OYA_MANAGED_IMAGE = k.image; // replaced with the pinned digest at provision time
  Object.assign(v, { OYA_MANAGED_CONTROL_URL: k.controlUrl, OYA_MANAGED_PROXY_URL: k.proxyUrl });
  v.OYA_MANAGED_REGION = k.namespace;
  egress(v);
}

/** Kubernetes: one pod per session, behind the governed-egress NetworkPolicy. Without its settings, just the name. */
const kubernetes: FleetSettings = (v, { a }) => {
  if (a.k8sFleet) kubeSettings(v, a.k8sFleet);
  else v.OYA_BROWSER_PROVIDER = a.fleet;
};

/** Each fleet's settings. Any other fleet is a provider name. */
const FLEETS: Record<string, FleetSettings> = {
  'oya-cloud': oyaCloud,
  'oya-selfhosted': governedDocker,
  k8s: kubernetes,
  'docker-workers': (v) => {
    v.OYA_BROWSER_PROVIDER = 'oya-selfhosted';
  },
};

/** The fleet's variables. */
function fleetSettings(v: Env, input: FleetInput): void {
  if (Object.hasOwn(FLEETS, input.a.fleet)) FLEETS[input.a.fleet](v, input);
  else v.OYA_BROWSER_PROVIDER = input.a.fleet;
}

/** The generated secrets, and the tenant key among them. */
interface Base {
  /** The values so far. */
  v: Env;
  /** The tenant key to hand the operator. */
  apiKey: string;
}

/**
 * Re-using the existing KEK is not an optimisation: a fresh one silently
 * orphans every credential already sealed in the data volume.
 */
function baseSecrets(a: Answers, existing: Env): Base {
  const v: Env = { OYA_PROFILE_SECRET: existing.OYA_PROFILE_SECRET || token() };
  const apiKey = existing.API_KEYS?.split(',')[0] || `oya_${token()}`;
  v.API_KEYS = existing.API_KEYS || apiKey;
  v.OYA_OPERATOR_TOKEN = existing.OYA_OPERATOR_TOKEN || token();
  if (a.host !== 'docker') v.PORT = String(Number(new URL(a.publicUrl).port || DEFAULT_PORT));
  return { v, apiKey };
}

/** CAPTCHA, recordings and metrics, when chosen. */
function optionalSettings(v: Env, a: Answers, existing: Env): void {
  if (a.optional.captcha) v.OYA_CAPTCHA_PROVIDER = a.optional.captcha;
  if (a.optional.recordingBucket) v.OYA_RECORDING_BUCKET = a.optional.recordingBucket;
  if (a.optional.metrics) v.OYA_METRICS_TOKEN = existing.OYA_METRICS_TOKEN || token();
}

/**
 * Generated but left commented: any browser holding this is accepted, so it is
 * opt-in rather than something an install quietly switches on.
 */
const fleetTokenLines = () => [
  '# ── Shared fleet enrolment ──',
  '# Uncomment to let any browser presenting this token enrol itself.',
  `# FLEET_TOKEN=${token()}`,
];

/** Every .env value for these answers. */
export function buildEnv(a: Answers, secrets: Secrets, existing: Env): BuiltEnv {
  const { v, apiKey } = baseSecrets(a, existing);
  Object.assign(v, secrets);
  if (a.llm.provider !== 'skip') Object.assign(v, { OPENAI_BASE_URL: a.llm.baseUrl, CHAT_MODEL: a.llm.model });
  fleetSettings(v, { a, existing, wsUrl: a.publicUrl.replace(/^http/, 'ws') + '/ws' });
  optionalSettings(v, a, existing);
  return { values: v, extra: fleetTokenLines(), apiKey };
}
