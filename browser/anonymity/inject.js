/**
 * The page injection, composed as ONE script in ONE scope.
 *
 * Order and scope both matter. The toString mask has to be installed before
 * anything it protects, and every patch has to be able to reach it — when the
 * fingerprint and stealth layers were separate IIFEs concatenated together,
 * the fingerprint patches ran first and were never masked at all.
 *
 * Nothing here declares a global: the whole thing is a single IIFE, so a page
 * cannot see the helpers, the profile, or that any of it ran.
 */

const { buildMaskPreamble, buildStealthBody } = require('./stealth');
const { buildFingerprintBody, buildWorkerBody } = require('./fingerprint');

/**
 * @param {object|null} profile - anonymity profile, or null for stealth only
 * @returns {string} source for Page.addScriptToEvaluateOnNewDocument
 */
function buildInjectionScript(profile) {
  const parts = [buildMaskPreamble()];
  if (profile) parts.push(buildFingerprintBody(profile));
  parts.push(buildStealthBody());
  return `(function() {\n'use strict';\n${parts.join('\n')}\n})();`;
}

/**
 * @param {object} profile - anonymity profile
 * @returns {string} source to evaluate in a paused worker before its script runs
 */
function buildWorkerScript(profile, options) {
  return `(function() {\n'use strict';\n${buildMaskPreamble()}\n${buildWorkerBody(profile, options)}\n})();`;
}

module.exports = { buildInjectionScript, buildWorkerScript };
