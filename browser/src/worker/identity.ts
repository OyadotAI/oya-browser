/**
 * Whether the one element a locator found is the element that was recorded.
 * One match is not proof: a text locator for a filter link ("Clothing, Shoes &
 * Jewelry" in a search's Shop By list) can match the top menu's link of the
 * same name, and a replay that clicks it passes on the wrong page. The found
 * element must sit in an element of the recorded tag (a text match often lands
 * on the span inside a link) and, for a link, lead to the same path.
 */
import type { Locator } from '@playwright/test';
import type { RecordedElement } from '../workflow/index.ts';

/** What the page says about the found element. */
interface Found {
  /** Whether it sits in an element of the recorded tag. */
  within: boolean;
  /** That element's link, if it is one. */
  href: string;
}

/** A URL's path, which a session's query tokens and hosts do not change. */
function pathOf(href: string): string {
  try {
    return new URL(href).pathname;
  } catch {
    return '';
  }
}

/** What the page says about the found element: the recorded tag around it, and that element's link. */
function describe(node: Element, tag: string): Found {
  const at = node.closest(tag);
  return { within: !!at, href: (at as HTMLAnchorElement | null)?.href || '' };
}

/** True unless the found element is plainly not the recorded one (another tag, or a link elsewhere). */
export async function isRecorded(locator: Locator, el: RecordedElement = {}, timeout?: number): Promise<boolean> {
  if (!el.tag || typeof locator.evaluate !== 'function') return true;
  const found = await locator.evaluate(describe, el.tag, { timeout }).catch(() => null);
  if (!found) return true;
  if (!found.within) return false;
  return !el.href || !found.href || pathOf(found.href) === pathOf(el.href);
}
