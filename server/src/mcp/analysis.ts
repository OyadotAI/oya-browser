/**
 * analyze's result as the model reads it: the page's markdown followed by a
 * compact index of its interactive elements, grouped by visibility.
 */
import { analysisText } from '../modules/agent/chat.ts';

/** One interactive element as analyze reports it. */
type Element = {
  /** The number the page annotates it with, [#id]. */
  id: number;
  /** Element kind, e.g. button or link. */
  type: string;
  /** Its label. */
  text?: string;
  /** Link target. */
  href?: string;
  /** Current input value. */
  value?: string;
  /** Checkbox or radio state. */
  checked?: boolean;
  /** Whether it is disabled. */
  disabled?: boolean;
  /** In the viewport without scrolling. */
  visible?: boolean;
  /** ARIA state the page keeps in attributes: expanded, selected, current, pressed. */
  state?: string;
};

/** What analyze returns. */
type Analyzed = {
  /** The page as markdown with inline element annotations. */
  markdown: string;
  /** Every interactive element found. */
  elements: Element[];
  /** Set when the page was too long to analyze whole. */
  truncated?: boolean;
};

/** An analysis, returned as text. */
export type Page = {
  /** Markdown followed by the element index. */
  page: string;
};

/** analyze's markdown plus the element index the agent reads (TOON tables of fields and elements), capped like the agent's. */
export function analysis({ markdown, elements, truncated }: Analyzed): Page {
  return { page: analysisText(markdown, elements, truncated) };
}
