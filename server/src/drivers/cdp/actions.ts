/**
 * The action vocabulary a CDP browser speaks, and how the Oya client's
 * spellings map onto it; both derived from the one vocabulary (drivers/vocabulary.ts).
 */
import { SPELLINGS, VOCABULARY } from '../vocabulary.ts';

/** The CDP driver's own name for a canonical action. */
const cdpNameOf = (action: string) => VOCABULARY[action].cdpName ?? action;

/** The canonical actions a CDP browser does that a caller may send. */
const cdpActions = Object.keys(VOCABULARY).filter((a) => VOCABULARY[a].cdp && !VOCABULARY[a].internal);

/**
 * Every spelling a CDP browser accepts, derived from the one vocabulary:
 * the canonical names, this driver's own hyphenated names, and the other
 * spellings callers send. A caller checking this set must not conclude that
 * press_key is unsupported when only press-key is this driver's name.
 */
export const CDP_CAPABILITIES = new Set([
  ...cdpActions,
  ...cdpActions.map(cdpNameOf),
  ...Object.keys(SPELLINGS).filter((spelling) => cdpActions.includes(SPELLINGS[spelling])),
]);

/**
 * One action vocabulary, two spellings: each name callers send, mapped to the
 * one this driver's handlers are keyed by. Scroll is left out: its direction
 * travels in the params, and normalise() puts it into the name.
 */
const ACTION_ALIASES: Record<string, string> = Object.fromEntries([
  ...Object.keys(VOCABULARY)
    .filter((a) => VOCABULARY[a].cdpName)
    .map((a) => [a, cdpNameOf(a)]),
  ...Object.entries(SPELLINGS)
    .filter(([spelling, a]) => a !== 'scroll' && spelling !== cdpNameOf(a))
    .map(([spelling, a]) => [spelling, cdpNameOf(a)]),
]);

/** Maps an action and its params onto this driver's own vocabulary. */
export function normalise(action, params: any = {}) {
  // `scroll` carries its direction in the params; this driver has it in the name.
  if (action === 'scroll') {
    const direction = String(params.direction || 'down');
    return { action: `scroll-${direction}`, params };
  }
  const mapped = Object.hasOwn(ACTION_ALIASES, action) ? ACTION_ALIASES[action] : action;
  // The Oya client names tabs `tab_id`; every Target.* call here wants `id`.
  const id = params.tab_id ?? params.id;
  return { action: mapped, params: id === undefined ? params : { ...params, id } };
}
