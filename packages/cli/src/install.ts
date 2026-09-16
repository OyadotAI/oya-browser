/**
 * oya install — stand up a self-hosted control plane.
 *
 * `oya login` + `oya init` configure a control plane that already exists, against
 * one API key. This command creates the control plane itself: the database, the
 * server, the browser fleet, the LLM and the optional services — then hands off
 * to those two, so provider questions are asked once, by the code that owns them.
 *
 * Everything it decides is written to oya-install.json (no secrets), so a second
 * run, a CI run or a colleague reproduces the same deployment with --config.
 */

import { randomBytes } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync, chmodSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { spawn } from 'node:child_process';
import { createConnection } from 'node:net';
import {
  ask, askSecret, choose, confirm, banner, steps, step, note, success, warn,
  spinner, style, icon, InputError, type Option,
} from './prompt.js';

// ── Shape ───────────────────────────────────────────────────────────────────

export interface Answers {
  version: 1;
  host: string;
  database: string;
  fleet: string;
  workers: number;
  k8sFleet?: { namespace: string; image: string; controlUrl: string; proxyUrl: string };
  llm: { provider: string; baseUrl: string; model: string };
  publicUrl: string;
  optional: { captcha: string; recordingBucket: string; metrics: boolean };
}

/** Values that must never reach oya-install.json. */
type Secrets = Record<string, string>;

const ANSWERS_FILE = 'oya-install.json';
const token = () => randomBytes(32).toString('base64url');

/**
 * Values that must never be printed. Named explicitly rather than matched on the
 * variable name: DATABASE_URL embeds a password and contains none of the words a
 * pattern would look for.
 */
const SECRET_KEYS = new Set([
  'OYA_PROFILE_SECRET', 'OYA_OPERATOR_TOKEN', 'OYA_METRICS_TOKEN', 'API_KEYS', 'FLEET_TOKEN',
  'OYA_CLUSTER_SECRET', 'DATABASE_URL', 'SUPABASE_SERVICE_KEY', 'OPENAI_API_KEY',
  'OYA_CLOUD_API_KEY', 'ANCHOR_API_KEY', 'BROWSERBASE_API_KEY', 'STEEL_API_KEY',
  'BROWSERUSE_API_KEY', 'OYA_CAPTCHA_API_KEY', 'OYA_CDP_WS_URL',
]);

// ── Shell ───────────────────────────────────────────────────────────────────

/** Streams, because `npm ci` and `docker build` are slow enough that silence reads as a hang. */
function run(cmd: string, args: string[], opts: { cwd: string; quiet?: boolean } = { cwd: process.cwd() }): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd: opts.cwd, stdio: opts.quiet ? 'ignore' : 'inherit' });
    child.on('error', reject);
    child.on('close', (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} ${args.join(' ')} exited ${code}`))));
  });
}

async function capture(cmd: string, args: string[], cwd = process.cwd()): Promise<string | null> {
  return new Promise((resolve) => {
    const child = spawn(cmd, args, { cwd, stdio: ['ignore', 'pipe', 'ignore'] });
    let out = '';
    child.stdout.on('data', (d) => { out += d; });
    child.on('error', () => resolve(null));
    child.on('close', (code) => resolve(code === 0 ? out.trim() : null));
  });
}

const has = async (cmd: string) => (await capture(cmd, ['--version'])) !== null;

/** Compare a leading "x.y[.z]" against a floor. Digit-wise, so 5.0 beats 2.24. */
function atLeast(version: string | null, major: number, minor: number): boolean {
  const m = /(\d+)\.(\d+)/.exec(version || '');
  if (!m) return false;
  return Number(m[1]) > major || (Number(m[1]) === major && Number(m[2]) >= minor);
}

function portFree(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = createConnection({ port, host: '127.0.0.1' })
      .on('connect', () => { socket.destroy(); resolve(false); })
      .on('error', () => resolve(true));
    setTimeout(() => { socket.destroy(); resolve(true); }, 700);
  });
}

// ── Repo ────────────────────────────────────────────────────────────────────

/** The wizard drives this repo's compose file and manifests, so it needs the checkout. */
function findRepoRoot(from = process.cwd()): string | null {
  let dir = from;
  for (let i = 0; i < 8; i++) {
    if (existsSync(join(dir, 'docker-compose.yml')) && existsSync(join(dir, 'server', 'src', 'index.js'))) return dir;
    const up = dirname(dir);
    if (up === dir) break;
    dir = up;
  }
  return null;
}

async function locateRepo(): Promise<string> {
  const found = findRepoRoot();
  if (found) return found;
  console.log('\nThis does not look like an oya-browser checkout.');
  const where = await ask('Clone it to:', join(process.cwd(), 'oya-browser'));
  if (existsSync(where)) throw new Error(`${where} already exists`);
  await run('git', ['clone', '--depth', '1', 'https://github.com/OyadotAI/oya-browser.git', where], { cwd: process.cwd() });
  return where;
}

// ── .env ────────────────────────────────────────────────────────────────────

/** Parse just enough to know what a previous install already decided. */
function readEnv(path: string): Record<string, string> {
  if (!existsSync(path)) return {};
  const out: Record<string, string> = {};
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    const m = /^([A-Z0-9_]+)=(.*)$/.exec(line.trim());
    if (m) out[m[1]] = m[2];
  }
  return out;
}

const SECTIONS: Array<[string, string[]]> = [
  ['Server', ['PORT', 'OYA_UI_MODE', 'OYA_DATA_DIR']],
  ['Tenant keys — each is an identity; add more from the dashboard', ['API_KEYS']],
  ['Host operations: /metrics, drain, fleet provision', ['OYA_OPERATOR_TOKEN', 'OYA_METRICS_TOKEN']],
  ['Credentials at rest. Lose this and every stored cookie, proxy and TOTP seed is unreadable', ['OYA_PROFILE_SECRET']],
  ['Database', ['SUPABASE_URL', 'SUPABASE_SERVICE_KEY', 'DATABASE_URL', 'OYA_RECORDING_BUCKET']],
  ['Agent LLM. A private or http base URL is operator-only: the dashboard and\n# POST /config/host reject one, on purpose — tenants can set that field too', ['OPENAI_API_KEY', 'OPENAI_BASE_URL', 'CHAT_MODEL']],
  ['Browser fleet', ['OYA_BROWSER_PROVIDER', 'OYA_FLEET_RUNTIME', 'OYA_PUBLIC_WS_URL', 'OYA_CDP_WS_URL']],
  ['Governed Docker runtime', ['OYA_MANAGED_NETWORK', 'OYA_MANAGED_IMAGE', 'OYA_MANAGED_CONTROL_URL', 'OYA_MANAGED_PROXY_URL', 'OYA_MANAGED_REGION', 'OYA_EGRESS_PORT', 'OYA_EGRESS_HOST']],
  ['Oya Cloud', ['OYA_CLOUD_API_KEY', 'OYA_CLOUD_SNAPSHOT', 'OYA_CLOUD_TARGET']],
  ['Hosted CDP vendors', ['BROWSERBASE_API_KEY', 'BROWSERBASE_PROJECT_ID', 'STEEL_API_KEY', 'ANCHOR_API_KEY', 'BROWSERUSE_API_KEY']],
  ['CAPTCHA', ['OYA_CAPTCHA_PROVIDER', 'OYA_CAPTCHA_API_KEY']],
  ['Multi-replica. One SQLite control database allows exactly one writer', ['OYA_INSTANCE_URL', 'OYA_CLUSTER_SECRET']],
];

function renderEnv(values: Record<string, string>, extra: string[]): string {
  const lines = ['# Written by `oya install`. Re-run it, or edit by hand — both are fine.', ''];
  const placed = new Set<string>();
  for (const [title, keys] of SECTIONS) {
    const present = keys.filter((k) => values[k] !== undefined && values[k] !== '');
    if (!present.length) continue;
    lines.push(`# ── ${title} ──`);
    for (const k of present) { lines.push(`${k}=${values[k]}`); placed.add(k); }
    lines.push('');
  }
  const leftover = Object.keys(values).filter((k) => !placed.has(k) && values[k]);
  if (leftover.length) {
    lines.push('# ── Other ──');
    for (const k of leftover) lines.push(`${k}=${values[k]}`);
    lines.push('');
  }
  if (extra.length) lines.push(...extra, '');
  return lines.join('\n');
}

