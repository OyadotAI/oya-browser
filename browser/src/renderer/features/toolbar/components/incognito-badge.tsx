/**
 * The toolbar's "Incognito" badge, where the Ask button sits in a normal
 * window. It has no action, so it is not a button and takes no focus; it is in
 * the toolbar's nav, so a screen reader reads it.
 */
import { TEXT } from '../model/constants.ts';

/** Marks an incognito window. */
export function IncognitoBadge() {
  return <span className="incognito-badge">{TEXT.incognito}</span>;
}
