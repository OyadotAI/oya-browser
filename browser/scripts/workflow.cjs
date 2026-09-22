/**
 * Facade for the workflow model: recorded drafts, their steps and locators,
 * the problems that block a run, and the Playwright code a draft becomes. The
 * desktop app and the server both use it through this file.
 *
 * Behind it, one job per file in workflow/: rules (actions and names),
 * locators, normalize, issues, generate, and chrome-recorder (Chrome
 * DevTools Recorder's JSON, both ways).
 */
const { ACTIONS, TARGETED } = require('./workflow/rules.cjs');
const { candidates, locatorCode } = require('./workflow/locators.cjs');
const {
  HANDLES,
  handlesOf,
  contradicts,
  missingIdentity,
  stableTarget,
  rawTargetOf,
  volatileTarget,
  stableId,
  withoutLiveCount,
} = require('./workflow/handles.cjs');
const { normalizeStep, normalizeDraft } = require('./workflow/normalize.cjs');
const { issues, variableNames } = require('./workflow/issues.cjs');
const { generate } = require('./workflow/generate.cjs');
const { isChromeRecording, fromChromeRecording, toChromeRecording } = require('./workflow/chrome-recorder.cjs');

module.exports = {
  ACTIONS,
  TARGETED,
  candidates,
  HANDLES,
  handlesOf,
  contradicts,
  missingIdentity,
  stableTarget,
  rawTargetOf,
  volatileTarget,
  stableId,
  withoutLiveCount,
  normalizeStep,
  normalizeDraft,
  issues,
  variableNames,
  locatorCode,
  generate,
  isChromeRecording,
  fromChromeRecording,
  toChromeRecording,
};