/** 0600: this file holds the KEK and every provider credential. */
function writeEnv(path: string, body: string): void {
  writeFileSync(path, body, { mode: 0o600 });
  chmodSync(path, 0o600);
}

// ── LLM ─────────────────────────────────────────────────────────────────────

const LLM_PRESETS: Record<string, { base: string; model: string; label: string }> = {
  anthropic: { base: 'https://api.anthropic.com/v1', model: 'claude-sonnet-4-5', label: 'Anthropic (Claude)' },
  openai: { base: 'https://api.openai.com/v1', model: 'gpt-4o-mini', label: 'OpenAI' },
  gemini: { base: 'https://generativelanguage.googleapis.com/v1beta/openai', model: 'gemini-3.8-flash', label: 'Gemini (Google)' },
  // Express mode: a global endpoint, no GCP project or location needed.
  vertex: { base: 'https://aiplatform.googleapis.com/v1/publishers/google', model: 'gemini-2.5-flash', label: 'Gemini Enterprise (Vertex AI)' },
};

async function askLlm(): Promise<{ answers: Answers['llm']; key: string }> {
  step('Agent LLM', 'Drives `oya ask` and the chat API.');
  note('CDP, the SDK and MCP all work without one.');
  const provider = await choose('Which LLM should agents use?', [
    { id: 'anthropic', label: 'Anthropic (Claude)' },
    { id: 'openai', label: 'OpenAI' },
    { id: 'gemini', label: 'Gemini (Google)' },
    { id: 'vertex', label: 'Gemini Enterprise (Vertex AI)', note: 'express-mode API key' },
    { id: 'compatible', label: 'An OpenAI-compatible endpoint', note: 'OpenRouter, Together, Groq, Azure' },
    { id: 'local', label: 'A local model', note: 'Ollama, vLLM, LM Studio' },
    { id: 'skip', label: 'Skip', note: 'no agent control; add it later with `oya config`' },
  ]);
  if (provider === 'skip') return { answers: { provider, baseUrl: '', model: '' }, key: '' };

  const preset = LLM_PRESETS[provider];
  const baseUrl = preset
    ? preset.base
    : (await ask('Base URL:', provider === 'local' ? 'http://localhost:11434/v1' : 'https://openrouter.ai/api/v1')).replace(/\/+$/, '');
  const model = await ask('Model:', preset?.model || (provider === 'local' ? 'qwen2.5' : ''));
  // A local model on loopback usually needs no key at all.
  for (;;) {
    const key = await askSecret(provider === 'local' ? 'API key (blank if none):' : 'API key:');
    if (!key) return { answers: { provider, baseUrl, model }, key: '' };
    const problem = await verifyLlm(baseUrl, key, model);
    if (!problem) return { answers: { provider, baseUrl, model }, key };
    warn(problem);
    // Losing every previous answer over one mistyped key would be absurd.
    const what = await choose('What now?', [
      { id: 'retry', label: 'Enter the key again' },
      { id: 'skip', label: 'Continue without an LLM', note: 'add it later with `oya config`' },
    ]);
    if (what === 'skip') return { answers: { provider: 'skip', baseUrl: '', model: '' }, key: '' };
  }
}

