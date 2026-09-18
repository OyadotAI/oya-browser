/**
 * Preflight: what the chosen deployment needs from this machine (Docker,
 * compose, Node, kubectl, psql, a free port), checked before anything is
 * written. A fatal failure stops a real install; a dry run only reports it.
 */
import { existsSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { atLeast, capture, has, portFree } from './shell.ts';
import type { Answers, Check } from './types.ts';
import { COMPOSE_MIN, DEFAULT_PORT, NODE_MIN, OCTAL, PERMISSION_BITS } from './constants.ts';

/**
 * The compose file uses the `env_file: [{path, required}]` form, added in 2.24.
 * fatal regardless of whether it is missing or merely old: 2.20 cannot parse
 * the `env_file: [{path, required}]` form in docker-compose.yml, and finding
 * that out at `docker compose up` is far from the check that knew about it.
 */
function composeCheck(compose: string | null): Check {
  const ok = atLeast(compose, COMPOSE_MIN.major, COMPOSE_MIN.minor);
  return { ok, label: 'docker compose ≥ 2.24', detail: compose || 'not found', fatal: true };
}

/** Docker, its daemon and a new enough compose. */
async function dockerChecks(): Promise<Check[]> {
  const version = await capture('docker', ['--version']);
  const daemon = await capture('docker', ['info', '--format', '{{.ID}}']);
  const compose = await capture('docker', ['compose', 'version', '--short']);
  return [
    { ok: !!version, label: 'docker', detail: version || 'not found', fatal: true },
    { ok: !!daemon, label: 'docker daemon reachable', detail: daemon ? 'ok' : 'not running', fatal: true },
    composeCheck(compose),
  ];
}

/** Outside Docker, the server runs on this Node, which needs node:sqlite. */
function nodeCheck(): Check {
  const ok = atLeast(process.versions.node, NODE_MIN.major, NODE_MIN.minor);
  return { ok, label: 'node ≥ 22.13 (node:sqlite)', detail: process.versions.node, fatal: true };
}

/** kubectl, and a cluster it can reach. */
async function kubeChecks(): Promise<Check[]> {
  const version = await capture('kubectl', ['version', '--client=true', '-o', 'json']);
  const cluster = await capture('kubectl', ['get', 'namespace', 'kube-system', '-o', 'name']);
  return [
    { ok: !!version, label: 'kubectl', detail: version ? 'ok' : 'not found', fatal: true },
    { ok: !!cluster, label: 'cluster reachable', detail: cluster || 'kubectl cannot reach a cluster', fatal: true },
  ];
}

/** The public URL's port is not already taken. */
async function portCheck(publicUrl: string): Promise<Check> {
  const port = Number(new URL(publicUrl).port || DEFAULT_PORT);
  const free = await portFree(port);
  return { ok: free, label: `port ${port} free`, detail: free ? 'ok' : 'something is already listening' };
}

/** An existing .env is kept, secrets and all; say so, with its mode. */
function envCheck(root: string): Check[] {
  const envPath = join(root, '.env');
  if (!existsSync(envPath)) return [];
  const mode = (statSync(envPath).mode & PERMISSION_BITS).toString(OCTAL);
  return [{ ok: true, label: '.env exists — keeping its OYA_PROFILE_SECRET and API keys', detail: `mode ${mode}` }];
}

/** psql, to apply migrations to Postgres or Supabase. */
const psqlCheck = async (): Promise<Check> => ({
  ok: await has('psql'),
  label: 'psql (to apply migrations)',
  detail: 'from libpq/postgresql-client',
  fatal: true,
});

/** Checks for the tools the host and fleet need. */
async function toolChecks(a: Answers): Promise<Check[]> {
  const checks: Check[] = [];
  const needsDocker = a.host === 'docker' || a.fleet === 'docker-workers' || a.fleet === 'oya-selfhosted';
  if (needsDocker) checks.push(...(await dockerChecks()));
  if (a.host !== 'docker') checks.push(nodeCheck());
  if (a.fleet === 'k8s' || a.host === 'k8s') checks.push(...(await kubeChecks()));
  if (a.database === 'postgres' || a.database === 'supabase') checks.push(await psqlCheck());
  return checks;
}

/** Every check for this deployment, in the order they are printed. */
export async function preflight(root: string, a: Answers): Promise<Check[]> {
  return [...(await toolChecks(a)), await portCheck(a.publicUrl), ...envCheck(root)];
}
