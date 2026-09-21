/**
 * Locators: the ways a recorded element can be found again, and the Playwright
 * code that finds it.
 */

/** Every locator kind a step may use. */
const { stableId, withoutLiveCount } = require('./handles.cjs');

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
  // The same page with another query (a filter, a tracking tag) is still the link.
  return base && base !== href ? [exact, { kind: 'css', value: `a[href^=${JSON.stringify(base)}]` }] : [exact];
}

/** CSS locators from the element's group and value, id, name and link target. */
function attributeCandidates(el) {
  const out = [];
  // A checkbox or radio by its group and value: stable where the page makes up its id.
  if (el.name && el.choice)
    out.push({ kind: 'css', value: `input[name=${JSON.stringify(el.name)}][value=${JSON.stringify(el.choice)}]` });
  if (stableId(el.domId)) out.push({ kind: 'css', value: `[id=${JSON.stringify(el.domId)}]` });
  if (el.name) out.push({ kind: 'css', value: `[name=${JSON.stringify(el.name)}]` });
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
  const byTestId = el.testId ? [{ kind: 'testId', value: el.testId }] : [];
  // A repeated name scoped to the container where it is unique (recorded only when the name repeats).
  const byScope = el.scoped ? [{ kind: 'css', value: el.scoped }] : [];
  // A name with a live count, or one the page repeats, will not find it alone: its handles lead.
  const handlesFirst = uniqueFirst(el, field, byScope);
  const byName = handlesFirst || [...nameCandidates(el), ...byScope];
  const ordered = field || handlesFirst ? [...byAttribute, ...byName] : [...byName, ...byAttribute];
  return withPath([...byTestId, ...ordered], el.path, el);
}

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
