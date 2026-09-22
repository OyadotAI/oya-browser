/**
 * Locators: the ways a recorded element can be found again, and the Playwright
 * code that finds it.
 */

/** Every locator kind a step may use. */
const { stableId, withoutLiveCount, volatileTarget, VOLATILE_PARAMS } = require('./handles.cjs');

const LOCATOR_KINDS = ['testId', 'role', 'label', 'text', 'placeholder', 'css'];

/** The Playwright page method behind each text-based locator kind. */
const LOCATOR_METHODS = {
  testId: 'getByTestId',
  label: 'getByLabel',
  text: 'getByText',
  placeholder: 'getByPlaceholder',
};

/** Element types that are form fields: their DOM handles outrank their text. */
const FIELD = new Set(['input', 'textarea', 'select', 'checkbox', 'radio', 'editable']);

/** Field types whose recorded text is a label rather than visible text. */
const LABELLED = ['input', 'textarea', 'editable'];

/** A link's locators: its target as written, and its path for the same page with another query. */
function hrefCandidates(el) {
  const href = el.rawHref ?? el.href;
  // A link to "#" or a script matches every such link on the page: no locator at all.
  if (!href || /^(#|javascript:)/i.test(href)) return [];
  const base = href.split(/[?#]/)[0];
  const exact = { kind: 'css', value: `a[href=${JSON.stringify(href)}]` };
  if (!base || base === href) return [exact];
  // The same page with another query (a filter, a tracking tag) is still the link.
  const prefix = { kind: 'css', value: `a[href^=${JSON.stringify(base)}]` };
  return volatileTarget(href) ? volatileHrefCandidates(href, base, prefix) : [exact, prefix];
}

/** A link whose query carries per-visit tokens (a session, a request id): what comes before them, then its path. */
function volatileHrefCandidates(href, base, prefix) {
  const stable = stablePrefix(href);
  return stable === base ? [prefix] : [{ kind: 'css', value: `a[href^=${JSON.stringify(stable)}]` }, prefix];
}

/**
 * The link's target up to its first per-visit parameter: all of it that the next
 * visit writes the same. Amazon's brand filter is `/s?k=cable&rh=…&qid=…`; up to
 * `qid` it names this brand, while `/s` alone names every search link on the page.
 */
function stablePrefix(href) {
  const [path, query = ''] = href.split('#')[0].split(/\?(.*)/s);
  const kept = [];
  for (const pair of query.split('&').filter(Boolean)) {
    if (VOLATILE_PARAMS.test(decodeURIComponent(pair.split('=')[0]))) break;
    kept.push(pair);
  }
  return path + (kept.length ? '?' + kept.join('&') : '');
}

/**
 * CSS locators from the element's group and value, id, name and link target.
 * Inside a component's shadow root those are unique only within it, so each is
 * scoped under the component's host.
 */
function attributeCandidates(el) {
  const within = el.host ? `${el.host} ` : '';
  return plainAttributeCandidates(el).map((c) => ({ ...c, value: within + c.value }));
}

/** The attribute locators as if the element were in the page itself. */
function plainAttributeCandidates(el) {
  const out = [];
  // A checkbox or radio by its group and value: stable where the page makes up its id.
  if (el.name && el.choice)
    out.push({ kind: 'css', value: `input[name=${JSON.stringify(el.name)}][value=${JSON.stringify(el.choice)}]` });
  if (stableId(el.domId)) out.push({ kind: 'css', value: `[id=${JSON.stringify(el.domId)}]` });
  if (el.name) out.push({ kind: 'css', value: `[name=${JSON.stringify(el.name)}]` });
  // A target the page repeats is tried only after the element's position (see candidates).
  if (el.hrefRepeats) return out;
  // A link's target outlives any id a renderer invents for it, so it comes before
  // a generated id rather than after every attribute. The generated id itself is
  // reported by `generatedId` so it can be ordered behind the element's position:
  // where Wikipedia renumbers every node per render, its place in the article is
  // the more durable of the two.
  return [...out, ...hrefCandidates(el)];
}

/** Locators from what the element says about itself: role, label, text, placeholder. */
function nameCandidates(el) {
  const out = [];
  const label = withoutLiveCount(el.ariaLabel);
  if (el.role && (label || el.text)) out.push({ kind: 'role', role: el.role, value: label || el.text });
  if (label) out.push({ kind: 'label', value: label });
  if (el.text) out.push({ kind: LABELLED.includes(el.type) ? 'label' : 'text', value: el.text });
  if (el.placeholder) out.push({ kind: 'placeholder', value: el.placeholder });
  return out;
}

/**
 * Locators for a recorded element, best first.
 *
 * A field's recorded text is whatever the page had to say about it, a real <label>,
 * but just as often a title, a hint ("Requires first letter") or its own control name,
 * and getByLabel resolves only the first of those. Its id and name are what the DOM
 * guarantees, so they lead for fields and back up everything else. A button or link is
 * the other way round: its text is its accessible name, and ids there are often generated.
 */
function candidates(el = {}) {
  const byAttribute = attributeCandidates(el);
  const field = FIELD.has(el.type);
  const byTestId = el.testIdRepeats ? [] : testIdOf(el);
  // A repeated name scoped to the container where it is unique (recorded only when the name repeats).
  const byScope = el.scoped ? [{ kind: 'css', value: el.scoped }] : [];
  // A name with a live count, or one the page repeats, will not find it alone: its handles lead.
  const handlesFirst = uniqueFirst(el, field, byScope);
  const byName = handlesFirst || [...nameCandidates(el), ...byScope];
  const ordered = field || handlesFirst ? [...byAttribute, ...byName] : [...byName, ...byAttribute];
  return [...withPath([...byTestId, ...ordered], el.path, el), ...repeatedHandles(el)];
}

/**
 * A test id or link target the page repeats (every row's toggle, a name linked
 * three times): tried only after everything that names this one element.
 */
function repeatedHandles(el) {
  return [...(el.testIdRepeats ? testIdOf(el) : []), ...(el.hrefRepeats ? hrefCandidates(el) : [])];
}

/** The element's test id as a locator, when it has one. */
const testIdOf = (el) => (el.testId ? [{ kind: 'testId', value: el.testId }] : []);

/**
 * When the name will not find the element alone, what follows its handles: the
 * stable name for one with a live count, the scoped then plain name for one the
 * page repeats (a "View Order" on every row). Null when the name leads as usual.
 */
function uniqueFirst(el, field, byScope) {
  if (el.stableText) return stableCandidates(el, byScope);
  return el.repeats && !field ? [...byScope, ...nameCandidates(el)] : null;
}

/** The stable name (without live counts), then the scoped name. */
function stableCandidates(el, byScope) {
  return [{ kind: 'text', value: el.stableText }, ...byScope];
}

/** The id this render invented, kept only as the very last thing to try. */
function generatedId(el) {
  return el.domId && !stableId(el.domId) ? [{ kind: 'css', value: `[id=${JSON.stringify(el.domId)}]` }] : [];
}

/**
 * Its position, then an invented id: what replay falls back to when every name is
 * ambiguous. A position survives a re-render that renumbers ids, so it goes first
 * of the two, and both stay available, because an element with no name and no
 * target has nothing better to offer.
 */
function withPath(found, path, el = {}) {
  const tail = [
    ...(path && !found.some((c) => c.value === path) ? [{ kind: 'css', value: path }] : []),
    ...generatedId(el),
  ];
  return [...found, ...tail.filter((c) => !found.some((f) => f.value === c.value))];
}

/**
 * A role name as a pattern: the recorded text exactly, allowing the icon glyphs
 * and punctuation a page draws around it. Playwright counts CSS ::before icons
 * in the accessible name (Magento's menu reads " Reports"), so an exact name
 * would never match. The generated module carries the same rule as `named`.
 */
function roleName(text) {
  return new RegExp('^\\W*' + String(text).replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\W*$');
}

/** Playwright code that locates `candidate` under `owner`, with `value` as the code for its value. */
function locatorCode(candidate, owner = 'p', value = JSON.stringify(candidate?.value)) {
  const c = candidate;
  if (!c || !LOCATOR_KINDS.includes(c.kind)) throw new Error('Pick a supported locator');
  if (c.kind === 'css') return `${owner}.locator(${value})`;
  if (c.kind === 'role') return `${owner}.getByRole(${JSON.stringify(c.role)}, {name: named(${value})})`;
  return `${owner}.${LOCATOR_METHODS[c.kind]}(${value}${c.kind === 'testId' ? '' : ', {exact:true}'})`;
}

module.exports = { LOCATOR_KINDS, LOCATOR_METHODS, candidates, locatorCode, roleName, withoutLiveCount, stableId };
