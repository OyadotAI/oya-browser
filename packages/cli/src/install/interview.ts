/**
 * The install questions, one step per decision: host, database, fleet, the
 * agent LLM, the public address and optional services. A preset (from
 * --config) answers a question in advance; replaying one takes secrets from
 * the environment instead of asking.
 */
import { ask, askSecret, choose, note, step, type Option } from '../prompt.ts';
import { askLlm } from './llm.ts';
import type { Answers, Interview, Secrets } from './types.ts';

/** Where the control plane may run. */
const HOST_OPTIONS: Option[] = [
  { id: 'docker', label: 'Docker on this machine', note: 'one command, no cluster' },
  { id: 'k8s', label: 'Kubernetes', disabled: 'not in this build yet' },
  { id: 'ecs', label: 'Amazon ECS', disabled: 'not in this build yet' },
  { id: 'fly', label: 'Fly.io', disabled: 'coming soon' },
  { id: 'exe', label: 'exe.dev', disabled: 'coming soon' },
];

/** The databases on offer. */
const DB_OPTIONS: Option[] = [
  { id: 'sqlite', label: 'SQLite', note: 'zero config, one replica, API keys only' },
  { id: 'supabase', label: 'Supabase', note: 'adds email sign-in and the dashboard' },
  { id: 'postgres', label: 'Postgres', note: 'many replicas; needs psql for migrations' },
];

/** Where browsers may run. */
const FLEET_OPTIONS: Option[] = [
  { id: 'docker-workers', label: 'Docker browser workers here', note: 'always-on containers, no daemon access needed' },
  {
    id: 'oya-selfhosted',
    label: 'Governed Docker, one container per session',
    note: 'policies and budgets; needs Docker socket access',
  },
  { id: 'oya-cloud', label: 'Oya Cloud', note: 'managed sandboxes, nothing to run yourself' },
  { id: 'browserbase', label: 'Browserbase' },
  { id: 'steel', label: 'Steel' },
  { id: 'anchor', label: 'Anchor' },
  { id: 'browseruse', label: 'Browser Use Cloud' },
  { id: 'cdp', label: 'Your own Chrome over CDP' },
  { id: 'k8s', label: 'Kubernetes fleet, one pod per session', note: 'needs kubectl and a cluster' },
  { id: 'ecs', label: 'ECS fleet, one task per session', disabled: 'not in this build yet' },
];

/** The credentials each hosted vendor needs. */
const VENDOR_KEYS: Record<string, string[]> = {
  browserbase: ['BROWSERBASE_API_KEY', 'BROWSERBASE_PROJECT_ID'],
  steel: ['STEEL_API_KEY'],
  anchor: ['ANCHOR_API_KEY'],
  browseruse: ['BROWSERUSE_API_KEY'],
};

/** Each menu step's heading and hint. */
const HEADINGS: Record<string, [string, string]> = {
  host: ['Control plane', 'Where the server itself runs.'],
  database: ['Database', 'What holds sessions, keys and personas.'],
  fleet: ['Browsers', 'What actually runs Chrome.'],
};

/** The optional-services menu. */
const MORE_OPTIONS: Option[] = [
  { id: 'no', label: 'No', note: 'change any of it later' },
  { id: 'yes', label: 'Yes', note: 'CAPTCHA solver, recording storage, metrics' },
];

/** The CAPTCHA solver menu. */
const CAPTCHA_OPTIONS: Option[] = [
  { id: '', label: 'No', note: 'hosted vendors still solve natively' },
  { id: 'capsolver', label: 'CapSolver' },
  { id: '2captcha', label: '2Captcha' },
];

/** The metrics menu. */
const METRICS_OPTIONS: Option[] = [
  { id: 'no', label: 'No' },
  { id: 'yes', label: 'Yes', note: 'generates a scrape token' },
];

/** The Kubernetes questions: field, prompt and default. */
const K8S_QUESTIONS: Array<[keyof NonNullable<Answers['k8sFleet']>, string, string]> = [
  ['namespace', 'Namespace for browser pods:', 'oya-browsers'],
  ['image', 'Browser image:', 'ghcr.io/oyadotai/oya-browser:latest'],
  [
    'controlUrl',
    'Control-plane WebSocket URL, as reachable from the cluster:',
    'ws://oya-server.oya-browser.svc:3100/ws',
  ],
  ['proxyUrl', 'Egress proxy URL, as reachable from the cluster:', 'http://oya-server.oya-browser.svc:3128'],
];

