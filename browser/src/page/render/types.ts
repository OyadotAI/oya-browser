/**
 * The page as the analyzer (scripts/analyzer.js) collects it, and the shape
 * every renderer shares. The analyzer runs in the page, so these fields are
 * what it reports, read loosely.
 */

/** The page's facts: url, title, scroll and the others that apply, in reading order. */
export type Facts = Record<string, unknown>;

/** One block of the page in reading order: a heading, a paragraph, a row, or an element with an id. */
export interface Block {
  /** The element's id, for an element you can act on. */
  id?: number | string;
  /** The part of the page it is in (nav, main, form/..., dialog). */
  region?: string;
  /** What it is: h1-h6, text, item, row, header, link, button, input:email, ... */
  kind: string;
  /** Its text or name. */
  text?: string;
  /** Where a link goes, or what a field holds. */
  target?: string;
  /** Its state words. */
  state?: string;
  /** Anything else the analyzer reported. */
  [field: string]: unknown;
}

/** One interactive element, as the element index lists it. */
export interface PageElement {
  /** Its id. */
  id: number | string;
  /** input, textarea, select, checkbox, radio, editable, link, button, ... */
  type: string;
  /** An input's own type (text, date, password). */
  inputType?: string;
  /** Its name. */
  text?: string;
  /** What a field holds. */
  value?: string;
  /** A select's options. */
  options?: string;
  /** A field's placeholder. */
  placeholder?: string;
  /** Where a link goes. */
  href?: string;
  /** ARIA state words. */
  state?: string;
  /** The page's error for an invalid field. */
  error?: string;
  /** Whether it is on screen. */
  visible?: boolean;
  /** Whether a toggle is on. */
  checked?: boolean;
  /** Field flags. */
  required?: boolean;
  /** Whether it is disabled. */
  disabled?: boolean;
  /** Whether it is read-only. */
  readOnly?: boolean;
  /** Whether the page marks it invalid. */
  invalid?: boolean;
}

/** An analysis: the facts, the blocks, the elements, and whether the page was cut. */
export interface Analysis {
  /** The page's facts. */
  facts?: Facts;
  /** The blocks in reading order. */
  blocks?: Block[];
  /** The interactive elements. */
  elements?: PageElement[];
  /** Whether the analyzer cut the page short. */
  truncated?: boolean;
  /** Anything else the analysis carries (format, page, markdown). */
  [field: string]: unknown;
}

/** One page format: the same interface for every renderer. */
export interface Renderer {
  /** Its name in configuration. */
  readonly format: string;
  /** The page as text. */
  render(analysis: Analysis): string;
  /** That text cut to `max` characters, still valid. */
  fit(text: string, max: number): string;
  /** What a model reads, within its budget. */
  forAgent(analysis: Analysis, max: number): string;
  /** The controls alone, within `max`. */
  controls(analysis: Analysis, max: number): string;
  /** How the agent is told to read this format. */
  readonly guide: string;
}
