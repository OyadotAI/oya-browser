/** Private source-picker contract; native capture handles never cross into its renderer. */
export const SHARE_PICKER = {
  READ: 'oya:share-picker:read',
  READY: 'oya:share-picker:ready',
  ANSWER: 'oya:share-picker:answer',
  SETTINGS: 'oya:share-picker:settings',
} as const;

/** Categories offered by the browser-owned picker. */
export type ShareKind = 'tab' | 'window' | 'screen';

/** A preview and opaque choice token, safe to render as text and a browser-produced image. */
export interface ShareOption {
  /** Random identifier scoped to this picker, never a native source identifier. */
  id: string;
  /** Tab, application window or entire display. */
  kind: ShareKind;
  /** Human-readable source title. */
  name: string;
  /** Secondary label, such as a tab's origin. */
  detail: string;
  /** Browser-produced PNG, or an empty string when no preview is available. */
  thumbnail: string;
}

/** Everything the picker may display, with no page-provided markup. */
export interface SharePresentation {
  /** Site receiving the selected stream. */
  origin: string;
  /** Sources this request is allowed to share. */
  sources: ShareOption[];
  /** Explains an unavailable desktop list while still allowing browser tabs. */
  desktopError: string;
  /** Show a browser-owned macOS recovery action, never a website-supplied URL. */
  canOpenSettings: boolean;
}
