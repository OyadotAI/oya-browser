/**
 * Facade for the workflow model: recorded drafts, their steps and locators,
 * the problems that block a run, and the Playwright code a draft becomes. The
 * desktop app, the validation worker and the server all use it through this file.
 *
 * Behind it, one job per file: rules (actions and names), handles, locators,
 * normalize, issues, generate, chrome-recorder (Chrome DevTools Recorder's
 * JSON, both ways) and diagnostics (redaction).
 */
export {
  ACTIONS,
  TARGETED,
  HIDDEN_TARGET_ACTIONS,
  visibleOnly,
  RESERVED,
  PLACEHOLDER,
  isVariableName,
} from './rules.ts';
export { candidates, locatorCode, roleName, LOCATOR_KINDS } from './locators.ts';
export {
  HANDLES,
  handlesOf,
  contradicts,
  missingIdentity,
  stableTarget,
  rawTargetOf,
  volatileTarget,
  stableId,
  withoutLiveCount,
  VOLATILE_PARAMS,
} from './handles.ts';
export { normalizeStep, normalizeDraft } from './normalize.ts';
export { issues, variableNames } from './issues.ts';
export { generate } from './generate.ts';
export { isChromeRecording, fromChromeRecording, toChromeRecording } from './chrome-recorder.ts';
export { redact, safeUrl } from './diagnostics.ts';
export { DRAFT } from './constants.ts';
export type { Candidate, Draft, ElementFacts, Generated, Issue, RecordedElement, Step, Variable } from './types.ts';
export type { Handle } from './handles.ts';
export type { ChromeRecording, ChromeStep } from './chrome-recorder.ts';