/**
 * Prove the credential before writing it, the way `oya login` does. A network
 * failure is not a bad key, so it warns rather than aborting the install.
 */
async function verifyLlm(baseUrl: string, key: string, model: string): Promise<string | null> {
  const spin = spinner('checking the key');
  try {
    // Anthropic authenticates with x-api-key and requires a version header; with
    // a valid key but no version it answers 400, which reads as "cannot verify"
    // when the key was in fact fine.
    const anthropic = /(^|\.)anthropic\.com$/i.test(new URL(baseUrl).hostname);
    const headers: Record<string, string> = anthropic
      ? { 'x-api-key': key, 'anthropic-version': '2023-06-01' }
      : { Authorization: `Bearer ${key}` };

    // Gemini Enterprise express mode has no model listing (its /models 404s) and takes
    // its key only in the query string — a header or bearer token answers 401. So it is
    // verified by the cheapest real generation instead, which does separate a bad key
    // (401) from a good one (200). Matched on the endpoint shape, the same way
    // server/src/llm.js routes: Gemini's other endpoint is an OpenAI shim that wants a
    // bearer token.
    const res = /\/publishers\/google\/?$/.test(baseUrl)
      ? await fetch(`${baseUrl.replace(/\/+$/, '')}/models/${encodeURIComponent(model || 'gemini-2.5-flash')}:generateContent?key=${encodeURIComponent(key)}`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ contents: [{ role: 'user', parts: [{ text: 'hi' }] }], generationConfig: { maxOutputTokens: 1 } }),
        })
      : await fetch(`${baseUrl}/models`, { headers });
    if (res.status === 401 || res.status === 403) {
      spin.fail();
      return `${baseUrl} rejected that key (HTTP ${res.status}).`;
    }
    if (!res.ok) {
      // Not proof of a bad key, so it must not block the install — but it is not
      // a tick either.
      spin.fail(`could not verify (HTTP ${res.status}) — continuing anyway`);
      return null;
    }
    const body = await res.json().catch(() => null) as { data?: Array<{ id: string }> } | null;
    const ids = (body?.data || []).map((m) => m.id);
    if (model && ids.length && !ids.includes(model)) {
      spin.done(`key works, though "${model}" is not in this endpoint's list`);
      return null;
    }
    spin.done('ok');
    return null;
  } catch (err) {
    // A network failure is not a bad key, so it must not block the install.
    spin.fail(`could not reach it (${(err as Error).message}) — continuing anyway`);
    return null;
  }
}

// ── Wizard ──────────────────────────────────────────────────────────────────

const hostOptions: Option[] = [
  { id: 'docker', label: 'Docker on this machine', note: 'one command, no cluster' },
  { id: 'k8s', label: 'Kubernetes', disabled: 'not in this build yet' },
  { id: 'ecs', label: 'Amazon ECS', disabled: 'not in this build yet' },
  { id: 'fly', label: 'Fly.io', disabled: 'coming soon' },
  { id: 'exe', label: 'exe.dev', disabled: 'coming soon' },
];

const dbOptions = (host: string): Option[] => [
  { id: 'sqlite', label: 'SQLite', note: 'zero config, one replica, API keys only' },
  { id: 'supabase', label: 'Supabase', note: 'adds email sign-in and the dashboard' },
  { id: 'postgres', label: 'Postgres', note: 'many replicas; needs psql for migrations' },
];

const fleetOptions: Option[] = [
  { id: 'docker-workers', label: 'Docker browser workers here', note: 'always-on containers, no daemon access needed' },
  { id: 'oya-selfhosted', label: 'Governed Docker, one container per session', note: 'policies and budgets; needs Docker socket access' },
  { id: 'oya-cloud', label: 'Oya Cloud', note: 'managed sandboxes, nothing to run yourself' },
  { id: 'browserbase', label: 'Browserbase' },
  { id: 'steel', label: 'Steel' },
  { id: 'anchor', label: 'Anchor' },
  { id: 'browseruse', label: 'Browser Use Cloud' },
  { id: 'cdp', label: 'Your own Chrome over CDP' },
  { id: 'k8s', label: 'Kubernetes fleet, one pod per session', note: 'needs kubectl and a cluster' },
  { id: 'ecs', label: 'ECS fleet, one task per session', disabled: 'not in this build yet' },
];

const VENDOR_KEYS: Record<string, string[]> = {
  browserbase: ['BROWSERBASE_API_KEY', 'BROWSERBASE_PROJECT_ID'],
  steel: ['STEEL_API_KEY'],
  anchor: ['ANCHOR_API_KEY'],
  browseruse: ['BROWSERUSE_API_KEY'],
};

