/**
 * Every number the page-side helpers use, by name: page rendering, the
 * recorder channel, typed dates and the agent's element queries.
 */

/**
 * Page rendering: the longest page an analysis returns, the budget cut keeps for
 * its note, and the element index's off-screen list and link lengths.
 */
export const PAGE_RENDER = {
  MAX_PAGE_CHARS: 80000,
  TRUNCATED_ROOM: 120,
  MAX_OFFSCREEN_LISTED: 30,
  MAX_INDEX_LINK: 80,
} as const;

/** Recording: the binding name's random bytes, how long the page has to connect, and CDP's name/value attribute pairs. */
export const RECORDING = {
  BINDING_BYTES: 12,
  READY_TIMEOUT_MS: 3000,
  ATTRIBUTE_STRIDE: 2,
  FRAME_ATTACH_GRACE_MS: 2000,
} as const;

/**
 * Dates typed into native date inputs: the last month, day, hour and minute,
 * where a two-digit year is placed, the noon hour for AM/PM, and pad widths.
 */
export const DATES = {
  MONTHS: 12,
  MAX_DAY: 31,
  HOURS: 24,
  MINUTES: 60,
  CENTURY: 2000,
  NOON: 12,
  YEAR_WIDTH: 4,
  PART_WIDTH: 2,
} as const;

/** The agent's read_elements: how many elements it lists when not told. */
export const QUERIES = { DEFAULT_LIMIT: 50 } as const;
