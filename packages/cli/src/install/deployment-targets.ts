/** Existing deployment entry points and the destination flags shared by their day-two commands. */

/** A deployable example; these names also identify its folder under deployments/. */
export type DeploymentTarget = 'docker' | 'k8s' | 'ecs' | 'gcloud';

/** Nonsecret flags passed to a deployment script without shell interpolation. */
export type DeploymentOptions = Record<string, string | boolean>;

/** The script's public capabilities, kept together so the wizard cannot advertise a placeholder. */
export interface DeploymentTargetSpec {
  /** Menu name. */
  label: string;
  /** What the operator gets. */
  note: string;
  /** Flags accepted by deploy, with their expected value type. */
  options: Record<string, 'string' | 'boolean'>;
  /** Safe defaults matching the example. */
  defaults: DeploymentOptions;
  /** Destination flags accepted by status, logs, smoke and keys. */
  globals: string[];
  /** Extra installed tools. */
  tools: string[];
  /** Infrastructure created by the example. */
  resources: string;
  /** Where existing secrets are preserved. */
  secrets: string;
}

/** Working deployment examples; never include unavailable providers in this menu. */
export const DEPLOYMENT_TARGETS: Record<DeploymentTarget, DeploymentTargetSpec> = {
  docker: {
    label: 'Docker',
    note: 'This machine · Postgres + HTTPS',
    options: { domain: 'string', 'browser-image': 'string', 'server-image': 'string' },
    defaults: { domain: 'localhost' },
    globals: [],
    tools: [],
    resources: 'Docker Compose: control plane, Postgres, Caddy, browser containers',
    secrets: 'deployments/docker/.env and Docker data volumes (kept on redeploy)',
  },
  k8s: {
    label: 'Kubernetes',
    note: 'Existing cluster · browser pods',
    options: {
      context: 'string',
      namespace: 'string',
      registry: 'string',
      image: 'string',
      platform: 'string',
      'in-cluster-postgres': 'boolean',
      host: 'string',
      'tls-secret': 'string',
      'cluster-issuer': 'string',
      'ingress-class': 'string',
      replicas: 'string',
      'browser-ttl': 'string',
      'browser-image': 'string',
      'env-file': 'string',
    },
    defaults: { namespace: 'oya' },
    globals: ['context', 'namespace'],
    tools: ['kubectl', 'python3'],
    resources: 'Namespace, control plane, RBAC, isolated browser pods; managed or in-cluster Postgres',
    secrets: 'Kubernetes Secrets oya-secrets and oya-settings (kept on redeploy)',
  },
  ecs: {
    label: 'Amazon ECS',
    note: 'AWS Fargate + RDS',
    options: {
      region: 'string',
      stack: 'string',
      'certificate-arn': 'string',
      'http-only': 'boolean',
      'vpc-id': 'string',
      'public-subnets': 'string',
      'private-subnets': 'string',
      'allowed-cidr': 'string',
      replicas: 'string',
      'db-class': 'string',
      'single-az-db': 'boolean',
      'browser-image': 'string',
      'server-image': 'string',
      'env-file': 'string',
    },
    defaults: { region: 'us-east-1', stack: 'oya' },
    globals: ['region', 'stack'],
    tools: ['aws'],
    resources: 'CloudFormation: VPC, ECS/Fargate, HTTPS load balancer, RDS Postgres, IAM and ECR',
    secrets: 'AWS Secrets Manager and RDS (kept on redeploy)',
  },
  gcloud: {
    label: 'Google Cloud',
    note: 'GKE Autopilot + Cloud SQL',
    options: {
      project: 'string',
      region: 'string',
      name: 'string',
      domain: 'string',
      replicas: 'string',
      'db-tier': 'string',
      'single-zone-db': 'boolean',
      'browser-image': 'string',
      'env-file': 'string',
    },
    defaults: { region: 'us-central1', name: 'oya' },
    globals: ['project', 'region', 'name'],
    tools: ['gcloud', 'kubectl', 'python3', 'gke-gcloud-auth-plugin'],
    resources: 'GKE Autopilot, Cloud SQL, Artifact Registry, Workload Identity; optional HTTPS',
    secrets: 'Kubernetes Secrets and Cloud SQL (kept on redeploy)',
  },
};

/** Refuse unknown targets before using one as a path or map key. */
export function deploymentTarget(value: unknown): DeploymentTarget {
  if (typeof value !== 'string' || !Object.hasOwn(DEPLOYMENT_TARGETS, value))
    throw new Error('Choose a deployment target: docker, k8s, ecs, or gcloud.');
  return value as DeploymentTarget;
}

/** Turn validated flags into argv; false flags and empty optional strings are omitted. */
export function optionArguments(options: DeploymentOptions): string[] {
  return Object.entries(options).flatMap(([key, value]) => {
    if (!value) return [];
    return value === true ? [`--${key}`] : [`--${key}`, value];
  });
}

/** Scope every day-two command to the same context, account region, namespace or project. */
export function destinationOptions(target: DeploymentTarget, options: DeploymentOptions): DeploymentOptions {
  return Object.fromEntries(
    DEPLOYMENT_TARGETS[target].globals.filter((key) => options[key]).map((key) => [key, options[key]]),
  );
}
