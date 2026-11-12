/** A short destination interview over the existing deployment scripts; replay never asks questions. */
import { ask, askSecret, choose, note, setupPhase, type Option } from '../prompt.ts';
import { deploymentQuestion } from './deployment-help.ts';
import {
  DEPLOYMENT_TARGETS,
  deploymentTarget,
  type DeploymentOptions,
  type DeploymentTarget,
} from './deployment-targets.ts';
import { parseDeploymentPlan, deploymentFieldProblem, type DeploymentPlan } from './deployment-plan.ts';
import { deploymentProbe } from './deployment-process.ts';
import { DEPLOYMENT_PLAN_VERSION } from './constants.ts';

/** Image selection describes a build destination rather than a second infrastructure choice. */
const IMAGE_OPTIONS: Option[] = [
  { id: 'registry', label: 'Build and push', note: 'Use a registry you can push to' },
  {
    id: 'image',
    label: 'Use a published image',
    note: 'Built from deployments/image',
  },
];
/** Managed Postgres is the production default; the local StatefulSet is an explicit trial choice. */
const DATABASE_OPTIONS: Option[] = [
  { id: 'managed', label: 'Managed Postgres', note: 'Production · use your existing database' },
  { id: 'cluster', label: 'Postgres in this cluster', note: 'Trial · create a persistent database pod' },
];
/** HTTP is a deliberate trial choice, never a fallback after certificate failure. */
const ACCESS_OPTIONS: Option[] = [
  { id: 'https', label: 'HTTPS', note: 'Production · use an ACM certificate' },
  { id: 'http', label: 'HTTP trial', note: 'unencrypted; for trying it out only' },
];
/** Optional customization stays off the normal path. */
const ADVANCED_OPTIONS: Option[] = [
  { id: 'no', label: 'Use the defaults', note: 'recommended; change settings later in oya.env' },
  { id: 'yes', label: 'Customize', note: 'replicas, images, networking and optional settings' },
];
/** Boolean script flags use the same keyboard menu as other decisions. */
const BOOLEAN_OPTIONS: Option[] = [
  { id: 'no', label: 'No' },
  { id: 'yes', label: 'Yes' },
];

/** A required destination field, its label and a useful default. */
type Question = [string, string, string];

/** Only location questions belong in the normal path; databases and runtimes follow the example. */
const QUESTIONS: Record<DeploymentTarget, Question[]> = {
  docker: [['domain', 'Domain (localhost for a local certificate):', 'localhost']],
  k8s: [
    ['context', 'Kubernetes context:', ''],
    ['namespace', 'Namespace:', 'oya'],
  ],
  ecs: [
    ['region', 'AWS region:', 'us-east-1'],
    ['stack', 'Stack name:', 'oya'],
  ],
  gcloud: [
    ['project', 'Google Cloud project ID:', ''],
    ['region', 'Region:', 'us-central1'],
    ['name', 'Deployment name:', 'oya'],
    ['domain', 'Domain (blank keeps access private):', ''],
  ],
};

/** Extra flags belong behind one explicit customization choice. */
const ADVANCED: Record<DeploymentTarget, string[]> = {
  docker: ['browser-image', 'server-image'],
  k8s: [
    'replicas',
    'host',
    'tls-secret',
    'cluster-issuer',
    'ingress-class',
    'browser-ttl',
    'browser-image',
    'env-file',
    'platform',
  ],
  ecs: [
    'replicas',
    'vpc-id',
    'public-subnets',
    'private-subnets',
    'allowed-cidr',
    'db-class',
    'single-az-db',
    'browser-image',
    'server-image',
    'env-file',
  ],
  gcloud: ['replicas', 'db-tier', 'single-zone-db', 'browser-image', 'env-file'],
};

/** Whether the operator is answering interactively or relying entirely on a plan/defaults. */
interface Session {
  /** Existing example selected. */
  target: DeploymentTarget;
  /** Arguments under construction. */
  options: DeploymentOptions;
  /** No secret or missing-field questions under --yes. */
  unattended: boolean;
}

/** The first decision describes actual working targets, without disabled promises. */
export async function chooseDeploymentTarget(): Promise<DeploymentTarget> {
  const options: Option[] = Object.entries(DEPLOYMENT_TARGETS).map(([id, value]) => ({
    id,
    label: value.label,
    note: value.note,
  }));
  return deploymentTarget(await choose('Where should Oya run?', options, deploymentQuestion('docker', 'target')));
}

/** A destination question validates required input without discarding previous answers. */
async function field(s: Session, key: string, label: string, fallback: string, required = true): Promise<void> {
  if (s.options[key] !== undefined || s.unattended) return;
  const presentation = deploymentQuestion(s.target, key, s.options);
  s.options[key] = await ask(label, fallback, { validate: (v) => fieldProblem(key, v, required), presentation });
}

/** Correct individual mistakes before the next destination question. */
function fieldProblem(key: string, value: string, required: boolean): string | undefined {
  if (required && !value) return 'Required for this destination.';
  if (/[\r\n\0]/.test(value)) return 'Use a single line.';
  return deploymentFieldProblem(key, value);
}