async function interview(preset: Partial<Answers>, replay: boolean): Promise<{ answers: Answers; secrets: Secrets; migrateUrl: string }> {
  const secrets: Secrets = {};
  let migrateUrl = '';
  /**
   * Replaying a saved plan is the CI path, and CI keeps credentials in the
   * environment. Prompting there would hang a pipeline, so take the value from
   * the environment and only ask when it is genuinely absent and interactive.
   */
  const secretFor = async (field: string, hidden = true): Promise<string> => {
    if (replay) return process.env[field] || '';
    return hidden ? askSecret(`${field}:`) : ask(`${field}:`);
  };

  if (!preset.host) step('Control plane', 'Where the server itself runs.');
  const host = preset.host || await choose('Where should the control plane run?', hostOptions);

  if (!preset.database) step('Database', 'What holds sessions, keys and personas.');
  const database = preset.database || await choose('Which database?', dbOptions(host));
  if (database === 'sqlite') {
    note('SQLite means API-key auth only — email sign-in needs Supabase.');
    note('The dashboard still works: sign in with the API key printed at the end.');
  }
  const isPostgresUrl = (v: string) => {
    if (!v) return;
    try {
      const url = new URL(v);
      if (!['postgres:', 'postgresql:'].includes(url.protocol)) return 'Must start with postgres://';
    } catch { return 'That is not a connection string — postgres://user:pass@host:5432/db'; }
  };

  if (database === 'supabase') {
    secrets.SUPABASE_URL = replay ? (process.env.SUPABASE_URL || '') : await ask('Supabase project URL:');
    secrets.SUPABASE_SERVICE_KEY = await secretFor('SUPABASE_SERVICE_KEY');
    if (!replay) {
      note('Migrations need the Postgres connection string, not the API URL.');
      note('Leave it blank to apply them yourself later with `make migrate`.');
      migrateUrl = await askSecret('Postgres connection string (optional):', { validate: isPostgresUrl });
    }
  }
  if (database === 'postgres') {
    secrets.DATABASE_URL = replay
      ? (process.env.DATABASE_URL || '')
      : await askSecret('Postgres connection string:', {
          validate: (v) => (!v ? 'Required for a Postgres deployment.' : isPostgresUrl(v)),
        });
    migrateUrl = secrets.DATABASE_URL;
  }

  if (!preset.fleet) step('Browsers', 'What actually runs Chrome.');
  const fleet = preset.fleet || await choose('Where should browsers run?', fleetOptions);
  let workers = preset.workers ?? 0;
  if (fleet === 'docker-workers' && preset.workers === undefined) {
    workers = Number(await ask('How many browser workers?', '2', {
      validate: (v) => (/^[1-9][0-9]*$/.test(v) ? undefined : 'Enter a whole number of 1 or more.'),
    }));
  }
  for (const field of VENDOR_KEYS[fleet] || []) {
    secrets[field] = await secretFor(field, !field.endsWith('PROJECT_ID'));
  }
  if (fleet === 'oya-cloud') secrets.OYA_CLOUD_API_KEY = await secretFor('OYA_CLOUD_API_KEY');
  if (fleet === 'cdp') secrets.OYA_CDP_WS_URL = await secretFor('OYA_CDP_WS_URL');

  let k8sFleet = preset.k8sFleet;
  if (fleet === 'k8s' && !k8sFleet) {
    step('Kubernetes fleet', 'One pod per session.');
    note('The image must be digest-pinned: a tag can move between verification and scheduling.');
    k8sFleet = {
      namespace: await ask('Namespace for browser pods:', 'oya-browsers'),
      image: await ask('Browser image:', 'ghcr.io/oyadotai/oya-browser:latest'),
      controlUrl: await ask('Control-plane WebSocket URL, as reachable from the cluster:', 'ws://oya-server.oya-browser.svc:3100/ws'),
      proxyUrl: await ask('Egress proxy URL, as reachable from the cluster:', 'http://oya-server.oya-browser.svc:3128'),
    };
  }

  const llm = preset.llm
    ? { answers: preset.llm, key: replay ? (process.env.OPENAI_API_KEY || '') : '' }
    : await askLlm();
  if (llm.key) secrets.OPENAI_API_KEY = llm.key;

  step('Address', 'Where clients and browsers reach this control plane.');
  const publicUrl = (preset.publicUrl || await ask('Public URL of this control plane:', 'http://localhost:3100', {
    validate: (v) => {
      try {
        const url = new URL(v);
        if (!['http:', 'https:'].includes(url.protocol)) return 'Must be http:// or https://';
      } catch { return 'That is not a URL — try http://localhost:3100'; }
    },
  })).replace(/\/+$/, '');

  const optional = preset.optional || { captcha: '', recordingBucket: '', metrics: false };
  if (!preset.optional) {
    step('Optional services', 'Everything here has a sensible default.');
    const more = await choose('Configure optional services now?', [
      { id: 'no', label: 'No', note: 'change any of it later' },
      { id: 'yes', label: 'Yes', note: 'CAPTCHA solver, recording storage, metrics' },
    ]);
    if (more === 'yes') {
      optional.captcha = await choose('Solve CAPTCHAs automatically?', [
        { id: '', label: 'No', note: 'hosted vendors still solve natively' },
        { id: 'capsolver', label: 'CapSolver' },
        { id: '2captcha', label: '2Captcha' },
      ]);
      if (optional.captcha) secrets.OYA_CAPTCHA_API_KEY = await secretFor('OYA_CAPTCHA_API_KEY');
      if (database === 'supabase') optional.recordingBucket = await ask('Private Storage bucket for recordings (blank to keep them on disk):', '');
      optional.metrics = (await choose('Expose /metrics to a Prometheus scraper?', [
        { id: 'no', label: 'No' }, { id: 'yes', label: 'Yes', note: 'generates a scrape token' },
      ])) === 'yes';
    }
  }

  return {
    answers: { version: 1, host, database, fleet, workers, k8sFleet, llm: llm.answers, publicUrl, optional },
    secrets,
    migrateUrl,
  };
}

