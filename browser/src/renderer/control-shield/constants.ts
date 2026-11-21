/**
 * The control shield's own fixed values: colours and words. Its timings and
 * sizes are the SHIELD_* entries of the renderer's constants.
 */

/**
 * Oya's colours for each kind of element, from its five (core/tokens.css): lime to click, paper to
 * type in, amber to choose from. The canvas cannot read CSS variables, so the values are named here;
 * a test holds them to the five.
 */
export const TYPE_COLORS: Readonly<Record<string, string>> = {
  link: '#cdf01e',
  button: '#cdf01e',
  input: '#f7f5f0',
  textarea: '#f7f5f0',
  editable: '#f7f5f0',
  select: '#f5b14c',
};

/** The colour of an element of a kind with no colour of its own: a click's. */
export const DEFAULT_COLOR = '#cdf01e';

/**
 * The veil's smoke, in ink, for a light page and a dark one: lighter in the middle and darker
 * towards the edges, like a vignette. A light page takes a thin smoke, so it dims without going
 * grey; a dark page needs a thick one, or the dim would not show.
 */
export const VEIL_SMOKE = {
  light: { middle: '#1a1a1a33', edge: '#1a1a1a75' },
  dark: { middle: '#1a1a1a80', edge: '#1a1a1abd' },
} as const;

/** The light a window lets in on a dark page, where clearing the smoke alone would leave it as dark as the page: a faint lime. */
export const VEIL_LIFT = '#cdf01e14';

/** A cut takes away as much as its own colour covers, so it must be solid to clear the smoke fully. */
export const VEIL_CUT = '#1a1a1a';

/** What Oya says. */
export const WORDS = {
  /** While the agent reads the page. */
  reading: 'Reading the page',
  /** When the reading found nothing to act on. */
  nothing: 'Nothing to interact with here',
  /** When the run ends. */
  done: 'Done',
} as const;
