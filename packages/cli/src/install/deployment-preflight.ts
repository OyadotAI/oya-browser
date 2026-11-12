/** Read-only checks for the existing example, without local ports or workstation migrations. */
import { existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { deploymentProbe } from './deployment-process.ts';
import { DEPLOYMENT_TARGETS } from './deployment-targets.ts';
import type { DeploymentPlan } from './deployment-plan.ts';
import type { Check } from './types.ts';
import { atLeast } from './shell.ts';
import { COMPOSE_MIN } from './constants.ts';

/** Prerequisite commands have different version interfaces. */
const VERSION_ARGS: Record<string, string[]> = {
  openssl: ['version'],
  kubectl: ['version', '--client=true', '-o', 'json'],
};
/** Installation guidance stays actionable without printing a cloud error payload. */
const REMEDIES: Record<string, string> = {
  docker: 'Install Docker Desktop or Docker Engine, then start it.',
  aws: 'Install AWS CLI v2 and run aws sso login or configure your AWS identity.',
  gcloud: 'Install the Google Cloud CLI and run gcloud auth login.',
  kubectl: 'Install kubectl and configure access to your cluster.',
  'gke-gcloud-auth-plugin': 'Install the Google Cloud gke-gcloud-auth-plugin component.',
};

/** Fixed portions of read-only credential probes. */
const KUBE_PROBE_ARGS = ['get', 'namespace', 'kube-system', '-o', 'name', '--request-timeout=10s'];
/** AWS returns the owning account without printing credentials. */
const AWS_PROBE_ARGS = ['sts', 'get-caller-identity', '--query', 'Account', '--output', 'text'];
/** Preview cannot establish destination permissions. */
const PREVIEW_CHECK: Check = {
  ok: true,
  label: 'Cloud credentials and destination access',
  detail: 'not checked in preview; checked before a real deploy',
};

/** Check one program with a deadline and without exposing stderr. */
async function toolCheck(tool: string): Promise<Check> {
  const args = Object.hasOwn(VERSION_ARGS, tool) ? VERSION_ARGS[tool] : ['--version'];
  const value = await deploymentProbe(tool, args),
    ok = value !== null;
  const remedy = REMEDIES[tool] || `Install ${tool} and put it on PATH.`;
  return { ok, label: tool, fatal: true, detail: ok ? 'available' : remedy };
}

/** A required daemon/plugin check reports a remedy rather than raw tool output. */
async function dockerCheck(args: string[], label: string, remedy: string): Promise<Check> {
  const result = await deploymentProbe('docker', args);
  return { ok: !!result, label, fatal: true, detail: result ? 'available' : remedy };
}

/** Every example builds or resolves images, so a daemon and buildx are required. */
async function dockerChecks(target: string): Promise<Check[]> {
  const checks = [
    await dockerCheck(['info', '--format', '{{.ID}}'], 'Docker daemon', 'Start Docker Desktop or the Docker daemon.'),
    await dockerCheck(['buildx', 'version'], 'Docker buildx', 'Install the Docker buildx plugin.'),
  ];
  if (target === 'docker') checks.push(await composeCheck());
  return checks;
}

/** Optional env-file support requires a recent Compose. */
async function composeCheck(): Promise<Check> {
  const version = await deploymentProbe('docker', ['compose', 'version', '--short']);
  const ok = atLeast(version, COMPOSE_MIN.major, COMPOSE_MIN.minor);
  return { ok, label: 'Docker Compose ≥ 2.24', fatal: true, detail: 'Update Docker Compose if this check fails.' };
}

/** Inspect only the explicitly selected cluster. */
async function kubernetesIdentity(p: DeploymentPlan): Promise<Check> {
  const args = ['--context', String(p.options.context), ...KUBE_PROBE_ARGS];
  return identity('kubectl', args, 'Kubernetes context', 'Check the context and your cluster access.');
}

/** Display the account that will own billable AWS resources. */
async function awsIdentity(p: DeploymentPlan): Promise<Check> {
  const args = ['--region', String(p.options.region), ...AWS_PROBE_ARGS];
  return identity('aws', args, 'AWS account', 'Sign in with aws sso login and check the selected region.');
}

/** Never use the active gcloud project implicitly. */
async function googleIdentity(p: DeploymentPlan): Promise<Check> {
  const project = String(p.options.project),
    args = ['--project', project, 'projects', 'describe', project, '--format=value(projectId)'];
  return identity('gcloud', args, 'Google Cloud project', 'Run gcloud auth login and check project permissions.');
}

/** Read-only destination checks selected in one place. */
const IDENTITIES = { docker: async () => [], k8s: kubernetesIdentity, ecs: awsIdentity, gcloud: googleIdentity };

/** Identity evidence never includes credential-bearing stderr. */
async function identity(command: string, args: string[], label: string, remedy: string): Promise<Check> {
  const result = await deploymentProbe(command, args);
  return { ok: !!result, label, fatal: true, detail: result || remedy };
}

/** Optional settings are checked without opening or printing their contents. */
function settingsChecks(root: string, p: DeploymentPlan): Check[] {
  const settings = p.options['env-file'];
  if (!settings) return [];
  const ok = existsSync(resolve(root, String(settings)));
  return [{ ok, label: 'Optional settings file', fatal: true, detail: 'The selected env-file must exist.' }];
}

/** Deployment needs a complete checkout containing the chosen example. */
function fileChecks(root: string, p: DeploymentPlan): Check[] {
  const script = join(root, 'deployments', p.target, 'deploy.sh'),
    ok = existsSync(script);
  const detail = 'Use an up-to-date Oya checkout containing deployments/.';
  return [{ ok, label: `deployments/${p.target}/deploy.sh`, fatal: true, detail }, ...settingsChecks(root, p)];
}

/** Previews omit credential-dependent requests and report that limit explicitly. */
async function destinationChecks(p: DeploymentPlan, preview: boolean, toolsReady: boolean): Promise<Check[]> {
  if (preview) return [PREVIEW_CHECK];
  if (!toolsReady || !Object.hasOwn(IDENTITIES, p.target)) return [];
  return [await IDENTITIES[p.target](p)].flat();
}

/** Match actual script prerequisites, adding destination authorization only for real runs. */
export async function deploymentPreflight(root: string, p: DeploymentPlan, preview: boolean): Promise<Check[]> {
  const tools = ['bash', 'git', 'docker', 'jq', 'openssl', 'curl', ...DEPLOYMENT_TARGETS[p.target].tools];
  const checks = await Promise.all(tools.map(toolCheck));
  checks.push(...(await dockerChecks(p.target)), ...fileChecks(root, p));
  const ready = checks.every((c) => c.ok);
  checks.push(...(await destinationChecks(p, preview, ready)));
  return checks;
}
