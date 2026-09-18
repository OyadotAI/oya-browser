/**
 * CAPTCHA detection and solving.
 *
 * One API regardless of who does the work. Hosted providers (Anchor,
 * Browserbase, Steel) solve natively, so for those the job is to notice and
 * report rather than solve twice. For Oya Cloud, self-hosted and plain CDP
 * there is no native solver, so an external one is used.
 *
 * The result always says which path ran. A silent failure that leaves an agent
 * stuck in a loop is worse than a clear "not solved".
 */

import { metrics } from '../../platform/metrics.ts';
import { DETECT_JS, applyTokenJS } from './captcha-page.ts';
import { isConfigured, solveExternally } from './captcha-solver.ts';

export { DETECT_JS, applyTokenJS } from './captcha-page.ts';
export { isConfigured, solveExternally } from './captcha-solver.ts';

/** Why a provider-solved challenge is left alone. */
const PROVIDER_NOTE = 'This provider solves natively; poll for the challenge to clear.';
/** What to do when nothing can solve a challenge. */
const NO_SOLVER = 'No CAPTCHA solver configured. Set OYA_CAPTCHA_API_KEY, or use a provider that solves natively.';

/**
 * Detect and, if asked, solve. `evaluate` runs a script in the page.
 * @returns {{ present, type, solved, method: 'provider'|'solver'|'none', error? }}
 */
export async function handle(evaluate, { solve = true, providerSolves = false, env = process.env } = {}) {
  const found = await evaluate(DETECT_JS);
  if (!found?.present) return { present: false, solved: false, method: 'none' };
  metrics.captchaSeen.inc({ type: found.type });
  // A provider that solves natively will clear it on its own; solving again
  // would pay twice and can race its own attempt.
  if (providerSolves) return { ...found, solved: false, method: 'provider', note: PROVIDER_NOTE };
  if (!solve) return { ...found, solved: false, method: 'none' };
  if (!isConfigured(env)) return { ...found, solved: false, method: 'none', error: NO_SOLVER };
  return solveAndApply(evaluate, found, env);
}

/** Solves externally and places the token; a failure is reported, not thrown. */
async function solveAndApply(evaluate, found, env) {
  try {
    const { token } = await solveExternally(found.type, found.sitekey, found.url, env);
    const result = await evaluate(applyTokenJS(found.type, token));
    return applied(found, !!result?.placed);
  } catch (err) {
    metrics.captchaSolved.inc({ type: found.type, outcome: 'error' });
    return { ...found, solved: false, method: 'solver', error: err.message };
  }
}

/** Counts and reports a solve, which only counts as solved when a field took the token. */
function applied(found, placed) {
  metrics.captchaSolved.inc({ type: found.type, outcome: placed ? 'ok' : 'unplaced' });
  return {
    ...found,
    solved: placed,
    method: 'solver',
    ...(placed ? {} : { error: 'Solved, but no field accepted the token' }),
  };
}
