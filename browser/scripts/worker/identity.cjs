/**
 * Whether the one element a locator found is the element that was recorded.
 * One match is not proof: a text locator for a filter link ("Clothing, Shoes &
 * Jewelry" in a search's Shop By list) can match the top menu's link of the
 * same name, and a replay that clicks it passes on the wrong page. The found
 * element must sit in an element of the recorded tag (a text match often lands
 * on the span inside a link) and, for a link, lead to the same path.
 */

/** A URL's path, which a session's query tokens and hosts do not change. */
function pathOf(href) {
  try {
    return new URL(href).pathname;
  } catch {
    return '';
  }
}

/** What the page says about the found element: the recorded tag around it, and that element's link. */
function describe(node, tag) {
  const at = node.closest(tag);
  return { within: !!at, href: at?.href || '' };
}

/** True unless the found element is plainly not the recorded one (another tag, or a link elsewhere). */
async function isRecorded(locator, el = {}, timeout) {
  if (!el.tag || typeof locator.evaluate !== 'function') return true;
  const found = await locator.evaluate(describe, el.tag, { timeout }).catch(() => null);
  if (!found) return true;
  if (!found.within) return false;
  return !el.href || !found.href || pathOf(found.href) === pathOf(el.href);
}

module.exports = { isRecorded };
