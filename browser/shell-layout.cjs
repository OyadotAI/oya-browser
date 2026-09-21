/**
 * Where the page and the dev panel sit in the window. Pure geometry, shared by
 * the main process (it places the page's view) and the shell tests.
 */
const {
  CHROME_HEIGHT,
  COMPACT_WIDTH,
  PANEL_MIN_WIDTH,
  PANEL_MAX_WIDTH,
  PAGE_MIN_WIDTH,
  COMPACT_PANEL_SHARE,
} = require('./constants.cjs');

/** The side panel's width: the preferred width, within bounds and leaving the page room. */
function panelWidthFor(width, preferredWidth) {
  return Math.round(Math.max(PANEL_MIN_WIDTH, Math.min(preferredWidth, PANEL_MAX_WIDTH, width - PAGE_MIN_WIDTH)));
}

/** The page's bounds once the panel has taken `reveal` pixels from the side or the bottom. */
function pageBounds(width, available, compact, reveal) {
  return {
    x: 0,
    y: CHROME_HEIGHT,
    width: Math.max(1, width - (!compact ? reveal : 0)),
    height: Math.max(1, available - (compact ? reveal : 0)),
  };
}

/** The layout for a window of `width`×`height` with the panel `open` (0..1, animated). */
function shellLayout(width, height, open, preferredWidth = 360) {
  const available = Math.max(0, height - CHROME_HEIGHT);
  const compact = width < COMPACT_WIDTH;
  const panelWidth = panelWidthFor(width, preferredWidth);
  const panelHeight = open && compact ? Math.round(available * COMPACT_PANEL_SHARE) : 0;
  const progress = Math.max(0, Math.min(1, Number(open)));
  const reveal = Math.round((compact ? panelHeight : panelWidth) * progress);
  const page = pageBounds(width, available, compact, reveal);
  return { compact, panelWidth, panelHeight, progress, reveal, chromeHeight: CHROME_HEIGHT, page };
}

module.exports = { shellLayout };
