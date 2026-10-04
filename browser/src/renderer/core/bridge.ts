/**
 * The shell page's way to the main process: window.oyaBrowser, typed by the
 * contract in src/shared/ipc.ts. ViewModels take it in their constructor, so a
 * test hands in a fake.
 */
import type { OyaBrowser } from '../../shared/ipc.ts';

export type { OyaBrowser };

declare global {
  /** What the preload exposes on the shell page. */
  interface Window {
    /** The bridge (src/preload/bridge.ts). */
    oyaBrowser: OyaBrowser;
  }
}

/** The bridge the preload exposed on this page. */
export const bridge = (): OyaBrowser => window.oyaBrowser;
