/**
 * Arrow-key navigation for the section tabs, as the ARIA tabs pattern expects:
 * arrows wrap around, Home and End jump to the ends.
 */

/** Given the current index and the tab count, where each key moves to. */
const MOVES: Record<string, (index: number, count: number) => number> = {
  Home: () => 0,
  End: (_index, count) => count - 1,
  ArrowRight: (index, count) => (index + 1) % count,
  ArrowDown: (index, count) => (index + 1) % count,
  ArrowLeft: (index, count) => (index + count - 1) % count,
  ArrowUp: (index, count) => (index + count - 1) % count,
};

/** The tab `key` moves to from `index`, or null when the key is not a navigation key. */
export function nextTab(key: string, index: number, count: number): number | null {
  return Object.hasOwn(MOVES, key) ? MOVES[key](index, count) : null;
}