// ── Preflight ───────────────────────────────────────────────────────────────

interface Check { ok: boolean; label: string; detail?: string; fatal?: boolean }

async function preflight(root: string, a: Answers): Promise<Check[]> {
  const checks: Check[] = [];
  const needsDocker = a.host === 'docker' || a.fleet === 'docker-workers' || a.fleet === 'oya-selfhosted';

  if (needsDocker) {
    const version = await capture('docker', ['--version']);
    checks.push({ ok: !!version, label: 'docker', detail: version || 'not found', fatal: true });
    const daemon = await capture('docker', ['info', '--format', '{{.ID}}']);
    checks.push({ ok: !!daemon, label: 'docker daemon reachable', detail: daemon ? 'ok' : 'not running', fatal: true });
    const compose = await capture('docker', ['compose', 'version', '--short']);
    // The compose file uses the `env_file: [{path, required}]` form, added in 2.24.
    // fatal regardless of whether it is missing or merely old: 2.20 cannot parse
  // the `env_file: [{path, required}]` form in docker-compose.yml, and finding
  // that out at `docker compose up` is far from the check that knew about it.
  checks.push({ ok: atLeast(compose, 2, 24), label: 'docker compose ≥ 2.24', detail: compose || 'not found', fatal: true });
  }
  if (a.host !== 'docker') {
    checks.push({ ok: atLeast(process.versions.node, 22, 13), label: 'node ≥ 22.13 (node:sqlite)', detail: process.versions.node, fatal: true });
  }
  if (a.fleet === 'k8s' || a.host === 'k8s') {
    const version = await capture('kubectl', ['version', '--client=true', '-o', 'json']);
    checks.push({ ok: !!version, label: 'kubectl', detail: version ? 'ok' : 'not found', fatal: true });
    const cluster = await capture('kubectl', ['get', 'namespace', 'kube-system', '-o', 'name']);
    checks.push({ ok: !!cluster, label: 'cluster reachable', detail: cluster || 'kubectl cannot reach a cluster', fatal: true });
  }
  if (a.database === 'postgres' || a.database === 'supabase') {
    checks.push({ ok: await has('psql'), label: 'psql (to apply migrations)', detail: 'from libpq/postgresql-client', fatal: true });
  }

  const port = Number(new URL(a.publicUrl).port || 3100);
  const free = await portFree(port);
  checks.push({ ok: free, label: `port ${port} free`, detail: free ? 'ok' : 'something is already listening' });

  const envPath = join(root, '.env');
  if (existsSync(envPath)) {
    const mode = (statSync(envPath).mode & 0o777).toString(8);
    checks.push({ ok: true, label: '.env exists — keeping its OYA_PROFILE_SECRET and API keys', detail: `mode ${mode}` });
  }
  return checks;
}

// ── Env assembly ────────────────────────────────────────────────────────────

