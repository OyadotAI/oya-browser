/**
 * Where the evidence pack looks and what it writes.
 */

import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The repository this pack reports on. */
export const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/** The scratch Postgres the append-only check proves itself against. */
export const SCRATCH_PG = process.env.COMPLIANCE_PG_CONTAINER || 'pg-audit';

/** Where the report is written. */
export const REPORT_PATH = resolve(REPO_ROOT, 'compliance', 'EVIDENCE.md');

/** Where the machine-readable result is written, for CI to gate on. */
export const RESULT_PATH = resolve(REPO_ROOT, 'compliance', 'evidence.json');

/** HIPAA's documentation retention, §164.316(b)(2): six years, in days. */
export const SIX_YEARS_DAYS = 2190;

/** The server file that sets the audit retention floor. */
export const RETENTION_SOURCE = 'server/src/modules/control/service/constants.ts';

/** Each hosted subprocessor on the ePHI path and the date its BAA was signed, null until it is. */
export const BAAS_PATH = 'compliance/baas.json';
