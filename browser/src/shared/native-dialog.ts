/** Private browser-owned dialog bridge; never installed in website content. */
export const NATIVE_DIALOG = {
  READ: 'oya:native-dialog:read',
  READY: 'oya:native-dialog:ready',
  ANSWER: 'oya:native-dialog:answer',
  WIDTH: 480,
  HEIGHT: 352,
  LOAD_TIMEOUT_MS: 10000,
} as const;
/** Minimal text-only presentation data; no engine callback crosses IPC. */
export interface DialogPresentation {
  /** Actual frame origin, displayed as text. */
  origin: string;
  /** Browser-owned heading. */
  title: string;
  /** Website message, displayed as text only. */
  message: string;
  /** Only prompts show an editable field. */
  prompt: boolean;
  /** Initial field value, not an implicit answer. */
  value: string;
  /** Explicit positive action. */
  accept: string;
  /** Safe default action. */
  cancel: string;
}
