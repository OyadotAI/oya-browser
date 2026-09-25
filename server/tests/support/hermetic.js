/**
 * Loaded before every test (`node --import`, inherited by forked children), so
 * no test reaches live services by accident: server/.env is never read, and
 * each test file gets its own scratch data directory unless it chooses one.
 * Set OYA_TEST_LIVE=1 for the suites that exist to talk to a real service.
 */
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';

if (!process.env.OYA_TEST_LIVE) process.env.DOTENV_CONFIG_PATH = join(tmpdir(), 'oya-test-no-env');
// Outbound analytics and ops messages are off in tests whatever the shell holds: a test must never
// post to a real PostHog or Slack. The suites that exercise sending set the variables themselves.
if (!process.env.OYA_TEST_LIVE)
  for (const name of ['POSTHOG_KEY', 'POSTHOG_HOST', 'SLACK_OPS_WEBHOOK_SIGNUPS', 'SLACK_OPS_WEBHOOK_EVENTS'])
    delete process.env[name];
// No test fetches OpenRouter's live model list: the catalog serves its built-in one.
if (!process.env.OYA_TEST_LIVE) process.env.OYA_OPENROUTER_MODELS_URL = '';
// Storage is the scratch directory's own SQLite file (the default driver), never a shell's database
// or Supabase project. Suites that exercise another driver set these themselves.
if (!process.env.OYA_TEST_LIVE)
  for (const name of ['OYA_STORAGE', 'DATABASE_URL', 'SUPABASE_URL', 'SUPABASE_SERVICE_KEY']) delete process.env[name];

/**
 * The runner's own process and every test file process get a fresh directory;
 * a process a test spawns (a server under test) keeps the one it was handed.
 * Sharing one directory across parallel test files made them trip over each
 * other's audit logs, stores and locks.
 */
const isTestFile = /\.test\.[cm]?[jt]s$/.test(basename(process.argv[1] || ''));
const inherited = process.env.OYA_DATA_DIR && process.env.OYA_DATA_DIR === process.env.OYA_TEST_SCRATCH;
if (!process.env.OYA_DATA_DIR || (inherited && isTestFile)) {
  process.env.OYA_DATA_DIR = mkdtempSync(join(tmpdir(), 'oya-test-'));
  process.env.OYA_TEST_SCRATCH = process.env.OYA_DATA_DIR;
}
