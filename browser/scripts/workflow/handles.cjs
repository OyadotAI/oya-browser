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

/**
 * Query parameters a site regenerates per visit: a request id, a session id, a
 * tracking tag. Amazon's pagination link carries `qid` (an epoch) and `xpid` (a
 * session), so the same "page 2" link reads differently every time — while
 * `page=2` itself is exactly what distinguishes it from page 3. Dropping the
 * volatile ones and keeping the rest is what makes a link comparable at all.
 */
const VOLATILE_PARAMS =
  /^(utm_[a-z]+|qid|xpid|_gl|_ga|gclid|fbclid|msclkid|igshid|ved|ei|sa|usg|sid|sessionid|sessid|nonce|csrf|csrftoken|requestid|rid|reqid|ts|timestamp|trk|trackingid|rdt|si|correlationid)$/i;

/** A link's target as the page wrote it, which is what a CSS locator matches. */
const rawTargetOf = (el) => el.rawHref ?? el.href;

/**
 * The same target with the per-visit noise removed and the rest in a fixed order,
 * so two renderings of one link compare equal and two different links do not.
 */
function stableTarget(href) {
  if (!href) return undefined;
  try {
    return withoutNoise(new URL(String(href), RELATIVE_BASE));
  } catch {
    return String(href);
  }
}

/** The base a relative href is parsed against, and stripped from the answer again. */
const RELATIVE_BASE = 'http://relative.invalid';

/** The url without its per-visit parameters, its remaining query in a fixed order. */
function withoutNoise(url) {
  for (const key of [...url.searchParams.keys()]) if (VOLATILE_PARAMS.test(key)) url.searchParams.delete(key);
  url.searchParams.sort();
  const query = url.searchParams.toString();
  const origin = url.origin === RELATIVE_BASE ? '' : url.origin;
  return `${origin}${url.pathname}${query ? `?${query}` : ''}`;
}

/** A link's target for comparison: what it points at, not which visit wrote it. */
const targetOf = (el) => stableTarget(rawTargetOf(el));

/**
 * Whether this element is the kind of thing a target belongs to.
 *
 * A target only identifies a link. A button that has somehow acquired one — an href
 * inherited from a stale analysis entry — must not be aimed at as `a[href="..."]`,
 * which is how a recorded click on a Start button replayed onto a footer link.
 */
const isLink = (el) => String(el.tag || '').toLowerCase() === 'a' || el.type === 'link';

/**
 * Whether a target carries per-visit noise, and so cannot be matched literally.
 *
 * Comparing two targets is safe — the noise is dropped from both. Writing one into
 * a selector is not: `a[href="...&qid=1789940643"]` matched the link it was recorded
 * from and nothing at all on the next visit, and the replay went on to click
 * whatever the browser found instead.
 */
function volatileTarget(href) {
  if (!href) return false;
  try {
    return [...new URL(String(href), RELATIVE_BASE).searchParams.keys()].some((k) => VOLATILE_PARAMS.test(k));
  } catch {
    return false;
  }
}

/**
 * The handles, most trustworthy first.
 *
 * `of` reads the handle from a recorded or live element, or nothing when it has
 * none. `scope` says what else must agree for two elements to be the same one: a
 * name means little across different tags, and text means little across types.
 */
const HANDLES = [
  { kind: 'testId', of: (el) => el.testId },
  { kind: 'href', of: (el) => (isLink(el) ? targetOf(el) : undefined), scope: 'tag' },
  // An id the page author wrote. One this render invented is not a handle at all.
  { kind: 'domId', of: (el) => (stableId(el.domId) ? el.domId : undefined) },
  { kind: 'ariaLabel', of: (el) => (el.ariaLabel ? withoutLiveCount(el.ariaLabel) : undefined), scope: 'tag' },
  // The same name, inside the nearest container where it is the only one: what the
  // analyzer works out for a name the page repeats. "Delete" in the second row is
  // this, and it is the difference between the right row and the first one.
  { kind: 'scoped', of: (el) => el.scoped, scope: 'tag' },
  // `drifts` because visible text is the handle most likely to read differently
  // and still be the same control: a data-driven label, a count, a date. Its
  // absence is evidence; a different value is not.
  //
  // `ambiguous` for a name the analyzer marked as repeating: it cannot be used to
  // find the element, because it never said which one — `scoped` above carries it
  // when a container could be named, and position is what is left when none could.
  // It is still read for identity, though: a slot whose occupant has lost the name
  // that was recorded is the wrong occupant, repeating name or not.
  { kind: 'text', of: (el) => el.text, scope: 'type', drifts: true, ambiguous: (el) => Boolean(el.repeats) },
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

/**
 * The handles this element can be found again by, in order of trust. A handle the
 * element does not carry is left out, and so is one that cannot tell this element
 * from its siblings — both are handles nothing can aim with.
 */
function handlesOf(el = {}) {
  const has = (v) => v !== undefined && v !== '' && v !== null;
  return HANDLES.filter((h) => has(h.of(el)) && !h.ambiguous?.(el));
}

module.exports = {
  HANDLES,
  handlesOf,
  contradicts,
  missingIdentity,
  stableId,
  withoutLiveCount,
  targetOf,
  stableTarget,
  rawTargetOf,
  volatileTarget,
  GENERATED_ID,
  LIVE_COUNT,
};
