/** IPC: overlays over the page, the dev panel, and the shell's own preferences. */
const { THEMES, PANES } = require('./constants.cjs');
const { FORMATS, DEFAULT_FORMAT } = require('../../scripts/page-render.cjs');

/** Each preference the shell may save, and the values it accepts. */
const PREFERENCES = { theme: THEMES, pane: PANES, pageFormat: FORMATS };

/** Saves the theme, pane and page format the shell chose, ignoring anything else. */
function saveUiPreferences(ctx, _e, preferences) {
  if (!preferences || typeof preferences !== 'object') return false;
  const ui = { ...ctx.config.values.ui };
  for (const [key, allowed] of Object.entries(PREFERENCES))
    if (allowed.includes(preferences[key])) ui[key] = preferences[key];
  ctx.config.values.ui = ui;
  ctx.config.save();
  ctx.shell.window?.setBackgroundColor(ctx.shell.background());
  return true;
}

/** Channel → handler. */
const SHELL_HANDLERS = {
  'backdrop-ready': (ctx, _event, token) => ctx.overlays.backdropReady(token),
  'show-overlay': (ctx, _e, name) => ctx.overlays.show(name),
  'hide-overlay': (ctx, _e, name) => ctx.overlays.hide(name),
  'toggle-dev-panel': (ctx, _event, reducedMotion) => ctx.layout.toggle(reducedMotion),
  'resize-dev-panel': (ctx, _e, width) => ctx.layout.resize(width),
  'get-ui-preferences': (ctx) => ({
    theme: 'system',
    pane: 'chat',
    pageFormat: DEFAULT_FORMAT,
    pageFormats: FORMATS,
    ...ctx.config.values.ui,
    platform: process.platform,
    systemDark: ctx.electron.nativeTheme.shouldUseDarkColors,
  }),
  'save-ui-preferences': saveUiPreferences,
};

module.exports = { SHELL_HANDLERS };
