/** Honest deployment progress, public-access limits and scoped day-two commands. */
import { note, success, warn, setupSection } from '../prompt.ts';
import { DEPLOYMENT_TARGETS, destinationOptions, optionArguments } from './deployment-targets.ts';
import type { DeploymentPlan } from './deployment-plan.ts';
import type { Check } from './types.ts';
import { deploymentHelp } from './deployment-help.ts';

/** Location fields are separated from resource sizing and image overrides. */
const DESTINATION_FIELDS = new Set(['context', 'namespace', 'region', 'stack', 'project', 'name']);
/** Public access settings form their own readable group. */
const ACCESS_FIELDS = new Set([
  'domain',
  'host',
  'tls-secret',
  'cluster-issuer',
  'ingress-class',
  'http-only',
  'certificate-arn',
  'allowed-cidr',
]);

/** Quote a nonsecret argument for copying to a POSIX shell. */
export const quoteDeploymentArg = (value: string): string => `'${value.replaceAll("'", "'\\''")}'`;

/** The manual equivalent remains visible so the wizard never hides which example it uses. */
export function deploymentCommand(p: DeploymentPlan, command: string): string {
  const args = optionArguments(destinationOptions(p.target, p.options));
  const line = ['bash', `deployments/${p.target}/deploy.sh`, command, ...args].map(quoteDeploymentArg).join(' ');
  return p.publicUrl && command === 'smoke' ? `OYA_DEPLOY_PUBLIC_URL=${quoteDeploymentArg(p.publicUrl)} ${line}` : line;
}

/** Before provisioning, show destination, resource scope, retained secrets and what is verified. */
export function printDeploymentPlan(p: DeploymentPlan, root: string): void {
  setupSection('Destination', [
    ['Target', DEPLOYMENT_TARGETS[p.target].label],
    ...planFields(p, (key) => DESTINATION_FIELDS.has(key)),
  ]);
  printResourcePlan(p, root);
  printAccessPlan(p);
  printVerificationPlan(p);
}

/** Keep infrastructure, preserved secrets and optional overrides together. */
function printResourcePlan(p: DeploymentPlan, root: string): void {
  setupSection('Resources', [
    ['Creates or updates', DEPLOYMENT_TARGETS[p.target].resources],
    ['Checkout', root],
    ['Secrets', DEPLOYMENT_TARGETS[p.target].secrets],
    ...planFields(p, (key) => !DESTINATION_FIELDS.has(key) && !ACCESS_FIELDS.has(key)),
  ]);
}

/** Public access can remain pending while a private cluster is verified. */
function printAccessPlan(p: DeploymentPlan): void {
  setupSection('Access', [...planFields(p, (key) => ACCESS_FIELDS.has(key)), ['Public URL', plannedAddress(p)]]);
}

/** Show the intended address without claiming DNS or certificate verification. */
function plannedAddress(p: DeploymentPlan): string {
  if (p.publicUrl) return p.publicUrl;
  if (p.target === 'docker') return `https://${p.options.domain || 'localhost'}`;
  if (p.options.host || p.options.domain) return `https://${p.options.host || p.options.domain}`;
  return p.target === 'ecs' ? 'Assigned by AWS after deploy' : 'Private · use a port-forward';
}

/** Friendly labels are shared with the question catalog rather than exposing raw flag names. */
function planFields(p: DeploymentPlan, include: (key: string) => boolean): Array<[string, string]> {
  return Object.entries(p.options)
    .filter(([key, value]) => value && include(key))
    .map(([key, value]) => {
      const label = key === 'http-only' ? 'HTTP trial' : deploymentHelp(p.target, key).title;
      return [label, value === true ? 'Yes' : String(value)];
    });
}

