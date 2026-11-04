/** Native picker size, thumbnail bounds and bounded lifecycle checks. */
export const PICKER = {
  WIDTH: 780,
  HEIGHT: 620,
  THUMBNAIL: { width: 280, height: 158 },
  LOAD_TIMEOUT_MS: 10000,
  PREVIEW_TIMEOUT_MS: 3000,
  PREVIEW_RETRY_MS: 50,
  PREVIEW_WIDTH: 1280,
  PREVIEW_HEIGHT: 800,
  SOURCES_TIMEOUT_MS: 10000,
  OWNER_CHECK_MS: 200,
  TAB_LIMIT: 80,
  BACKGROUND: '#10120f',
  DESKTOP_ERROR:
    'Windows and screens couldn’t be listed. Cancel and try sharing again. Available Oya tabs are listed separately.',
  DESKTOP_EMPTY: 'No windows or screens are available. Available Oya tabs are listed separately.',
  PERMISSION_ERROR: 'Screen recording access is blocked by macOS. Allow Oya Browser in System Settings and try again.',
  RESTRICTED_ERROR: 'Your Mac restricts screen recording. This setting may be managed by your organization.',
  SETTINGS_URL: 'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture',
} as const;

/** Browser-owned fixed-size sheet, styled consistently with native Oya dialogs. */
export const PICKER_WINDOW = {
  width: PICKER.WIDTH,
  height: PICKER.HEIGHT,
  show: false,
  title: 'Oya · Share a tab, window or screen',
  resizable: false,
  minimizable: false,
  maximizable: false,
  backgroundColor: PICKER.BACKGROUND,
};
/** Privilege isolation is independent of the requesting site's profile. */
export const PICKER_WEB = {
  sandbox: true,
  contextIsolation: true,
  nodeIntegration: false,
  webviewTag: false,
  disableDialogs: true,
};
/** Ask the OS for bounded thumbnails of every supported desktop source. */
export const DESKTOP_SOURCES = {
  types: ['window', 'screen'] as ('window' | 'screen')[],
  thumbnailSize: PICKER.THUMBNAIL,
};
/** How often a live tab share is checked for having ended; the badge clears at most this late. */
export const SHARE_POLL_MS = 1000;