function buildEnv(a: Answers, secrets: Secrets, existing: Record<string, string>): { values: Record<string, string>; extra: string[]; apiKey: string } {
  const v: Record<string, string> = {};
  const port = String(Number(new URL(a.publicUrl).port || 3100));

  // Re-using the existing KEK is not an optimisation: a fresh one silently
  // orphans every credential already sealed in the data volume.
  v.OYA_PROFILE_SECRET = existing.OYA_PROFILE_SECRET || token();
  const apiKey = existing.API_KEYS?.split(',')[0] || `oya_${token()}`;
  v.API_KEYS = existing.API_KEYS || apiKey;
  v.OYA_OPERATOR_TOKEN = existing.OYA_OPERATOR_TOKEN || token();
  if (a.host !== 'docker') v.PORT = port;

  Object.assign(v, secrets);

  if (a.llm.provider !== 'skip') {
    v.OPENAI_BASE_URL = a.llm.baseUrl;
    v.CHAT_MODEL = a.llm.model;
  }

  const wsUrl = a.publicUrl.replace(/^http/, 'ws') + '/ws';
  if (a.fleet === 'oya-cloud') {
    v.OYA_BROWSER_PROVIDER = 'oya-cloud';
    v.OYA_PUBLIC_WS_URL = wsUrl;
    // A .env written before the rename still carries the old spelling.
    v.OYA_CLOUD_SNAPSHOT = existing.OYA_CLOUD_SNAPSHOT || existing.DAYTONA_SNAPSHOT || 'oya-browser';
  } else if (a.fleet === 'oya-selfhosted') {
    v.OYA_BROWSER_PROVIDER = 'oya-selfhosted';
    v.OYA_FLEET_RUNTIME = 'docker';
    v.OYA_MANAGED_NETWORK = 'oya-browsers';
    v.OYA_MANAGED_IMAGE = 'oya-browser:local';
    v.OYA_MANAGED_CONTROL_URL = a.host === 'docker' ? 'ws://server:3100/ws' : wsUrl;
    v.OYA_EGRESS_PORT = '3128';
    v.OYA_EGRESS_HOST = '0.0.0.0';
    v.OYA_MANAGED_PROXY_URL = a.host === 'docker' ? 'http://server:3128' : 'http://127.0.0.1:3128';
    v.OYA_MANAGED_REGION = 'local';
  } else if (a.fleet === 'k8s' && a.k8sFleet) {
    v.OYA_BROWSER_PROVIDER = 'oya-selfhosted';
    v.OYA_FLEET_RUNTIME = 'k8s';
    v.OYA_K8S_NAMESPACE = a.k8sFleet.namespace;
    v.OYA_MANAGED_NETWORK = 'oya-governed-egress';
    v.OYA_MANAGED_IMAGE = a.k8sFleet.image;   // replaced with the pinned digest at provision time
    v.OYA_MANAGED_CONTROL_URL = a.k8sFleet.controlUrl;
    v.OYA_MANAGED_PROXY_URL = a.k8sFleet.proxyUrl;
    v.OYA_MANAGED_REGION = a.k8sFleet.namespace;
    v.OYA_EGRESS_PORT = '3128';
    v.OYA_EGRESS_HOST = '0.0.0.0';
  } else if (a.fleet === 'docker-workers') {
    v.OYA_BROWSER_PROVIDER = 'oya-selfhosted';
  } else {
    v.OYA_BROWSER_PROVIDER = a.fleet;
  }

  if (a.optional.captcha) v.OYA_CAPTCHA_PROVIDER = a.optional.captcha;
  if (a.optional.recordingBucket) v.OYA_RECORDING_BUCKET = a.optional.recordingBucket;
  if (a.optional.metrics) v.OYA_METRICS_TOKEN = existing.OYA_METRICS_TOKEN || token();

  // Generated but left commented: any browser holding this is accepted, so it is
  // opt-in rather than something an install quietly switches on.
  const extra = [
    '# ── Shared fleet enrolment ──',
    '# Uncomment to let any browser presenting this token enrol itself.',
    `# FLEET_TOKEN=${token()}`,
  ];
  return { values: v, extra, apiKey };
}

/**
 * Pinned by digest. priorState() pulls this image and runs it with a volume of
 * sealed credentials mounted, so a mutable `alpine:latest` would mean running
 * whatever that tag points at today against the operator's data. This is the
 * multi-arch index digest of alpine:3.22.
 */
const PROBE_IMAGE = 'alpine@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce';

/**
 * Encrypted state left behind by a previous install, keyed to a secret we no
 * longer have. Generating a fresh KEK on top of it does not fail at install
 * time — the server crash-loops later on "unable to authenticate data", which is
 * an awful way to find out. So look before writing.
 */
async function priorState(): Promise<string | null> {
  const volumes = (await capture('docker', ['volume', 'ls', '-q', '--filter', 'name=oya-data'])) || '';
  for (const name of volumes.split('\n').map((v) => v.trim()).filter(Boolean)) {
    const listing = await capture('docker', ['run', '--rm', '-v', `${name}:/d`, PROBE_IMAGE, 'ls', '/d']);
    // If the probe cannot run, treat a volume that exists as suspect rather than
    // assuming it is empty — the failure mode of guessing wrong is a crash loop.
    if (listing === null || /cookies\.json|personas\.json|profiles|\.secret/.test(listing)) return name;
  }
  return null;
}

/** Returns the secret to use, or null to keep generating a fresh one. */
async function resolveKekConflict(volume: string, root: string): Promise<string | null> {
  warn(`${volume} already holds encrypted data from an earlier install.`);
  note('Cookies, proxy credentials and TOTP seeds in it were sealed with that');
  note('install\'s OYA_PROFILE_SECRET. A new secret cannot open them, and the');
  note('server refuses to start rather than silently losing them.');
  const choice = await choose('How should that be handled?', [
    { id: 'paste', label: 'I have the previous OYA_PROFILE_SECRET', note: 'reuse it and keep the data' },
    { id: 'reset', label: 'Delete the old data and start clean', note: 'personas, cookies and sessions are lost' },
    { id: 'abort', label: 'Stop so I can go and find it' },
  ]);
  if (choice === 'paste') {
    return askSecret('Previous OYA_PROFILE_SECRET:', {
      validate: (v) => (v ? undefined : 'Paste the secret, or pick another option.'),
    });
  }
  if (choice === 'abort') throw new InputError('Stopped without changing anything.');
  if (!(await confirm(`Delete ${volume} and everything stored in it?`, false))) {
    throw new InputError('Stopped without changing anything.');
  }
  // cwd: root, not process.cwd() — findRepoRoot walks up, so running the
  // wizard from a subdirectory leaves `docker compose` with no compose file and
  // nothing stopped. `docker volume rm -f` does not force a volume that is
  // still attached, so that failure must not be swallowed either: the whole
  // point of this branch is that the next step mints a fresh KEK, and doing
  // that over surviving sealed data is the crash loop this function exists to
  // prevent.
  await run('docker', ['compose', 'down', '-v'], { cwd: root, quiet: true }).catch(() => {});
  await run('docker', ['volume', 'rm', '-f', volume], { cwd: root, quiet: true });
  success(`removed ${volume}`);
  return null;
}

// ── Targets ─────────────────────────────────────────────────────────────────