/** Credentials each fleet needs besides its vendor keys. */
const FLEET_SECRETS: Record<string, string> = { 'oya-cloud': 'OYA_CLOUD_API_KEY', cdp: 'OYA_CDP_WS_URL' };

/** The interview's working state: the preset, whether this is a replay, and what has been gathered. */
interface Session {
  /** Answers given in advance. */
  preset: Partial<Answers>;
  /** Replaying a saved plan: never prompt for a secret. */
  replay: boolean;
  /** Credentials gathered so far. */
  secrets: Secrets;
  /** Where migrations run, if anywhere. */
  migrateUrl: string;
}

/**
 * Replaying a saved plan is the CI path, and CI keeps credentials in the
 * environment. Prompting there would hang a pipeline, so take the value from
 * the environment and only ask when it is genuinely absent and interactive.
 */
function secretFor(s: Session, field: string, hidden = true): Promise<string> {
  if (s.replay) return Promise.resolve(process.env[field] || '');
  return hidden ? askSecret(`${field}:`) : ask(`${field}:`);
}

/** A problem with a Postgres connection string, or nothing when it is fine (or blank). */
function isPostgresUrl(v: string): string | undefined {
  if (!v) return;
  try {
    const url = new URL(v);
    if (!['postgres:', 'postgresql:'].includes(url.protocol)) return 'Must start with postgres://';
  } catch {
    return 'That is not a connection string — postgres://user:pass@host:5432/db';
  }
}

/** A preset answer, or the choice from a menu under a step heading. */
async function pick(preset: string | undefined, heading: [string, string], question: string, options: Option[]) {
  if (preset) return preset;
  step(...heading);
  return choose(question, options);
}

/** Supabase needs its URL and service key, and optionally a connection string for migrations. */
async function askSupabase(s: Session): Promise<void> {
  s.secrets.SUPABASE_URL = s.replay ? process.env.SUPABASE_URL || '' : await ask('Supabase project URL:');
  s.secrets.SUPABASE_SERVICE_KEY = await secretFor(s, 'SUPABASE_SERVICE_KEY');
  if (s.replay) return;
  note('Migrations need the Postgres connection string, not the API URL.');
  note('Leave it blank to apply them yourself later with `make migrate`.');
  s.migrateUrl = await askSecret('Postgres connection string (optional):', { validate: isPostgresUrl });
}

/** Postgres needs its connection string, which migrations also use. */
async function askPostgres(s: Session): Promise<void> {
  s.secrets.DATABASE_URL = s.replay
    ? process.env.DATABASE_URL || ''
    : await askSecret('Postgres connection string:', {
        validate: (v) => (!v ? 'Required for a Postgres deployment.' : isPostgresUrl(v)),
      });
  s.migrateUrl = s.secrets.DATABASE_URL;
}

/** What SQLite gives up. */
function sqliteNotes(): void {
  note('SQLite means API-key auth only — email sign-in needs Supabase.');
  note('The dashboard still works: sign in with the API key printed at the end.');
}

/** The database, and what it needs. */
async function askDatabase(s: Session): Promise<string> {
  const database = await pick(s.preset.database, HEADINGS.database, 'Which database?', DB_OPTIONS);
  if (database === 'sqlite') sqliteNotes();
  if (database === 'supabase') await askSupabase(s);
  if (database === 'postgres') await askPostgres(s);
  return database;
}

/** How many docker workers, unless the preset says. */
async function askWorkers(s: Session, fleet: string): Promise<number> {
  if (fleet !== 'docker-workers' || s.preset.workers !== undefined) return s.preset.workers ?? 0;
  const answer = await ask('How many browser workers?', '2', {
    validate: (v) => (/^[1-9][0-9]*$/.test(v) ? undefined : 'Enter a whole number of 1 or more.'),
  });
  return Number(answer);
}

/** The fleet's own credentials. */
async function askFleetSecrets(s: Session, fleet: string): Promise<void> {
  for (const field of VENDOR_KEYS[fleet] || []) {
    s.secrets[field] = await secretFor(s, field, !field.endsWith('PROJECT_ID'));
  }
  if (Object.hasOwn(FLEET_SECRETS, fleet)) {
    s.secrets[FLEET_SECRETS[fleet]] = await secretFor(s, FLEET_SECRETS[fleet]);
  }
}

