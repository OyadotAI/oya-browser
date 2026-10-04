/**
 * A line that reports how something went (`role="status"`), so assistive
 * technology reads it out when it changes. The caller gives its id and class.
 */
import type { HTMLAttributes } from 'react';

/** What a StatusLine is given: a paragraph's attributes, and whether it is a polite live region. */
export interface StatusLineProps extends HTMLAttributes<HTMLParagraphElement> {
  /** Announce changes politely (`aria-live="polite"`). */
  live?: boolean;
}

/** A status paragraph. */
export function StatusLine({ live, ...rest }: StatusLineProps) {
  return <p role="status" aria-live={live ? 'polite' : undefined} {...rest} />;
}