async function waitReady(url: string, timeoutMs = 180_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  process.stdout.write('  waiting for /readyz… ');
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${url}/readyz`);
      if (res.ok) { console.log('ready'); return; }
    } catch { /* not up yet */ }
    await new Promise((r) => setTimeout(r, 2000));
  }
  console.log('timed out');
  throw new Error(`${url}/readyz did not become ready. Check logs with: docker compose logs -f server`);
}

/**
 * The runtime refuses a mutable tag, so resolve one to a digest here rather than
 * making the operator go and find it. `docker manifest inspect` reads the registry
 * without pulling; if Docker is not around, say exactly what to run.
 */
async function pinImage(reference: string): Promise<string> {
  if (/@sha256:[0-9a-f]{64}$/.test(reference)) return reference;
  process.stdout.write(`  resolving ${reference} to a digest… `);
  const raw = await capture('docker', ['manifest', 'inspect', '--verbose', reference]);
  const digest = raw && /"digest":\s*"(sha256:[0-9a-f]{64})"/.exec(raw)?.[1];
  if (!digest) {
    console.log('could not');
    throw new Error(
      `Kubernetes browsers must run a digest-pinned image, and ${reference} is a tag.\n`
      + `  Get the digest with:  docker manifest inspect --verbose ${reference}\n`
      + `  then re-run with the image written as ${reference.split(':')[0]}@sha256:<digest>`,
    );
  }
  const pinned = `${reference.split('@')[0].replace(/:[^:/]+$/, '')}@${digest}`;
  console.log(digest.slice(0, 19) + '…');
  return pinned;
}

/**
 * A governed pod's only isolation is this NetworkPolicy — the Kubernetes stand-in
 * for Docker's internal bridge — so the runtime verifies it exists, selects these
 * pods and restricts Egress. It is written to disk before being applied: it is the
 * operator's security boundary and they should be able to read and tighten it.
 */
function networkPolicy(namespace: string, name: string): string {
  return JSON.stringify({
    apiVersion: 'networking.k8s.io/v1',
    kind: 'NetworkPolicy',
    metadata: { name, namespace },
    spec: {
      podSelector: { matchLabels: { app: 'oya-managed-browser' } },
      policyTypes: ['Egress'],
      egress: [
        // DNS only, to the cluster resolver.
        { ports: [{ protocol: 'UDP', port: 53 }, { protocol: 'TCP', port: 53 }] },
        // Everything else must go through the control plane's egress proxy, which
        // is what makes per-session egress policy and budgets enforceable.
        // `app: server` is the label k8s/base/server.yaml gives the control plane;
        // the empty namespaceSelector lets the browsers live in their own namespace.
        // A control plane OUTSIDE the cluster is not matched by any pod selector —
        // add an ipBlock rule here for that case, or the pods will resolve DNS and
        // then fail to enrol.
        { to: [{ namespaceSelector: {}, podSelector: { matchLabels: { app: 'server' } } }] },
      ],
    },
  }, null, 2);
}

async function provisionK8sFleet(root: string, a: Answers): Promise<string> {
  const fleet = a.k8sFleet!;
  const policyName = 'oya-governed-egress';
  const pinned = await pinImage(fleet.image);

  // apply, not create: re-running an install must not fail on an existing namespace.
  const nsDoc = JSON.stringify({ apiVersion: 'v1', kind: 'Namespace', metadata: { name: fleet.namespace } });
  await pipeTo('kubectl', ['apply', '-f', '-'], nsDoc, root);

  const { writeFileSync, mkdirSync } = await import('node:fs');
  mkdirSync(join(root, 'k8s', 'fleet'), { recursive: true });
  const policyPath = join(root, 'k8s', 'fleet', 'networkpolicy.json');
  writeFileSync(policyPath, networkPolicy(fleet.namespace, policyName) + '\n');
  await pipeTo('kubectl', ['apply', '-f', '-'], networkPolicy(fleet.namespace, policyName), root);
  console.log(`  applied namespace ${fleet.namespace} and NetworkPolicy ${policyName}`);
  console.log(`  policy written to ${policyPath} — review it; it is your egress boundary`);
  return pinned;
}

/** kubectl reads manifests on stdin, so nothing sensitive lands in argv. */
function pipeTo(cmd: string, args: string[], stdin: string, cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: ['pipe', 'inherit', 'inherit'] });
    child.on('error', reject);
    child.on('close', (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} ${args.join(' ')} exited ${code}`))));
    child.stdin.end(stdin);
  });
}

async function provisionDocker(root: string, a: Answers): Promise<void> {
  if (a.fleet === 'oya-selfhosted') {
    // verifyRuntime() refuses anything but an internal bridge, so create it that way.
    const exists = await capture('docker', ['network', 'inspect', 'oya-browsers']);
    if (!exists) await run('docker', ['network', 'create', '--internal', '--driver', 'bridge', 'oya-browsers'], { cwd: root });
    console.log('  building the governed browser image…');
    await run('docker', ['build', '-t', 'oya-browser:local', 'browser'], { cwd: root });
    console.log('\n  Governed browsers need Docker daemon access from the server container.');
    console.log('  Add this to the server service in docker-compose.yml, then re-run `docker compose up -d`:');
    console.log('    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock');
  }
  const scale = a.fleet === 'docker-workers' ? ['--scale', `browser=${a.workers}`] : ['--scale', 'browser=0'];
  await run('docker', ['compose', 'up', '-d', '--build', ...scale], { cwd: root });
}

// ── Command ─────────────────────────────────────────────────────────────────