/** The Kubernetes fleet's settings, for the k8s fleet. Asked in order, one question each. */
async function askK8s(s: Session, fleet: string): Promise<Answers['k8sFleet']> {
  if (fleet !== 'k8s' || s.preset.k8sFleet) return s.preset.k8sFleet;
  step('Kubernetes fleet', 'One pod per session.');
  note('The image must be digest-pinned: a tag can move between verification and scheduling.');
  const answers: Record<string, string> = {};
  for (const [field, question, fallback] of K8S_QUESTIONS) answers[field] = await ask(question, fallback);
  return answers as NonNullable<Answers['k8sFleet']>;
}

/** The LLM: the preset's (with its key from the environment on a replay), or asked. */
async function llmFor(s: Session): Promise<Answers['llm']> {
  const llm = s.preset.llm
    ? { answers: s.preset.llm, key: s.replay ? process.env.OPENAI_API_KEY || '' : '' }
    : await askLlm();
  if (llm.key) s.secrets.OPENAI_API_KEY = llm.key;
  return llm.answers;
}

/** A problem with a public URL, or nothing when it is fine. */
function checkPublicUrl(v: string): string | undefined {
  try {
    const url = new URL(v);
    if (!['http:', 'https:'].includes(url.protocol)) return 'Must be http:// or https://';
  } catch {
    return 'That is not a URL — try http://localhost:3100';
  }
}

/** Where clients and browsers reach this control plane. */
async function askPublicUrl(s: Session): Promise<string> {
  step('Address', 'Where clients and browsers reach this control plane.');
  const url =
    s.preset.publicUrl ||
    (await ask('Public URL of this control plane:', 'http://localhost:3100', { validate: checkPublicUrl }));
  return url.replace(/\/+$/, '');
}

/** Optional services, only if the user wants to configure them now. */
async function askOptional(s: Session, database: string): Promise<Answers['optional']> {
  const optional = s.preset.optional || { captcha: '', recordingBucket: '', metrics: false };
  if (s.preset.optional) return optional;
  step('Optional services', 'Everything here has a sensible default.');
  const more = await choose('Configure optional services now?', MORE_OPTIONS);
  if (more === 'yes') await askServices(s, database, optional);
  return optional;
}

/** CAPTCHA solving, recording storage and metrics. */
async function askServices(s: Session, database: string, optional: Answers['optional']): Promise<void> {
  optional.captcha = await choose('Solve CAPTCHAs automatically?', CAPTCHA_OPTIONS);
  if (optional.captcha) s.secrets.OYA_CAPTCHA_API_KEY = await secretFor(s, 'OYA_CAPTCHA_API_KEY');
  if (database === 'supabase') {
    optional.recordingBucket = await ask('Private Storage bucket for recordings (blank to keep them on disk):', '');
  }
  optional.metrics = (await choose('Expose /metrics to a Prometheus scraper?', METRICS_OPTIONS)) === 'yes';
}

/** Where browsers run, how many, and what the fleet needs. */
async function askFleet(s: Session) {
  const fleet = await pick(s.preset.fleet, HEADINGS.fleet, 'Where should browsers run?', FLEET_OPTIONS);
  const workers = await askWorkers(s, fleet);
  await askFleetSecrets(s, fleet);
  return { fleet, workers, k8sFleet: await askK8s(s, fleet) };
}

/** Asks everything the preset does not already answer. */
export async function interview(preset: Partial<Answers>, replay: boolean): Promise<Interview> {
  const s: Session = { preset, replay, secrets: {}, migrateUrl: '' };
  const host = await pick(preset.host, HEADINGS.host, 'Where should the control plane run?', HOST_OPTIONS);
  const database = await askDatabase(s);
  const { fleet, workers, k8sFleet } = await askFleet(s);
  const [llm, publicUrl] = [await llmFor(s), await askPublicUrl(s)];
  const optional = await askOptional(s, database);
  const answers: Answers = { version: 1, host, database, fleet, workers, k8sFleet, llm, publicUrl, optional };
  return { answers, secrets: s.secrets, migrateUrl: s.migrateUrl };
}
