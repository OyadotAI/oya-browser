/**
 * The browser half of the fleet view: counts of a key's browsers by client,
 * provider, health and persona, plus their command totals.
 */

/** Adds one to a tally. */
function bump(tally, name) {
  tally[name] = (tally[name] || 0) + 1;
}

/** Counts one browser row into the running summary. */
function count(summary, b) {
  bump(summary.byClient, b.clientType || 'oya');
  if (b.provider) bump(summary.byProvider, b.provider);
  bump(summary.byHealth, b.health);
  bump(summary.byPersona, b.personaName || b.persona || '—');
  summary.commands += b.commands;
  summary.errors += b.errors;
  summary.pending += b.pending;
}

/** Summarises a key's browser rows for GET /fleet. */
export function summarize(rows) {
  const tallies = { byClient: {}, byProvider: {}, byHealth: { ok: 0, stale: 0, errors: 0, dead: 0 }, byPersona: {} };
  const summary = { ...tallies, commands: 0, errors: 0, pending: 0 };
  for (const row of rows) count(summary, row);
  const { byClient, byProvider, byHealth, byPersona, commands, errors, pending } = summary;
  return { total: rows.length, byClient, byProvider, byHealth, byPersona, commands, errors, pending };
}