export async function cmdInstall(flags: Record<string, string | boolean>): Promise<void> {
  const dryRun = flags['dry-run'] === true;
  const root = await locateRepo();
  const configPath = typeof flags.config === 'string' ? flags.config : null;
  const preset: Partial<Answers> = configPath ? JSON.parse(readFileSync(configPath, 'utf8')) : {};

  banner('Oya Browser — self-host install', root);
  if (configPath) note(`replaying ${configPath}`);
  // A replay asks nothing, so numbering the handful of surviving prompts would
  // count to six and never get there.
  steps(configPath ? 0 : 6);

  const { answers, secrets, migrateUrl } = await interview(preset, !!configPath);

  const checks = await preflight(root, answers);
  console.log(`\n${style.bold('  Preflight')}`);
  for (const c of checks) {
    const mark = c.ok ? icon.ok : c.fatal ? icon.fail : icon.warn;
    console.log(`  ${mark} ${c.label}${c.detail ? style.grey(`  — ${c.detail}`) : ''}`);
  }
  const blocking = checks.filter((c) => !c.ok && c.fatal);
  // A preview reports what is missing; only a real install refuses to proceed.
  if (blocking.length && !dryRun) throw new Error(`Install cannot continue: ${blocking.map((c) => c.label).join(', ')}`);

  if (answers.host === 'docker' && /(localhost|127\.0\.0\.1)/.test(secrets.DATABASE_URL || '')) {
    warn('DATABASE_URL points at localhost, which inside a container means the container itself.');
    note('Use host.docker.internal (macOS, Windows) or the host IP so the server can reach it.');
  }

  const envPath = join(root, '.env');
  const existing = readEnv(envPath);

  // Only a fresh KEK can orphan existing data; reusing .env's is always safe.
  if (!existing.OYA_PROFILE_SECRET && answers.host === 'docker' && !dryRun) {
    const volume = await priorState();
    if (volume) {
      const reused = await resolveKekConflict(volume, root);
      if (reused) existing.OYA_PROFILE_SECRET = reused;
    }
  }

  const { values, extra, apiKey } = buildEnv(answers, secrets, existing);
  const body = renderEnv(values, extra);

  console.log(`\n${style.bold('  Plan')}`);
  const row = (k: string, v: string) => console.log(`  ${style.grey(k.padEnd(12))}${v}`);
  row('control', `${answers.host} · ${answers.database}`);
  row('browsers', `${answers.fleet}${answers.workers ? ` ×${answers.workers}` : ''}`);
  row('llm', answers.llm.provider === 'skip' ? style.grey('none') : `${answers.llm.model} ${style.grey(`@ ${answers.llm.baseUrl}`)}`);
  row('writes', `${envPath} ${style.grey('(0600)')}`);
  row('then', `docker compose up -d, wait for ${answers.publicUrl}/readyz`);

  if (dryRun) {
    console.log('\n--dry-run: nothing was written. Settings that would be applied:');
    for (const k of Object.keys(values)) {
      console.log(`  ${k}=${SECRET_KEYS.has(k) ? style.grey('<hidden>') : values[k]}`);
    }
    return;
  }

  writeEnv(envPath, body);
  writeFileSync(join(root, ANSWERS_FILE), JSON.stringify(answers, null, 2) + '\n');
  success(`wrote ${envPath}`);
  if (!existing.OYA_PROFILE_SECRET) {
    warn('OYA_PROFILE_SECRET was generated — back it up.');
    note('Without it, stored cookies, proxy credentials and TOTP seeds cannot be decrypted.');
  }

  if (migrateUrl) {
    // The schema has to exist before the server opens a connection to it.
    const spin = spinner('applying migrations');
    try {
      await new Promise<void>((resolve, reject) => {
        const child = spawn(process.execPath, [join(root, 'server', 'migrations', 'run.mjs')], {
          cwd: root, stdio: ['ignore', 'pipe', 'pipe'],
          env: { ...process.env, DATABASE_URL: migrateUrl },
        });
        let err = '';
        child.stderr.on('data', (d) => { err += d; });
        child.on('error', reject);
        child.on('close', (code) => (code === 0 ? resolve() : reject(new Error(err.trim() || `migrations exited ${code}`))));
      });
      spin.done();
    } catch (e) {
      spin.fail();
      throw new Error(`Migrations failed: ${(e as Error).message}\n  Fix the database, then re-run: DATABASE_URL=… make migrate`);
    }
  }

  if (answers.fleet === 'k8s') {
    const pinned = await provisionK8sFleet(root, answers);
    if (pinned !== values.OYA_MANAGED_IMAGE) {
      // The runtime rejects a tag, so the .env must carry the resolved digest.
      values.OYA_MANAGED_IMAGE = pinned;
      writeEnv(envPath, renderEnv(values, extra));
      console.log('  .env updated with the pinned image digest');
    }
  }
  if (answers.host === 'docker') await provisionDocker(root, answers);
  await waitReady(answers.publicUrl);

  banner(`${answers.publicUrl} is up`, 'Sign in to the dashboard with the key below.');
  console.log(`\n  ${style.bold('API key')}  ${style.green(apiKey)}\n`);
  console.log(`  ${style.grey('# your shell may export OYA_API_KEY for the hosted service')}`);
  console.log(`  unset OYA_API_KEY`);
  console.log(`  oya login --url ${answers.publicUrl} --key ${apiKey}`);
  console.log(`  oya ls && oya goto https://example.com\n`);
}
