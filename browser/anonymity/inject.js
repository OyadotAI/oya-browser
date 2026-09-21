/**
 * The page injection, composed as ONE script in ONE scope.
 *
 * Order and scope both matter. The toString mask has to be installed before
 * anything it protects, and every patch has to be able to reach it, when the
 * fingerprint and stealth layers were separate IIFEs concatenated together,
 * the fingerprint patches ran first and were never masked at all.
 *
 * Nothing here declares a global: the whole thing is a single IIFE, so a page
 * cannot see the helpers, the profile, or that any of it ran.
 */

const { buildMaskPreamble, buildStealthBody, buildPasskeyBody, buildPermissionsBody } = require('./stealth');
const { buildFingerprintBody, buildWorkerBody } = require('./fingerprint');

/**
 * @param {object|null} profile - anonymity profile, or null for stealth only
 * @param {object} [options]
 * @param {boolean} [options.noPasskeyDialog] - the runtime cannot show a passkey dialog (stealth.js)
 * @param {boolean} [options.noPermissionPrompt] - the runtime refuses what Chrome would prompt for (stealth.js)
 * @returns {string} source for Page.addScriptToEvaluateOnNewDocument
 */
function buildInjectionScript(profile, { noPasskeyDialog = false, noPermissionPrompt = false } = {}) {
  const parts = [buildMaskPreamble()];
  if (profile) parts.push(buildFingerprintBody(profile));
  parts.push(buildStealthBody());
  if (noPasskeyDialog) parts.push(buildPasskeyBody());
  if (noPermissionPrompt) parts.push(buildPermissionsBody());
  return `(function() {\n'use strict';\n${parts.join('\n')}\n})();`;
}

/**
 * @param {object} profile - anonymity profile
 * @param {object} [options] - `userAgent` for the worker's navigator, and the injection options that apply to workers
 * @returns {string} source to evaluate in a paused worker before its script runs
 */
function buildWorkerScript(profile, options = {}) {
  const parts = [buildMaskPreamble(), buildWorkerBody(profile, options)];
  // A worker answers navigator.permissions too, and detectors compare it with its page.
  if (options.noPermissionPrompt) parts.push(buildPermissionsBody());
  return `(function() {\n'use strict';\n${parts.join('\n')}\n})();`;
}

module.exports = { buildInjectionScript, buildWorkerScript };
