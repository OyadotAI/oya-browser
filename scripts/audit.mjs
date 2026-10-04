/**
 * The dependency audit CI runs on each part: `npm audit` on what ships (no dev
 * dependencies), failing on any high or critical advisory except the few
 * listed in ALLOWED, each with the reason it cannot be fixed yet. npm audit
 * has no allowlist of its own, and a new tool for one list is not worth it.
 *
 *   node scripts/audit.mjs [dir]   audits the package in dir (default: here)
 */
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

/** Advisories that may stay, by GHSA id, with why. Remove an entry once a fix ships. */
export const ALLOWED = {
  // braces stack exhaustion on deeply nested patterns, pulled in by the server's
  // dependencies: no fixed release to move to yet.
  'GHSA-vfj7-8cjw-p6xm': 'braces: no fixed version published',
};

/** Severities that fail the build. */
const BLOCKING = new Set(['high', 'critical']);

/** Room for npm audit's JSON on a large tree. */
const MAX_OUTPUT_BYTES = 64 * 1024 * 1024;

/** The GHSA id at the end of an advisory URL. */
const ghsaOf = (url = '') => url.split('/').pop();

/** Every advisory in an `npm audit --json` report, once each. */
export function advisories(report) {
  const found = new Map();
  for (const entry of Object.values(report.vulnerabilities ?? {})) {
    for (const via of entry.via ?? []) if (typeof via === 'object') found.set(via.url, via);
  }
  return [...found.values()];
}

/** The high or critical advisories not allowed: what fails the build. */
export const blocking = (report, allowed = ALLOWED) =>
  advisories(report).filter((a) => BLOCKING.has(a.severity) && !Object.hasOwn(allowed, ghsaOf(a.url)));

/** Runs `npm audit` in `dir` and returns its parsed report; npm exits non-zero whenever it finds anything. */
function audit(dir) {
  const run = spawnSync('npm', ['audit', '--json', '--omit=dev'], { cwd: dir, maxBuffer: MAX_OUTPUT_BYTES });
  const text = run.stdout?.toString() ?? '';
  if (!text.trim()) throw new Error(`npm audit gave no report in ${dir}: ${run.stderr?.toString() ?? ''}`);
  return JSON.parse(text);
}

/** Audits `dir`, prints what blocks, and sets the exit code. */
function main(dir = '.') {
  const found = blocking(audit(dir));
  for (const a of found) console.error(`${a.severity}: ${a.name} ${a.title} ${a.url}`);
  console.log(`${dir}: ${found.length} blocking advisories (${Object.keys(ALLOWED).length} allowed by id)`);
  process.exitCode = found.length ? 1 : 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv[2]);
