/** Summarize native cookie rejections using fixed labels without exposing saved login data. */
import { COOKIE_REJECTION_CODES, COOKIE_LOCAL_CONFLICT_CODES } from './constants.ts';
/** Only native enum names on the fixed allowlist can appear in diagnostics. */
export function cookieRejectionCodes(reason: unknown): string[] {
  const message = reason instanceof Error ? reason.message : '';
  const found = [...new Set(message.match(/\bEXCLUDE_[A-Z_]+\b/g) || [])];
  return found.length ? found.map(knownCookieRejection) : ['unclassified'];
}
/** Count each reported native reason without including source errors or cookie attributes. */
export function cookieRejectionSummary(results: PromiseSettledResult<unknown>[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const result of results) {
    if (result.status !== 'rejected') continue;
    for (const code of cookieRejectionCodes(result.reason)) counts[code] = (counts[code] || 0) + 1;
  }
  return counts;
}

/** Unknown native enums never become a successful local-conflict outcome. */
function knownCookieRejection(code: string): string {
  return COOKIE_REJECTION_CODES.includes(code as never) ? code : 'unclassified';
}
/** Only explicit native overwrite protection is safe to treat as keeping a stronger local login. */
function preservedLocalCookie(result: PromiseSettledResult<unknown>): boolean {
  if (result.status !== 'rejected' || !(result.reason instanceof Error)) return false;
  if (!result.reason.message.startsWith('Failed to set cookie - ')) return false;
  return cookieRejectionCodes(result.reason).every((code) => COOKIE_LOCAL_CONFLICT_CODES.includes(code as never));
}
/** Keep native overwrite protections intact while unknown, malformed and storage failures remain fatal. */
export function checkCookieRejections(results: PromiseSettledResult<unknown>[]): void {
  const conflicts = results.filter(preservedLocalCookie);
  if (conflicts.length)
    console.warn('[oya] Cookie sync kept stronger local cookies:', JSON.stringify(cookieRejectionSummary(conflicts)));
  const failures = results.filter((result) => result.status === 'rejected' && !preservedLocalCookie(result));
  if (failures.length) throw Error('Cookie sync incomplete: ' + JSON.stringify(cookieRejectionSummary(failures)));
}
