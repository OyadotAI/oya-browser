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
