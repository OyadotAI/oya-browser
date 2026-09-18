/**
 * Locators: the ways a recorded element can be found again, and the Playwright
 * code that finds it.
 */

/** Every locator kind a step may use. */
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

/** CSS locators from the element's id, name and link target. */
function attributeCandidates(el) {
  const out = [];
  if (el.domId) out.push({ kind: 'css', value: `[id=${JSON.stringify(el.domId)}]` });
  if (el.name) out.push({ kind: 'css', value: `[name=${JSON.stringify(el.name)}]` });
  if (el.href) out.push({ kind: 'css', value: `a[href=${JSON.stringify(el.href)}]` });
  return out;
}

/** Locators from what the element says about itself: role, label, text, placeholder. */
function nameCandidates(el) {
  const out = [];
  if (el.role && (el.ariaLabel || el.text)) out.push({ kind: 'role', role: el.role, value: el.ariaLabel || el.text });
  if (el.ariaLabel) out.push({ kind: 'label', value: el.ariaLabel });
  if (el.text) out.push({ kind: LABELLED.includes(el.type) ? 'label' : 'text', value: el.text });
  if (el.placeholder) out.push({ kind: 'placeholder', value: el.placeholder });
  return out;
}

/**
 * Locators for a recorded element, best first.
 *
 * A field's recorded text is whatever the page had to say about it — a real <label>,
 * but just as often a title, a hint ("Requires first letter") or its own control name —
 * and getByLabel resolves only the first of those. Its id and name are what the DOM
 * guarantees, so they lead for fields and back up everything else. A button or link is
 * the other way round: its text is its accessible name, and ids there are often generated.
 */
function candidates(el = {}) {
  const byAttribute = attributeCandidates(el);
  const field = FIELD.has(el.type);
  const byTestId = el.testId ? [{ kind: 'testId', value: el.testId }] : [];
  return [...byTestId, ...(field ? byAttribute : []), ...nameCandidates(el), ...(field ? [] : byAttribute)];
}

/** Playwright code that locates `candidate` under `owner`, with `value` as the code for its value. */
function locatorCode(candidate, owner = 'p', value = JSON.stringify(candidate?.value)) {
  const c = candidate;
  if (!c || !LOCATOR_KINDS.includes(c.kind)) throw new Error('Pick a supported locator');
  if (c.kind === 'css') return `${owner}.locator(${value})`;
  if (c.kind === 'role') return `${owner}.getByRole(${JSON.stringify(c.role)}, {name: ${value}, exact:true})`;
  return `${owner}.${LOCATOR_METHODS[c.kind]}(${value}${c.kind === 'testId' ? '' : ', {exact:true}'})`;
}

module.exports = { LOCATOR_KINDS, LOCATOR_METHODS, candidates, locatorCode };
