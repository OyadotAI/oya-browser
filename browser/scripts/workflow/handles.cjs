/**
 * What an element can be found by again, and in what order to trust it.
 *
 * This is the one place that knows it. Three copies of this order existed — the
 * live replay matcher, the version-1 Playwright export and the version-2 locator
 * builder — and each carried its own idea of which handles were safe. Fixing a
 * handle in one left the other two aiming at a live notification count or an id a
 * framework invented for that render, which is how a recording came to click a
 * different button than the one it recorded.
 *
 * A consumer takes the order from here and renders each handle its own way: as a
 * Playwright locator, or as a predicate over the analyzer's elements.
 */

/**
 * Ids a framework makes up per render, which look like handles and are not:
 * Wikipedia's Parsoid numbers every node `mwAQ`, `mwCg`; React's useId gives
 * `:r3:`; Ember, ExtJS and Radix have their own. A recording that aims at one
 * finds a different element, or none, the next time the page renders.
 */
const GENERATED_ID =
  /^(mw[\w-]{1,4}|:r[0-9a-z]+:|ember\d+|ext-gen\d+|radix-[\w:-]+|[a-f0-9]{8}-[a-f0-9]{4}-)|[0-9]{6,}/i;

/** Whether this id will still name the same element after a re-render. */
function stableId(domId) {
  return Boolean(domId) && !GENERATED_ID.test(String(domId));
}

/**
 * A count a page keeps up to date, inside an accessible name: LinkedIn labels its
 * nav "Home, 1 new notification" and renames it the moment a notification
 * arrives, so a locator built on the whole label finds nothing on the next run.
 */
const LIVE_COUNT = /,?\s*\d+\+?\s+(new|unread)\b[^,]*|\s*\(\d+\+?\)\s*$/gi;

/** The name without its live count, or the name itself when that leaves nothing. */
function withoutLiveCount(name) {
  const cleaned = String(name ?? '')
    .replace(LIVE_COUNT, '')
    .replace(/\s+/g, ' ')
    .replace(/[,\s]+$/, '')
    .trim();
  return cleaned || String(name ?? '');
}

/** A link's target as the page wrote it, which is what a CSS locator matches. */
const targetOf = (el) => el.rawHref ?? el.href;

/**
 * The handles, most trustworthy first.
 *
 * `of` reads the handle from a recorded or live element, or nothing when it has
 * none. `scope` says what else must agree for two elements to be the same one: a
 * name means little across different tags, and text means little across types.
 */
const HANDLES = [
  { kind: 'testId', of: (el) => el.testId },
  { kind: 'href', of: targetOf },
  // An id the page author wrote. One this render invented is not a handle at all.
  { kind: 'domId', of: (el) => (stableId(el.domId) ? el.domId : undefined) },
  { kind: 'ariaLabel', of: (el) => (el.ariaLabel ? withoutLiveCount(el.ariaLabel) : undefined), scope: 'tag' },
  // `drifts` because visible text is the handle most likely to read differently
  // and still be the same control: a data-driven label, a count, a date. Its
  // absence is evidence; a different value is not.
  { kind: 'text', of: (el) => el.text, scope: 'type', drifts: true },
  { kind: 'name', of: (el) => el.name, scope: 'tag' },
  { kind: 'placeholder', of: (el) => el.placeholder, scope: 'tag' },
  // Where it sits: all that is left for an element with no name and no target.
  // Scoped to the tag because a position among same-shaped siblings is exactly
  // where it goes wrong — the send button and the emoji button sit side by side.
  // `positional` because a place is not an identity: an element that moved is
  // still itself, so a differing path is no evidence of a different element —
  // while a differing label or name is.
  { kind: 'path', of: (el) => el.path, scope: 'tag', positional: true },
];

/**
 * Whether a live element contradicts what was recorded.
 *
 * Matching on one handle is not enough on its own: a composer's send button, its
 * emoji button and its expand-to-full-screen button are siblings with the same
 * shape, so a position or a weak name can land on the wrong one. If both elements
 * carry a handle and the values differ, they are not the same element, whatever
 * else agreed.
 *
 * Handles that drift are left out of this: a label built from a variable, a count
 * or a date is expected to read differently and still be the same control.
 */
function contradicts(el = {}, candidate = {}) {
  return HANDLES.filter((h) => !h.positional && !h.drifts).some((h) => {
    const [was, now] = [h.of(el), h.of(candidate)];
    if (was === undefined || was === null || was === '') return false;
    if (now === undefined || now === null || now === '') return false;
    return String(was).trim().toLowerCase() !== String(now).trim().toLowerCase();
  });
}

/**
 * Whether the candidate is missing a name the recording had. Only asked of a
 * match made on position alone: if the recording knew this button said "Send"
 * and the thing in that slot says nothing, it is the wrong button.
 */
function missingIdentity(el = {}, candidate = {}) {
  return HANDLES.filter((h) => !h.positional).some((h) => {
    const was = h.of(el);
    if (was === undefined || was === null || was === '') return false;
    const now = h.of(candidate);
    return now === undefined || now === null || now === '';
  });
}

/** The handles this element actually has, in order of trust. */
function handlesOf(el = {}) {
  return HANDLES.filter((h) => h.of(el) !== undefined && h.of(el) !== '' && h.of(el) !== null);
}

module.exports = { HANDLES, handlesOf, contradicts, missingIdentity, stableId, withoutLiveCount, targetOf, GENERATED_ID, LIVE_COUNT };