/** Kubernetes explicitly chooses the image source and how to provide Postgres. */
async function kubernetes(s: Session): Promise<void> {
  if (s.target !== 'k8s' || s.unattended) return;
  const source = await choose('Control-plane image:', IMAGE_OPTIONS, deploymentQuestion(s.target, 'image-source'));
  await field(s, source, source === 'registry' ? 'Registry repository:' : 'Deployment image:', '');
  const database = await choose('Postgres:', DATABASE_OPTIONS, deploymentQuestion(s.target, 'database-source'));
  s.options['in-cluster-postgres'] = database === 'cluster';
}

/** ECS distinguishes production HTTPS from an explicitly selected HTTP trial. */
async function ecs(s: Session): Promise<string | undefined> {
  if (s.target !== 'ecs' || s.unattended) return;
  const access = await choose('Public access:', ACCESS_OPTIONS, deploymentQuestion(s.target, 'access'));
  if (access === 'http') {
    s.options['http-only'] = true;
    return;
  }
  await field(s, 'certificate-arn', 'ACM certificate ARN:', '');
  return publicUrlQuestion();
}

/** The certificate hostname is required to test TLS without disabling verification. */
function publicUrlQuestion(): Promise<string> {
  return ask('Public HTTPS URL matching that certificate:', '', {
    presentation: deploymentQuestion('ecs', 'public-url'),
    validate: (v) =>
      /^https:\/\/[^\s/@]+\/?$/.test(v) ? undefined : 'Enter an HTTPS origin, such as https://oya.example.com.',
  });
}

/** Customization reuses exact script flag names, so manual and wizard installs stay interchangeable. */
async function advanced(s: Session): Promise<void> {
  if (s.unattended) return;
  const choice = await choose('Advanced settings?', ADVANCED_OPTIONS, deploymentQuestion(s.target, 'advanced'));
  if (choice === 'no') return;
  for (const key of ADVANCED[s.target]) await advancedField(s, key);
}

/** Text and boolean flags have different input controls. */
async function advancedField(s: Session, key: string): Promise<void> {
  if (DEPLOYMENT_TARGETS[s.target].options[key] === 'boolean')
    s.options[key] = (await choose(`${key}?`, BOOLEAN_OPTIONS, deploymentQuestion(s.target, key, s.options))) === 'yes';
  else await field(s, key, `${key} (blank keeps the default):`, '', false);
}

/** Ask only for this target; cloud infrastructure details are otherwise owned by the scripts. */
export async function interviewDeployment(target: DeploymentTarget, unattended: boolean): Promise<DeploymentPlan> {
  const s: Session = { target, unattended, options: unattended ? { ...DEPLOYMENT_TARGETS[target].defaults } : {} };
  setupPhase('Configure');
  for (const [key, label, fallback] of QUESTIONS[target]) await field(s, key, label, fallback, key !== 'domain');
  await kubernetes(s);
  const publicUrl = await ecs(s);
  await advanced(s);
  return finishInterview(s, publicUrl);
}

/** Keep constructing the replay envelope separate from the sequence of questions. */
function finishInterview(s: Session, publicUrl?: string): DeploymentPlan {
  return parseDeploymentPlan({
    version: DEPLOYMENT_PLAN_VERSION,
    target: s.target,
    options: s.options,
    ...(publicUrl ? { publicUrl } : {}),
  });
}

/** Keep the Postgres password outside plans, previews and argv. */
export async function deploymentDatabase(plan: DeploymentPlan, unattended: boolean): Promise<string> {
  if (plan.target !== 'k8s' || plan.options['in-cluster-postgres']) return '';
  const existing = process.env.OYA_DEPLOY_DATABASE_URL || process.env.DATABASE_URL;
  if (existing) return checkedDatabase(existing);
  return askDatabase(unattended);
}

/** Reject malformed environment credentials before the script creates any resources. */
function checkedDatabase(value: string): string {
  const url = databaseUrl(value);
  if (!url || !['postgres:', 'postgresql:'].includes(url.protocol) || !url.hostname || /[\r\n\0]/.test(value))
    throw new Error('OYA_DEPLOY_DATABASE_URL must be a Postgres connection string. Its value was not printed.');
  return value;
}

/** Preserve Node 20 support and avoid URL parser errors that echo a secret. */
function databaseUrl(value: string): URL | null {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

/** Replay fails explicitly instead of reading secrets from a closed terminal. */
function askDatabase(unattended: boolean): Promise<string> {
  if (unattended)
    throw new Error(
      'Managed Kubernetes Postgres needs OYA_DEPLOY_DATABASE_URL in the environment; it is never saved in the plan.',
    );
  return askSecret('Postgres connection string:', {
    presentation: deploymentQuestion('k8s', 'database-url'),
    validate: (v) => (/^postgres(?:ql)?:\/\/\S+$/.test(v) ? undefined : 'Enter a postgres:// connection string.'),
  });
}

/** Expose the current context as a hint, never silently select it as a deployment destination. */
export async function showContextHint(target: DeploymentTarget): Promise<void> {
  if (target !== 'k8s') return;
  const current = await deploymentProbe('kubectl', ['config', 'current-context']);
  if (current) note(`Current context: ${current}. Confirm it by entering the context below.`);
}
