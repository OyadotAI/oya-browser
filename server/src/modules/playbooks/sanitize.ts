/**
 * Checking what a browser recorded, or a workflow draft, before it becomes a
 * playbook. The payload is built in a web page, so nothing here trusts a shape.
 */
import workflow from '../../../../browser/scripts/workflow.cjs';

import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_LEN, MAX_SCROLL_AMOUNT, MAX_STEPS } from './constants.ts';

/** Actions a recording may contain — the same set the agent records, minus what cannot replay. */
const RECORDABLE = new Set(['navigate', 'click', 'type', 'select_option', 'upload_file', 'press_key', 'scroll']);
/** Element handles kept from a recorded step. */
const EL_FIELDS = ['type', 'tag', 'text', 'domId', 'name', 'ariaLabel', 'testId', 'placeholder', 'href'];
/** Step fields kept from a recorded step. */
const STEP_FIELDS = ['url', 'text', 'option', 'file', 'key', 'direction'];
/** Element types a person types into. */
const FIELDY = new Set(['input', 'textarea', 'editable', 'select']);
/** Actions that need an element to replay. */
const NEEDS_EL = ['click', 'type', 'select_option'];

/** Whether two recorded elements have the same handles. */
const sameEl = (a = {}, b = {}) => EL_FIELDS.every((k) => a[k] === b[k]);
/** A string field capped in length, a number as a string, anything else dropped. */
const str = (v) => (typeof v === 'string' ? v.slice(0, MAX_LEN) : typeof v === 'number' ? String(v) : undefined);

/**
 * Steps recorded in a browser, cleaned up and checked before they become a playbook.
 * The caller is authenticated but the payload is built in a web page, so nothing
 * here trusts a shape: unknown actions, unknown fields and oversized strings are
 * dropped rather than stored and replayed later.
 */
export function sanitizeSteps(steps) {
  checkShape(steps);
  const out = [];
  for (const step of steps.map(cleanStep)) if (step) append(out, step);
  if (!out.some((s) => s.action !== 'navigate'))
    throw new HttpError(Status.BAD_REQUEST, 'Nothing to save: the recording has no actions.');
  return out;
}

/** Refuses anything that is not an array of at most MAX_STEPS. */
function checkShape(steps) {
  if (!Array.isArray(steps)) throw new HttpError(Status.BAD_REQUEST, 'steps must be an array');
  if (steps.length > MAX_STEPS) throw new HttpError(Status.BAD_REQUEST, `A recording is limited to ${MAX_STEPS} steps`);
}

/** One recorded step with only known fields, or null when it cannot replay. */
function cleanStep(raw) {
  if (!raw || !RECORDABLE.has(raw.action)) return null;
  const step: any = { action: raw.action };
  if (raw.start) step.start = true;
  copyFields(step, raw);
  if (raw.amount !== undefined) step.amount = Math.min(Math.abs(Number(raw.amount)) || 0, MAX_SCROLL_AMOUNT);
  return withEl(step, raw.el);
}

/** Copies the known step fields that hold a string or number. */
function copyFields(step, raw) {
  for (const k of STEP_FIELDS) {
    const v = str(raw[k]);
    if (v !== undefined) step[k] = v;
  }
}

/** Adds the cleaned element to the step; null when the step then cannot replay. */
function withEl(step, rawEl) {
  const el = cleanEl(rawEl);
  if (el) step.el = el;
  if (step.action === 'navigate' && !/^https?:\/\//i.test(step.url || '')) return null;
  if (NEEDS_EL.includes(step.action) && !step.el) return null;
  return step;
}

/** A recorded element with only known, non-empty handles, or null. */
function cleanEl(rawEl) {
  if (!rawEl || typeof rawEl !== 'object') return null;
  const el = {};
  for (const k of EL_FIELDS) {
    const v = str(rawEl[k]);
    if (v) el[k] = v;
  }
  return Object.keys(el).length ? el : null;
}

/** Adds a step to the cleaned list, folding away what a replay should not repeat. */
function append(out, step) {
  const prev = out[out.length - 1];
  // Two sources can see the same navigation — the browser's own address bar and the
  // command the live view sent — and going there twice is a slower way to be nowhere new.
  if (step.action === 'navigate' && prev?.action === 'navigate' && prev.url === step.url) return;
  // A click into a field, then typing in it, is one action to a person and two
  // to the DOM. Replaying the click adds a step that can only go wrong.
  if (step.action === 'type' && prev?.action === 'click' && FIELDY.has(prev.el?.type) && sameEl(prev.el, step.el))
    out.pop();
  out.push(step);
}

/** A version-2 workflow draft normalized and checked by generating its code; 400 if invalid. */
export function validateWorkflow(input) {
  try {
    const draft = workflow.normalizeDraft(input);
    workflow.generate(draft);
    return draft;
  } catch (error) {
    throw new HttpError(Status.BAD_REQUEST, error.message);
  }
}
