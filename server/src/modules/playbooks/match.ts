/**
 * Finding a recorded element on the live page by its stable handles, never by the
 * numeric id, which dies with each analysis.
 */

/** A way to find the recorded element in the page's elements, or a falsy value. */
type Matcher = (el: any, pool: any[]) => any;

/** Case- and whitespace-insensitive equality. */
const same = (a, b) => String(a).trim().toLowerCase() === String(b).trim().toLowerCase();

/** Matches on one recorded field, optionally only among elements with the same tag. */
function byField(field: string, sameTag = false): Matcher {
  return (el, pool) => el[field] && pool.find((e) => e[field] === el[field] && (!sameTag || e.tag === el.tag));
}

/** Matches on visible text among elements of the same type. */
const byText: Matcher = (el, pool) => el.text && pool.find((e) => e.type === el.type && e.text === el.text);

/** The handles tried in order, most stable first. */
const MATCHERS: Matcher[] = [
  byField('testId'),
  byField('domId'),
  byField('ariaLabel', true),
  byText,
  byField('name', true),
  byField('placeholder', true),
  byField('href'),
];

/** The live element a recorded one corresponds to, or null. `text` overrides the label for a data-driven click. */
export function matchElement(el: any = {}, elements = [], text) {
  const pool = [...elements.filter((e) => e.visible), ...elements.filter((e) => !e.visible)];
  if (text !== undefined) return pool.find((e) => e.text && same(e.text, text)) || null;
  for (const matcher of MATCHERS) {
    const found = matcher(el, pool);
    if (found) return found;
  }
  return null;
}
