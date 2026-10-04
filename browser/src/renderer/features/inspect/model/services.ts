/**
 * What the Inspect panes are built from: the main process, the workspace
 * panel (to register Clear and to show a pane) and the clipboard. The root
 * hands in window.oyaBrowser, the PanelViewModel and navigator.clipboard.
 */
import type { OyaBrowser } from '../../../core/bridge.ts';

/** The panel, as the Inspect panes use it. */
export interface InspectPanel {
  /** Registers what Clear does while `pane` shows; answers the undo. */
  onClear(pane: 'network' | 'source' | 'actions', clear: () => void): () => void;
  /** Shows a pane (View Page Source and Inspect show Source). */
  show(pane: 'source'): void;
}

/** Copies text (navigator.clipboard in the page). */
export interface InspectClipboard {
  /** Puts `text` on the clipboard. */
  writeText(text: string): Promise<void>;
}

/** The Inspect panes' dependencies. */
export interface InspectServices {
  /** The main process. */
  bridge: OyaBrowser;
  /** The workspace panel. */
  panel: InspectPanel;
  /** The clipboard. */
  clipboard: InspectClipboard;
}
