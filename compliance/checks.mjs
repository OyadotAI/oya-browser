/**
 * The checks the evidence pack runs. Each returns a verdict and the artifact it
 * produced, so the report can show what was run rather than assert an outcome.
 *
 * Every check must be able to fail. A check that cannot fail proves nothing, so
 * the tamper-evidence check edits a row and expects the database to refuse it.
 */

import { execFile } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { promisify } from 'node:util';
import { REPO_ROOT, SCRATCH_PG } from './constants.mjs';

const run = promisify(execFile);

/** Runs a command, returning its output and whether it succeeded, never throwing. */
async function attempt(cmd, args, opts = {}) {
  try {
    const { stdout, stderr } = await run(cmd, args, { cwd: REPO_ROOT, ...opts });
    return { ok: true, output: (stdout + stderr).trim() };
  } catch (e) {
    return { ok: false, output: `${e.stdout || ''}${e.stderr || e.message}`.trim() };
  }
}

/**
 * Runs one of the repo's own test files as evidence, the way the repo runs it:
 * the hermetic preload keeps a test off `.env` and off live infrastructure, so
 * the evidence cannot come from a developer's credentials by accident.
 */
async function runTests(files, cwd = 'server') {
  const args = ['--import', './tests/support/hermetic.js', '--test', ...files];
  const result = await attempt('node', args, { cwd: `${REPO_ROOT}/${cwd}` });
  return { ...tally(result), command: `node --import ./tests/support/hermetic.js --test ${files.join(' ')}` };
}

/** Node's reporters spell the totals either way; read whichever is present. */
function tally(result) {
  const pass = /(?:#|ℹ) pass (\d+)/.exec(result.output)?.[1];
  const fail = /(?:#|ℹ) fail (\d+)/.exec(result.output)?.[1];
  if (pass === undefined || fail === undefined)
    return { ok: false, detail: `test output could not be read: ${result.output.slice(-200)}` };
  return { ok: result.ok && fail === '0' && Number(pass) > 0, detail: `${pass} passed, ${fail} failed` };
}

/** psql inside the scratch database container. */
async function psql(sql) {
  return attempt('docker', ['exec', SCRATCH_PG, 'psql', '-U', 'postgres', '-tAc', sql]);
}

/** The audit trail records privileged actions, and the rows carry chain columns. */
async function auditRecords() {
  const tests = await runTests(['tests/unit/platform/audit.test.ts']);
  const schema = await readFile(`${REPO_ROOT}/server/migrations/004_control_plane.sql`, 'utf8');
  const columns = ['action', 'actor', 'outcome', 'ip', 'user_agent', 'meta'].every((c) => schema.includes(c));
  return { ...tests, ok: tests.ok && columns, detail: `${tests.detail}; audit_log carries who/what/outcome/source: ${columns}` };
}

/** Hash linking is real: an edited row stops verifying. */
async function tamperEvidence() {
  return runTests(['tests/unit/platform/audit-chain.test.ts']);
}

/** The database itself refuses to change history. Proven on a scratch instance. */
async function appendOnly() {
  const update = await psql("update oya_browser.audit_log set outcome='denied' where seq=1;");
  const del = await psql('delete from oya_browser.audit_log where seq=1;');
  const grants = await psql(
    "select coalesce(string_agg(privilege_type,',' order by privilege_type),'none') from information_schema.role_table_grants where grantee='service_role' and table_name='audit_log';",
  );
  const refused = !update.ok && !del.ok;
  const leastPrivilege = grants.output === 'INSERT,SELECT';
  return {
    ok: refused && leastPrivilege,
    detail: `update refused: ${!update.ok}; delete refused: ${!del.ok}; service_role holds ${grants.output}`,
    command: 'psql: update/delete against audit_log, then read its grants',
  };
}

/** Authentication is more than a password. */
async function mfa() {
  return runTests([
    'tests/unit/modules/challenges/mfa.test.ts',
    'tests/unit/modules/challenges/totp.test.ts',
    'tests/unit/modules/challenges/mfa-factors.test.ts',
  ]);
}

/** The agent can only reach approved hosts, and both halves agree on which. */
async function egressContainment() {
  return runTests(['tests/unit/modules/control/egress.test.ts']);
}

/** Credentials are stored as ciphertext or digests, never as themselves. */
async function secretsAtRest() {
  const tests = await runTests(['tests/unit/platform/secrets.test.ts']);
  const migration = await readFile(`${REPO_ROOT}/server/migrations/010_hash_api_keys.sql`, 'utf8');
  const hashed = migration.includes('key_hash');
  return { ...tests, ok: tests.ok && hashed, detail: `${tests.detail}; api keys stored as sha256: ${hashed}` };
}

/** What redaction actually covers today. Expected to fail until PHI is in scope. */
async function phiRedaction() {
  const redactors = await attempt('grep', ['-rl', 'redact', 'server/src']);
  const phiAware = /phi|hipaa/i.test(redactors.output);
  return {
    ok: phiAware,
    detail: phiAware
      ? 'redaction names PHI explicitly'
      : `redaction exists in ${redactors.output.split('\n').filter(Boolean).length} files but targets secrets and placeholders, not PHI`,
    command: 'grep -rl redact server/src',
  };
}

/** A third party on the ePHI path needs a BAA; a residential proxy vendor will not sign one. */
async function subprocessorEgress() {
  const configured = Boolean(process.env.OYA_RESIDENTIAL_PROXY_URL || process.env.BENCH_PROXY_URL);
  return {
    ok: !configured,
    detail: configured
      ? 'a third-party proxy is configured in this environment: not permissible on an ePHI path without a BAA'
      : 'no third-party proxy configured: egress is direct or customer-owned',
    command: 'inspect OYA_RESIDENTIAL_PROXY_URL / BENCH_PROXY_URL',
  };
}

/** Retention needs a floor for audit records and a purge for everything else. */
async function retentionPolicy() {
  const floor = await attempt('grep', ['-rEn', 'AUDIT_RETENTION|RETENTION_YEARS', 'server/src']);
  const found = floor.ok && floor.output.length > 0;
  return {
    ok: found,
    detail: found ? floor.output.split('\n')[0] : 'no audit retention floor and no purge job found in server/src',
    command: 'grep -rEn "AUDIT_RETENTION|RETENTION_YEARS" server/src',
  };
}

/** Check id → the function that runs it. */
export const CHECKS = {
  'audit.records': auditRecords,
  'audit.tamper-evidence': tamperEvidence,
  'audit.append-only': appendOnly,
  'auth.mfa': mfa,
  'egress.containment': egressContainment,
  'secrets.at-rest': secretsAtRest,
  'phi.redaction': phiRedaction,
  'subprocessor.egress': subprocessorEgress,
  'retention.policy': retentionPolicy,
};
