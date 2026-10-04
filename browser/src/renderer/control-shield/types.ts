/**
 * The shapes the control shield works with: boxes on the page, and the updates
 * the main process sends through `window.oyaShield(update)`.
 */

/** A box on the page, in the shield's CSS pixels. */
export interface Rect {
  /** Left edge. */
  x: number;
  /** Top edge. */
  y: number;
  /** Width. */
  w: number;
  /** Height. */
  h: number;
}

/** A point on the page, in the shield's CSS pixels. */
export interface Point {
  /** Across. */
  x: number;
  /** Down. */
  y: number;
}

/** Where one element the agent found sits, and its kind (which colours its outline). */
export interface ShieldBox extends Rect {
  /** The element's analysis id, shown as its number. */
  id: number;
  /** Its kind: link, button, input, textarea, editable or select. */
  type: string;
}

/** How light the page under the shield is, which sets the veil's smoke. */
export type Tone = 'light' | 'dark';

/** An agent began reading the page. */
export interface ScanUpdate {
  /** Which update this is. */
  phase: 'scan';
}

/** The reading is back: what it found that the page shows. */
export interface FoundUpdate {
  /** Which update this is. */
  phase: 'found';
  /** The elements found, in page order. */
  boxes?: ShieldBox[];
  /** How light the page is, when it could be measured. */
  tone?: Tone;
}

/** The page moved: where the found elements sit now (an empty list when the page is gone). */
export interface MoveUpdate {
  /** Which update this is. */
  phase: 'move';
  /** The found elements still on the page, where they are now. */
  boxes?: ShieldBox[];
}

/** The agent is acting on the page. */
export interface ActUpdate {
  /** Which update this is. */
  phase: 'act';
  /** What Oya says it is doing. */
  text: string;
  /** The element it acts on, when it could be measured. */
  box?: ShieldBox | null;
  /** Whether the action changes the page, so the show's outlines are out of date. */
  changes?: boolean;
}

/** The agent's run ended. */
export interface EndUpdate {
  /** Which update this is. */
  phase: 'end';
}

/** Every update the main process sends. */
export type ShieldUpdate = ScanUpdate | FoundUpdate | MoveUpdate | ActUpdate | EndUpdate;

/** The name of an update. */
export type Phase = ShieldUpdate['phase'];

/** The window measures the shield draws by. */
export interface ViewPort {
  /** The page's width. */
  innerWidth: number;
  /** The page's height. */
  innerHeight: number;
  /** Device pixels per CSS pixel. */
  devicePixelRatio: number;
}
