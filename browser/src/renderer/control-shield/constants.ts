/**
 * The control shield's own fixed values: colours and words. Its timings and
 * sizes are the SHIELD_* entries of the renderer's constants.
 */

/** Oya's colours for each kind of element: green to click, blue to type in, amber to choose from. */
export const TYPE_COLORS: Readonly<Record<string, string>> = {
  link: '#39ed35',
  button: '#39ed35',
  input: '#6cb4ff',
  textarea: '#6cb4ff',
  editable: '#6cb4ff',
  select: '#f5a623',
};

/** The colour of an element of a kind with no colour of its own: a click's. */
export const DEFAULT_COLOR = '#39ed35';

/**
 * The veil's smoke, for a light page and a dark one: lighter in the middle and darker
 * towards the edges, like a vignette. A light page takes a thin cool smoke, so it dims
 * without going grey; a dark page needs a deep black one, or the dim would not show.
 */
export const VEIL_SMOKE = {
  light: { middle: 'rgba(6, 14, 20, 0.2)', edge: 'rgba(6, 14, 20, 0.46)' },
  dark: { middle: 'rgba(0, 0, 0, 0.5)', edge: 'rgba(0, 0, 0, 0.74)' },
} as const;

/** The light a window lets in on a dark page, where clearing the smoke alone would leave it as dark as the page: a faint mint. */
export const VEIL_LIFT = 'rgba(166, 255, 201, 0.08)';

/** A cut takes away as much as its own colour covers, so it must be solid to clear the smoke fully. */
export const VEIL_CUT = '#000';

/** What Oya says. */
export const WORDS = {
  /** While the agent reads the page. */
  reading: 'Reading the page',
  /** When the reading found nothing to act on. */
  nothing: 'Nothing to interact with here',
  /** When the run ends. */
  done: 'Done',
} as const;
