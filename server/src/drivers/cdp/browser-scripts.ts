/**
 * What the driver borrows from the browser package: the analyzer source and
 * the persona applier, each loaded once, plus the element-id check that keeps
 * caller values out of evaluated source.
 */
import { readFileSync } from 'fs';
import { join } from 'path';
import { createRequire } from 'module';
import { BROWSER_DIR } from '../../platform/paths.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';

const require = createRequire(import.meta.url);

let applierFactory;
/** The browser-side persona applier, loaded once; null when the browser package is absent. */
export function getApplier() {
  if (applierFactory === undefined) applierFactory = loadApplier();
  return applierFactory;
}

/** Requires the applier; null (with a warning) when the browser package is absent. */
function loadApplier() {
  try {
    return require('../../../../browser/anonymity/apply.js').createPersonaApplier;
  } catch (e) {
    console.warn(`[cdp] anonymity/apply.js not found (${e.message}), CDP browsers run unspoofed`);
    return null;
  }
}

let analyzerScript = null;
let analyzerMissing = false;
/** The analyzer source, read once; null (with one warning) when the file is missing. */
export function getAnalyzer() {
  if (analyzerScript || analyzerMissing) return analyzerScript;
  try {
    analyzerScript = readFileSync(join(BROWSER_DIR, 'scripts', 'analyzer.js'), 'utf8');
  } catch {
    analyzerMissing = true;
    console.warn('[cdp] analyzer.js not found, analyze/click-by-id unavailable for CDP browsers');
  }
  return analyzerScript;
}

/** The analyzer source for one session: its tag attribute filled in, its own recorder off. */
export function analyzerSource(analyzer, tagAttr) {
  return analyzer.replace('__OYA_ATTR__', tagAttr).replace('__OYA_RECORD__', 'false');
}

/**
 * Analyzer element ids are integers assigned by analyzePage(). Rejecting
 * anything else keeps caller-supplied values out of evaluated source, rather
 * than relying on every interpolation site escaping correctly.
 */
export function elementSelector(elementId) {
  // Only a number or a numeric string. An object with a coercing toString()
  // would slip past Number() alone.
  const id = typeof elementId === 'number' || typeof elementId === 'string' ? Number(elementId) : NaN;
  if (!Number.isInteger(id) || id < 0) {
    throw new HttpError(Status.BAD_REQUEST, 'element_id must be an analyzer element id');
  }
  return `[data-ac-id="${id}"]`;
}