/** Explain readiness and cloud cost at the point where an operator reviews the plan. */
function printVerificationPlan(p: DeploymentPlan): void {
  setupSection('Verification', [
    ['Control plane', 'Wait for rollout and /readyz'],
    ['Browser', 'Start → connect → navigate → stop'],
  ]);
  if (p.target === 'ecs' || p.target === 'gcloud')
    note('Cloud resources are billable. Existing secrets and data are retained on redeploy.');
}

/** A skipped preview check is visibly distinct from verified readiness. */
export function printDeploymentChecks(checks: Check[]): void {
  const rows: Array<[string, string]> = checks.map((check) => [check.label, checkResult(check)]);
  setupSection('Preflight', rows);
}

/** Keep execution flags unchanged while presenting preview uncertainty honestly. */
function checkResult(check: Check): string {
  if (/not checked/i.test(check.detail || '')) return `Skipped · ${check.detail}`;
  const status = check.ok ? 'Ready' : check.fatal ? 'Required' : 'Check';
  return `${status}${check.detail ? ` · ${check.detail}` : ''}`;
}

/** Endpoint reporting distinguishes a verified service from public DNS or certificate propagation. */
function endpoint(p: DeploymentPlan): void {
  directEndpoint(p);
  clusterEndpoint(p);
  if ((p.target === 'k8s' && !p.options.host) || (p.target === 'gcloud' && !p.options.domain)) privateAccess(p);
}

/** Direct deployments verify the same URL the operator opens. */
function directEndpoint(p: DeploymentPlan): void {
  if (p.target === 'docker')
    note(
      `Endpoint: https://${p.options.domain || 'localhost'}${p.options.domain === 'localhost' ? ' (local certificate)' : ''}`,
    );
  if (p.target === 'ecs') note(`Endpoint: ${p.publicUrl || 'the load-balancer URL printed above'}`);
}

/** Cluster smoke can finish before public DNS and TLS propagation. */
function clusterEndpoint(p: DeploymentPlan): void {
  if (p.target === 'k8s' && p.options.host)
    note(`Public URL: https://${p.options.host}; DNS/TLS must resolve separately.`);
  if (p.target === 'gcloud' && p.options.domain)
    note(
      `Public URL: https://${p.options.domain}; follow the DNS instructions above; certificate issuance may still be pending.`,
    );
}

/** Give a copyable path to a private deployment without inventing a public URL. */
function privateAccess(p: DeploymentPlan): void {
  const context =
    p.target === 'k8s' ? String(p.options.context) : `gke_${p.options.project}_${p.options.region}_${p.options.name}`;
  const namespace = String(p.options.namespace || 'oya');
  const args = ['kubectl', '--context', context, '-n', namespace, 'port-forward', 'svc/oya-control-plane', '3100:3100'];
  note('Access is private. To open the dashboard at http://localhost:3100:');
  note(args.map(quoteDeploymentArg).join(' '));
}

/** Always retain scoped recovery commands; neither a failed smoke nor an interrupted deploy destroys data. */
export function printDeploymentRecovery(p: DeploymentPlan, root: string): void {
  note(`Run these from ${root}:`);
  for (const command of ['status', 'logs', 'keys', 'smoke']) note(deploymentCommand(p, command));
  note(`Retry: oya install --config ${quoteDeploymentArg(`${root}/oya-install.json`)}`);
}

/** Success belongs to the installer only after the example's smoke test has completed. */
export function printDeploymentDone(p: DeploymentPlan, root: string): void {
  success('Oya is ready. Control-plane readiness and browser smoke check passed.');
  if (p.target === 'k8s' || p.target === 'gcloud')
    note('Verified through a private port-forward; public DNS/TLS was not tested.');
  endpoint(p);
  note('Sign in with the API key printed by the deployment, or retrieve it with the keys command.');
  printDeploymentRecovery(p, root);
}

/** A partial deployment is recoverable and never mislabeled as success. */
export function printDeploymentIncomplete(p: DeploymentPlan, root: string): void {
  warn('Deployment verification incomplete. Existing resources and secrets were kept.');
  printDeploymentRecovery(p, root);
}
