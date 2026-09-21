/**
 * Finding a recorded element on the live page by its stable handles, never by the
 * numeric id, which dies with each analysis.
 *
 * Which handles to trust, and in what order, is not decided here: it comes from
 * the one place that knows (browser/scripts/workflow/handles.cjs), so the live
 * replay, the version-1 export and the version-2 locator builder cannot drift
 * apart. They did, and a recording ended up clicking a different button than the
 * one it recorded.
 */

import workflow from '../../../../browser/scripts/workflow.cjs';

const { handlesOf, contradicts, missingIdentity } = workflow as any;

/** Case- and whitespace-insensitive equality. */
const same = (a, b) => String(a).trim().toLowerCase() === String(b).trim().toLowerCase();

/** Whether a live element is the same kind of thing as the recorded one. */
function inScope(scope: string | undefined, el, candidate) {
  if (scope === 'tag') return candidate.tag === el.tag;
  if (scope === 'type') return candidate.type === el.type;
  return true;
}

/**
 * Every live element carrying the same value for this handle, and not contradicting
 * any other handle that was recorded. Without that second condition a position or
 * a weak name lands on a sibling: on a message composer that meant clicking the
 * emoji picker and the expand-to-full-screen button instead of Send.
 */
function byHandle(handle, el, pool) {
  const want = handle.of(el);
  if (want === undefined || want === null || want === '') return [];
  const fits = (e) => inScope(handle.scope, el, e) && same(handle.of(e) ?? '', want) && !contradicts(el, e);
  // A match made on position alone must agree on everything else that was
  // recorded; otherwise the slot has been taken by a different control.
  const enough = (e) => !handle.positional || !missingIdentity(el, e);
  return pool.filter((e) => fits(e) && enough(e));
}

/**
 * The live element a recorded one corresponds to, or null. `text` overrides the
 * label for a data-driven click.
 *
 * A handle that fits several elements has not said which one: three rows with a
 * Delete button each all answer to "Delete", and taking the first deleted the wrong
 * row. So the handles are read for a value that fits exactly one element, and only
 * if none does is an ambiguous handle's first fit used, which is the best that can
 * be said for a page whose controls are genuinely indistinguishable.
 */
export function matchElement(el: any = {}, elements = [], text?) {
  const pool = [...elements.filter((e) => e.visible), ...elements.filter((e) => !e.visible)];
  if (text !== undefined) return pool.find((e) => e.text && same(e.text, text)) || null;
  return bestFit(el, pool);
}

/** The first handle that fits exactly one element; failing that, the best ambiguous handle's first fit. */
function bestFit(el, pool) {
  let ambiguous: any = null;
  // Only the handles this element can be found by: one it does not carry, or one
  // that cannot tell it from its siblings, is not a handle to try.
  for (const handle of handlesOf(el)) {
    const hits = byHandle(handle, el, pool);
    if (hits.length === 1) return hits[0];
    if (hits.length > 1) ambiguous = ambiguous || hits[0];
  }
  return ambiguous;
}
