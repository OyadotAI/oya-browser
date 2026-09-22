/**
 * What an action changed on the page, in words. The model otherwise has to diff two
 * element lists itself to learn that a click opened a dialog, moved to another
 * page, or did nothing at all, and a click that did nothing is the most common
 * way a run goes wrong without noticing.
 */

/** What is remembered of a page: its facts, and the fields showing an error. */
type Seen = {
  /** The analysis facts (url, title, modal, covered, elements). */
  facts: Record<string, any>;
  /** Labels of fields that showed an error. */
  errors: Set<string>;
};

/** browserId → what the last analysis showed. */
const seen = new Map<string, Seen>();

/** The fields showing an error, by label. */
const errorsOf = (elements: any[] = []) =>
  new Set(elements.filter((e) => e.error).map((e) => `${e.text || e.name || 'a field'}: ${e.error}`));

/** A facts string's leading count ("42 total, 30 visible" → 42). */
const countOf = (value: unknown) => Number(/\d+/.exec(String(value ?? ''))?.[0] ?? 0);

/** Where the page is: another address, or a new title on the same one. */
function place(before: Record<string, any>, after: Record<string, any>) {
  if (before.url !== after.url) return [`the page moved to ${after.url}`];
  return before.title !== after.title ? [`the title is now "${after.title}"`] : [];
}

/** What is drawn over the page: a dialog that opened or closed, something covering elements. */
function overlays(before: Record<string, any>, after: Record<string, any>) {
  const said: string[] = [];
  if (!before.modal && after.modal) said.push(`a dialog opened: ${after.modal}`);
  if (before.modal && !after.modal) said.push('the dialog closed');
  if (!before.covered && after.covered) said.push(after.covered);
  return said;
}

/** How many more or fewer elements the page has. */
function size(before: Record<string, any>, after: Record<string, any>) {
  const delta = countOf(after.elements) - countOf(before.elements);
  return delta ? [`${Math.abs(delta)} ${delta > 0 ? 'more' : 'fewer'} elements`] : [];
}

/** The ways two pages' facts differ, in words. */
const differences = (before: Record<string, any>, after: Record<string, any>) => [
  ...place(before, after),
  ...overlays(before, after),
  ...size(before, after),
];

/** New field errors, in words. */
const newErrors = (before: Set<string>, after: Set<string>) =>
  [...after].filter((e) => !before.has(e)).map((e) => `a field shows an error (${e})`);

/** Remembers what an analysis showed, answering what it was before. */
function remember(browserId: string, data: any): Seen | undefined {
  const before = seen.get(browserId);
  if (data?.facts) seen.set(browserId, { facts: data.facts, errors: errorsOf(data.elements) });
  return before;
}

/**
 * The line an action's result starts with: what changed since the last analysis,
 * or, when `expected` and nothing did, that the action may not have worked.
 */
export function changeNote(browserId: string, data: any, expected = false) {
  const before = remember(browserId, data);
  const after = seen.get(browserId);
  if (!before || !data?.facts || !after) return '';
  const said = [...differences(before.facts, after.facts), ...newErrors(before.errors, after.errors)];
  if (said.length) return `Changed: ${said.join('; ')}.`;
  return expected ? 'Nothing on the page changed. If you expected it to, the action may not have worked.' : '';
}

/** Remembers an analysis the model reads whole, so the next action is compared with it. */
export const noteAnalysis = (browserId: string, data: any) => void remember(browserId, data);

/** Forgets a browser's last page, as a run starts. */
export const forgetPage = (browserId: string) => void seen.delete(browserId);

/** The url of the page the last analysis showed, or undefined before one. */
export const pageUrl = (browserId: string): string | undefined => seen.get(browserId)?.facts.url;
