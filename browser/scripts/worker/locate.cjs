/**
 * Finds a step's target in the page: turns a locator candidate into a
 * Playwright locator, with `{{variables}}` in its value filled in.
 */
const { PLACEHOLDER } = require('../workflow/rules.cjs');

/** The Playwright call behind each locator kind. */
const LOCATE = {
  css: (page, c) => page.locator(c.value),
  role: (page, c) => page.getByRole(c.role, { name: c.value, exact: true }),
  testId: (page, c) => page.getByTestId(c.value, { exact: true }),
  label: (page, c) => page.getByLabel(c.value, { exact: true }),
  text: (page, c) => page.getByText(c.value, { exact: true }),
  placeholder: (page, c) => page.getByPlaceholder(c.value, { exact: true }),
};

/** A Playwright locator for `candidate` under `page` (a page or a frame). */
function locate(page, candidate) {
  if (!Object.hasOwn(LOCATE, candidate.kind)) throw new TypeError(`Unknown locator kind: ${candidate.kind}`);
  return LOCATE[candidate.kind](page, candidate);
}

/** The candidate with its placeholders filled from the run's values, then the draft's defaults. */
function resolveCandidate(candidate, vars, draft) {
  const fill = (_, key) => String(vars?.[key] ?? draft.variables[key]?.default ?? '');
  return { ...candidate, value: candidate.value.replace(PLACEHOLDER, fill) };
}

module.exports = { locate, resolveCandidate };
