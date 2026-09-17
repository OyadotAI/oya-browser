const CHROME_HEIGHT = 88;
function shellLayout(width, height, open, preferredWidth = 360) {
  const available = Math.max(0, height - CHROME_HEIGHT);
  const compact = width < 960;
  const panelWidth = Math.round(Math.max(320, Math.min(preferredWidth, 560, width - 480)));
  const panelHeight = open && compact ? Math.round(available * 0.45) : 0;
  const progress = Math.max(0, Math.min(1, Number(open)));
  const reveal = Math.round((compact ? panelHeight : panelWidth) * progress);
  return {
    compact, panelWidth, panelHeight, progress, reveal, chromeHeight: CHROME_HEIGHT,
    page: { x: 0, y: CHROME_HEIGHT, width: Math.max(1, width - (!compact ? reveal : 0)), height: Math.max(1, available - (compact ? reveal : 0)) },
  };
}
module.exports = { shellLayout };
