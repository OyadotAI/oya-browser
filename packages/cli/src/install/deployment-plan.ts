/** Validate replay files before they can become command arguments or saved installation plans. */
import {
  deploymentTarget,
  DEPLOYMENT_TARGETS,
  type DeploymentOptions,
  type DeploymentTarget,
} from './deployment-targets.ts';
import { DEPLOYMENT_PLAN_VERSION } from './constants.ts';

/** A reproducible deployment with credentials deliberately absent. */
export interface DeploymentPlan {
  /** Separates deployment examples from the original database/fleet interview. */
  version: typeof DEPLOYMENT_PLAN_VERSION;
  /** Folder containing the existing deployment script. */
  target: DeploymentTarget;
  /** Only the script's allowlisted nonsecret options. */
  options: DeploymentOptions;
  /** ECS HTTPS hostname; the ACM certificate does not cover the ALB's generated DNS name. */
  publicUrl?: string;
}

/** A plain record, never an array or null. */
function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

/** Reject credential-bearing or multiline flags without echoing their values. */
function validateOption(key: string, value: unknown, expected: string | undefined): void {
  if (!expected || typeof value !== expected) throw new Error(`Unsupported or invalid deployment option: ${key}.`);
  if (typeof value === 'string' && /[\r\n\0]/.test(value)) throw new Error(`Invalid deployment option: ${key}.`);
  if (typeof value === 'string' && /(?:postgres(?:ql)?:\/\/|https?:\/\/[^/]*@)/i.test(value))
    throw new Error(`Credentials cannot be saved in ${key}. Use OYA_DEPLOY_DATABASE_URL for Kubernetes Postgres.`);
}

/** One required relationship, stated without secret values. */
function requireOption(ok: unknown, message: string): void {
  if (!ok) throw new Error(message);
}

/** Kubernetes never guesses a context, image destination or database policy. */
function validateKubernetes(o: DeploymentOptions): void {
  requireOption(o.context, 'Kubernetes needs an explicit context. List yours with: kubectl config get-contexts');
  requireOption(!!o.registry !== !!o.image, 'Kubernetes needs exactly one of registry or image.');
  validateKubernetesTls(o);
  requireOption(
    !String(o.context).split('/').includes('..'),
    'Kubernetes context must not contain a parent path segment.',
  );
}

/** Public Kubernetes access must name one certificate source. */
function validateKubernetesTls(o: DeploymentOptions): void {
  const valid = !o.host || !!o['tls-secret'] !== !!o['cluster-issuer'];
  requireOption(valid, 'Kubernetes HTTPS needs exactly one TLS secret or cluster issuer.');
}

/** AWS trial HTTP must be intentional; a partial existing-VPC specification is unusable. */
function validateEcs(o: DeploymentOptions, publicUrl?: string): void {
  requireOption(
    !!o['certificate-arn'] !== !!o['http-only'],
    'ECS needs a certificate-arn or explicit http-only trial mode.',
  );
  requireOption(!o['certificate-arn'] || publicUrl, 'ECS HTTPS needs publicUrl matching your ACM certificate and DNS.');
  validateNetworking(o);
}

/** An existing VPC needs the complete public/private subnet layout. */
function validateNetworking(o: DeploymentOptions): void {
  const networking = [o['vpc-id'], o['public-subnets'], o['private-subnets']];
  requireOption(
    networking.every(Boolean) || !networking.some(Boolean),
    'An existing VPC needs vpc-id, public-subnets and private-subnets together.',
  );
}

/** Validate script values which are embedded into generated YAML or resource names. */
function validateValues(o: DeploymentOptions): void {
  for (const key of ['replicas', 'browser-ttl'])
    if (o[key]) requireOption(/^[1-9]\d*$/.test(String(o[key])), `${key} must be a whole number of 1 or more.`);
  validateImages(o);
  validateDnsValues(o);
}

/** Image references are inserted into generated deployment manifests. */
function validateImages(o: DeploymentOptions): void {
  for (const key of ['registry', 'image', 'browser-image', 'server-image'])
    if (o[key])
      requireOption(
        /^[\w][\w.:/@-]*$/.test(String(o[key])),
        `${key} must be a container image reference without credentials.`,
      );
}

/** Values inserted into Kubernetes YAML must stay DNS names. */
function validateDnsValues(o: DeploymentOptions): void {
  for (const key of ['namespace', 'host', 'domain', 'ingress-class', 'tls-secret', 'cluster-issuer'])
    if (o[key]) requireOption(/^[a-z0-9][a-z0-9.-]*$/.test(String(o[key])), `${key} must be a DNS name.`);
}

/** Share per-field validation with prompts so a typo can be corrected in place. */
export function deploymentFieldProblem(key: string, value: string): string | undefined {
  try {
    validateValues({ [key]: value });
  } catch (err) {
    return (err as Error).message;
  }
}

/** Only a public HTTPS origin can override ECS's generated load-balancer address. */
function validatePublicUrl(value: unknown): string | undefined {
  if (value === undefined) return;
  const url = safePublicUrl(value);
  requireOption(
    url?.protocol === 'https:' && !url.username && !url.password && url.pathname === '/' && !url.search && !url.hash,
    'publicUrl must be an HTTPS origin without credentials, a path or query.',
  );
  return url!.origin;
}

/** URL parser errors must not echo a malformed credential-bearing input. */
function safePublicUrl(value: unknown): URL | null {
  try {
    return typeof value === 'string' ? new URL(value) : null;
  } catch {
    return null;
  }
}

/** Strict replay validation prevents a saved plan from injecting extra script options. */
function planRecord(value: unknown): Record<string, unknown> {
  if (!record(value) || value.version !== DEPLOYMENT_PLAN_VERSION || !record(value.options))
    throw new Error('Expected a version 2 deployment plan with target and options.');
  for (const key of Object.keys(value))
    requireOption(
      ['version', 'target', 'options', 'publicUrl'].includes(key),
      `Unknown deployment plan field: ${key}.`,
    );
  return value;
}

/** Validate the option allowlist before adding the example's defaults. */
function validatedOptions(target: DeploymentTarget, value: unknown): DeploymentOptions {
  const spec = DEPLOYMENT_TARGETS[target];
  for (const [key, entry] of Object.entries(value as Record<string, unknown>))
    validateOption(key, entry, Object.hasOwn(spec.options, key) ? spec.options[key] : undefined);
  const options = { ...spec.defaults, ...(value as DeploymentOptions) };
  validateValues(options);
  return options;
}

/** Strict replay validation prevents a saved plan from injecting extra script options. */
export function parseDeploymentPlan(value: unknown): DeploymentPlan {
  const input = planRecord(value),
    target = deploymentTarget(input.target);
  const options = validatedOptions(target, input.options),
    publicUrl = validatePublicUrl(input.publicUrl);
  if (target === 'k8s') validateKubernetes(options);
  if (target === 'ecs') validateEcs(options, publicUrl);
  if (target === 'gcloud') requireOption(options.project, 'Google Cloud needs an explicit project.');
  return { version: DEPLOYMENT_PLAN_VERSION, target, options, ...(publicUrl ? { publicUrl } : {}) };
}
