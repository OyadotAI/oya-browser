/**
 * The shapes page commands pass around: what a command is sent, what a page
 * script answers, and what an element search finds.
 */

/** How the server tells its commands apart; the answer carries it back. */
export type CommandId = string | number;

/**
 * What a server command or a dev panel action may carry. Every field is
 * optional: each command reads the few it needs and checks them itself.
 */
export interface CommandParams {
  /** The address to load. */
  url?: string;
  /** The page format for an analysis, or 'jpeg' for a screenshot. */
  format?: string;
  /** The element to act on: an analyzer id selector or CSS. */
  selector?: string;
  /** The text to type. */
  text?: string;
  /** The key to press. */
  key?: string;
  /** The value to pick in a <select>. */
  value?: string;
  /** The agent's script for run_script. */
  script?: string;
  /** The page-world expression for evaluate_raw. */
  expression?: unknown;
  /** A point's distance from the left, in page pixels. */
  x?: number;
  /** A point's distance from the top, in page pixels. */
  y?: number;
  /** Where a drag starts, from the left. */
  from_x?: number;
  /** Where a drag starts, from the top. */
  from_y?: number;
  /** Where a drag ends, from the left. */
  to_x?: number;
  /** Where a drag ends, from the top. */
  to_y?: number;
  /** False for live control's one-shot scroll. */
  smooth?: boolean;
  /** How far to scroll; read as a number whatever it is. */
  amount?: unknown;
  /** 'up', or down otherwise. */
  direction?: string;
  /** The dev panel's analyzer element number. */
  element_id?: unknown;
  /** How long a wait lasts; read as a number whatever it is. */
  timeout?: unknown;
  /** Options handed to the analyzer after a scroll. */
  analyze?: object;
  /** The most elements read_page returns. */
  limit?: number;
}

/** What a page script answers: success, its data, or why it failed. */
export interface ScriptResult<D = unknown> {
  /** Whether it worked; missing counts as yes. */
  ok?: boolean;
  /** What it found. */
  data?: D;
  /** Why it failed. */
  error?: string;
}

/** Where an element sits and what it is, as the finder script reports it. */
export interface ElementSpot {
  /** Its centre's distance from the left, in page pixels. */
  x: number;
  /** Its centre's distance from the top, in page pixels. */
  y: number;
  /** Whether it lives in an iframe, where the DOM events are replayed too. */
  inIframe?: boolean;
  /** Whether something drawn over it would take a click at its centre. */
  covered?: boolean;
  /** The handles a replay finds it by again. */
  handle?: object;
}

/** An element search: found with its spot, or not with the reason. */
export interface Located {
  /** Whether the element was found. */
  ok: boolean;
  /** Where it is, when found. */
  data: ElementSpot;
  /** Why it was not found. */
  error?: string;
}

/** What CDP's Page.captureScreenshot answers. */
export interface Screenshot {
  /** The image, base64-encoded. */
  data: string;
}

/** A viewport's size, as the page reports it. */
export interface Viewport {
  /** Its width in pixels. */
  w?: number;
  /** Its height in pixels. */
  h?: number;
}

/** What an analysis found, as far as telling an empty page goes. */
export interface AnalysisData {
  /** The interactive elements on the page. */
  elements?: unknown[];
}

/** Where a page ended up. */
export interface Landed {
  /** Its address. */
  url: string;
  /** Its title. */
  title: string;
}
