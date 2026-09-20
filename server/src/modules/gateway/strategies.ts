/**
 * Routing strategies: how the pool chooses among available providers. Each
 * is a small picker in a map keyed by strategy name.
 */

/** How a pool may choose among available providers; 'priority' unless OYA_ROUTING_STRATEGY or the key says otherwise. */
export const STRATEGIES = ['priority', 'round-robin', 'least-connections', 'latency', 'weighted'];

/** Holds the round-robin cursor between picks. */
export interface Cursor {
  /** Round-robin position. */
  rr: number;
}

/** Chooses one provider from a non-empty list of available ones. */
type Picker = (candidates: any[], cursor: Cursor) => any;

/** Share of picks proportional to each provider's weight. */
function weighted(candidates) {
  const total = candidates.reduce((sum, p) => sum + p.weight, 0);
  let r = Math.random() * total;
  for (const p of candidates) {
    r -= p.weight;
    if (r <= 0) return p;
  }
  return candidates[candidates.length - 1];
}

/** Lowest priority number wins. */
const priority: Picker = (candidates) => candidates.reduce((a, b) => (b.priority < a.priority ? b : a));

/** The picker for each strategy. */
const PICKERS: Record<string, Picker> = {
  'round-robin': (candidates, cursor) => candidates[cursor.rr++ % candidates.length],
  'least-connections': (candidates) =>
    candidates.reduce((a, b) => (b.active / b.maxConcurrent < a.active / a.maxConcurrent ? b : a)),
  // Unmeasured providers sort first so they get a chance to be measured.
  latency: (candidates) => candidates.reduce((a, b) => ((b.latencyMs ?? -1) < (a.latencyMs ?? -1) ? b : a)),
  weighted,
  priority,
};

/** Picks by the named strategy; an unknown one falls back to priority. */
export function pickBy(strategy, candidates, cursor: Cursor) {
  return (Object.hasOwn(PICKERS, strategy) ? PICKERS[strategy] : priority)(candidates, cursor);
}
