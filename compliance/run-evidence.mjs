/**
 * Generates the HIPAA evidence pack: every control, the code that implements it,
 * the check that was run, and what that check returned.
 *
 * Nothing here asserts a control passes. It runs the check and writes down the
 * answer, gaps included, so the document can be handed to an auditor and
 * regenerated on every release rather than maintained by hand.
 *
 * Usage: node compliance/run-evidence.mjs
 * The append-only check needs the scratch Postgres from compliance/scratch-db.sh.
 */

import { writeFile } from 'node:fs/promises';
import { CHECKS } from './checks.mjs';
import { CONTROLS } from './controls.mjs';
import { REPORT_PATH, RESULT_PATH } from './constants.mjs';

/** Runs every distinct check once, since several controls share one. */
async function runChecks() {
  const ids = [...new Set(CONTROLS.map((c) => c.check))];
  const results = new Map();
  for (const id of ids) {
    const check = CHECKS[Object.hasOwn(CHECKS, id) ? id : ''];
    results.set(id, check ? await check() : { ok: false, detail: `no check implemented for ${id}` });
  }
  return results;
}

/** PASS, GAP for a control that already declares one, else FAIL. */
function verdict(control, result) {
  if (result?.ok) return 'PASS';
  return control.gap ? 'GAP' : 'FAIL';
}

/** One row of the control table. */
function controlRow(control, result) {
  const code = control.implements.map((p) => `\`${p}\``).join('<br>') || '—';
  return `| ${control.id} | ${control.title} | ${verdict(control, result)} | ${code} | ${result?.detail ?? ''} |`;
}

/** The gap list, which is the part worth reading twice. */
function gapSection(rows) {
  const gaps = rows.filter(({ control, result }) => verdict(control, result) !== 'PASS');
  if (!gaps.length) return ['No open gaps.', ''];
  return gaps.flatMap(({ control }) => [
    `### ${control.id} — ${control.title}`,
    '',
    control.gap || 'The check for this control did not pass; see the table.',
    '',
  ]);
}

/** The report, as an auditor reads it: verdicts first, then how each was reached. */
function render(rows, when) {
  const counts = { PASS: 0, GAP: 0, FAIL: 0 };
  rows.forEach(({ control, result }) => (counts[verdict(control, result)] += 1));
  return [
    '# HIPAA Security Rule — evidence pack',
    '',
    `Generated ${when} by \`compliance/run-evidence.mjs\`. Regenerate it per release; do not edit by hand.`,
    '',
    `**${counts.PASS} proven · ${counts.GAP} known gaps · ${counts.FAIL} failing**`,
    '',
    '| § | Control | Verdict | Implementation | What the check found |',
    '|:--|:--|:--|:--|:--|',
    ...rows.map(({ control, result }) => controlRow(control, result)),
    '',
    '## How each verdict was reached',
    '',
    ...rows.flatMap(({ control, result }) =>
      result?.command ? [`- **${control.id}** — \`${result.command}\``] : [],
    ),
    '',
    '## Open gaps',
    '',
    ...gapSection(rows),
    '## Scope',
    '',
    'This pack covers the Oya browser infrastructure: the control plane, the browser and the',
    'audit trail. It does not cover the customer application driving it, the cloud provider',
    'underneath it, or any workflow content. A covered entity remains responsible for its own',
    'risk analysis under §164.308(a)(1).',
    '',
  ].join('\n');
}

/** Writes both artifacts and exits non-zero when a control fails outright. */
async function main() {
  const results = await runChecks();
  const rows = CONTROLS.map((control) => ({ control, result: results.get(control.check) }));
  const when = new Date().toISOString();
  await writeFile(REPORT_PATH, render(rows, when));
  await writeFile(
    RESULT_PATH,
    JSON.stringify({ generated: when, controls: rows.map(({ control, result }) => ({ id: control.id, verdict: verdict(control, result), detail: result?.detail })) }, null, 2),
  );
  const failing = rows.filter(({ control, result }) => verdict(control, result) === 'FAIL');
  console.log(`${REPORT_PATH}: ${rows.length} controls, ${failing.length} failing`);
  process.exitCode = failing.length ? 1 : 0;
}

await main();
