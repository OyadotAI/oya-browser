/**
 * Normalization: turns an untrusted draft or step (from the renderer, a file
 * or the server) into the one shape the rest of the workflow code relies on,
 * or throws saying what is wrong.
 */
const { randomUUID } = require('node:crypto');
const { DRAFT, STEP } = require('../constants.cjs');
const { LOCATOR_KINDS, candidates } = require('./locators.cjs');
const { RESERVED, VARIABLE_NAME, isVariableName } = require('./rules.cjs');

/** Step fields that are free text. */
const STRING_FIELDS = ['action', 'url', 'text', 'option', 'file', 'key', 'direction', 'expected', 'captureIssue'];
/** Recorded element fields kept for later repair. */
const ELEMENT_FIELDS = [
  'type',
  'tag',
  'text',
  'domId',
  'name',
  'placeholder',
  'ariaLabel',
  'testId',
  'role',
  'href',
  'rawHref',
  'choice',
  'stableText',
  'repeats',
  'scoped',
  'path',
];
/** Draft fields copied through untouched when present. */
const PASSTHROUGH = ['repairedFrom', 'publishedAt', 'run'];

/** A deep copy, so a normalized draft never shares state with its input. */
const clone = (value) => structuredClone(value);

/** Copies the step's text fields, refusing any that is not a string or is too long. */
function copyStrings(raw, step) {
  for (const key of STRING_FIELDS) {
    if (raw[key] === undefined) continue;
    if (typeof raw[key] !== 'string' || raw[key].length > DRAFT.MAX_VALUE) throw new Error('Invalid step ' + key);
    step[key] = raw[key];
  }
}

/** Whether a locator candidate is well formed: a known kind, a bounded value, a plain role. */
function validCandidate(c) {
  if (!c || !LOCATOR_KINDS.includes(c.kind)) return false;
  if (typeof c.value !== 'string' || c.value.length > DRAFT.MAX_CANDIDATE_VALUE) return false;
  return c.kind !== 'role' || (typeof c.role === 'string' && /^[a-z]{1,40}$/.test(c.role));
}

/** One candidate reduced to its known fields. */
function normalizeCandidate(c) {
  if (!validCandidate(c)) throw new Error('Invalid locator candidate');
  return { kind: c.kind, value: c.value, ...(c.kind === 'role' ? { role: c.role } : {}) };
}

/**
 * The step's locators. Ones given (a saved or edited draft) must all be well
 * formed. Ones derived from a recorded element skip what a page made unusable,
 * such as a multi-word role or a huge href: losing one locator is better than
 * losing the step.
 */
function stepCandidates(raw) {
  if (Array.isArray(raw.candidates)) return raw.candidates.slice(0, DRAFT.MAX_CANDIDATES).map(normalizeCandidate);
  return candidates(raw.el).filter(validCandidate).slice(0, DRAFT.MAX_CANDIDATES).map(normalizeCandidate);
}

/** The recorded element's text fields, each capped. */
function stepElement(el) {
  const out = {};
  for (const key of ELEMENT_FIELDS)
    if (typeof el[key] === 'string') out[key] = el[key].slice(0, DRAFT.MAX_CANDIDATE_VALUE);
  return out;
}

/** The frame selectors from the page down to the step's frame. */
function stepFrames(frames) {
  const bad = (f) => typeof f !== 'string' || f.length > DRAFT.MAX_CANDIDATE_VALUE;
  if (frames && (!Array.isArray(frames) || frames.some(bad) || frames.length > DRAFT.MAX_FRAMES)) {
    throw new Error('Invalid frame path');
  }
  return frames || [];
}

/** The step's timeout, clamped to the allowed range. */
function stepTimeout(value) {
  const timeout = Math.max(STEP.MIN_TIMEOUT, Math.min(Number(value) || STEP.DEFAULT_TIMEOUT, STEP.MAX_TIMEOUT));
  return Number.isFinite(timeout) ? timeout : STEP.DEFAULT_TIMEOUT;
}

/** Identity and switches: a safe id (or a fresh one), enabled, breakpoint. */
function stepIdentity(raw, step) {
  step.id = typeof raw.id === 'string' && /^[\w:.-]{1,150}$/.test(raw.id) ? raw.id : randomUUID();
  step.enabled = raw.enabled !== false;
  step.breakpoint = raw.breakpoint === true;
}

/** Scroll distance and recording marks, kept only when present. */
function stepExtras(raw, step) {
  if (raw.amount !== undefined)
    step.amount = Math.min(STEP.MAX_SCROLL, Math.abs(Number(raw.amount)) || STEP.DEFAULT_SCROLL);
  if (raw.t) step.t = Number(raw.t);
  if (raw.start) step.start = true;
}

/** Where the step acts: its locators, recorded element, tab, frames and timeout. */
function stepTarget(raw, step) {
  step.candidates = stepCandidates(raw);
  if (raw.el && typeof raw.el === 'object') step.el = stepElement(raw.el);
  step.tab = typeof raw.tab === 'string' ? raw.tab.slice(0, DRAFT.MAX_TAB_NAME) : 'main';
  step.frames = stepFrames(raw.frames);
  step.timeout = stepTimeout(raw.timeout);
}

/** One step in its normalized shape; throws on anything malformed. */
function normalizeStep(raw) {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Invalid step');
  const step = {};
  copyStrings(raw, step);
  stepIdentity(raw, step);
  stepTarget(raw, step);
  stepExtras(raw, step);
  return step;
}

/** Refuses a draft recorded under another schema. */
function checkVersion(raw) {
  if (raw.schemaVersion && raw.schemaVersion !== DRAFT.SCHEMA_VERSION) {
    throw new Error(`Unsupported recording version ${raw.schemaVersion}`);
  }
}

/** Refuses a draft from another schema, or with too many steps or variables. */
function checkShape(raw) {
  checkVersion(raw);
  const steps = raw.steps || [];
  if (!Array.isArray(steps) || steps.length > DRAFT.MAX_STEPS) throw new Error('A draft supports at most 500 steps');
  const vars = raw.variables;
  if (vars && (typeof vars !== 'object' || Array.isArray(vars) || Object.keys(vars).length > DRAFT.MAX_VARIABLES)) {
    throw new Error('Invalid variables');
  }
}

/** A non-secret variable's default, checked; undefined when it has none. */
function variableDefault(config) {
  if (typeof config.default !== 'string' || config.default.length > DRAFT.MAX_VALUE) {
    throw new Error('Invalid variable default');
  }
  return config.default;
}

/** One variable's settings; a secret never keeps a default. */
function normalizeVariable(name, config) {
  if (!VARIABLE_NAME.test(name) || RESERVED.includes(name) || !config || typeof config !== 'object') {
    throw new Error('Invalid variable name');
  }
  const variable = { secret: config.secret === true };
  if (!config.secret && config.default !== undefined) variable.default = variableDefault(config);
  return variable;
}

/** Every variable, normalized. */
function normalizeVariables(raw) {
  const variables = {};
  for (const [name, config] of Object.entries(raw || {})) variables[name] = normalizeVariable(name, config);
  return variables;
}

/** The secret names: the listed ones plus every variable marked secret. */
function secretNames(raw, variables) {
  const listed = raw || [];
  if (!Array.isArray(listed) || listed.some((name) => !isVariableName(name)))
    throw new Error('Invalid secret variable');
  return [...new Set([...listed, ...Object.keys(variables).filter((k) => variables[k].secret)])];
}

/** The draft's identity and description, with defaults for anything missing. */
function draftHeader(raw) {
  return {
    schemaVersion: DRAFT.SCHEMA_VERSION,
    id: raw.id || randomUUID(),
    revision: Number(raw.revision) || 0,
    name: String(raw.name || 'Untitled workflow').slice(0, DRAFT.MAX_NAME),
    description: String(raw.description || '').slice(0, DRAFT.MAX_DESCRIPTION),
  };
}

/** The draft's own fields, with defaults for anything missing. */
function draftFields(raw, variables, secrets) {
  const header = draftHeader(raw);
  const steps = (raw.steps || []).map(normalizeStep);
  const times = { createdAt: raw.createdAt || Date.now(), updatedAt: raw.updatedAt || Date.now() };
  return { ...header, steps, variables, secrets, ...times, phase: raw.phase === 'recording' ? 'recording' : 'paused' };
}

/** Refuses two steps with one id: the editor and replay address steps by id. */
function checkUniqueIds(steps) {
  const ids = new Set();
  for (const step of steps) {
    if (ids.has(step.id)) throw new Error('Step IDs must be unique');
    ids.add(step.id);
  }
}

/** A draft in its normalized shape (a fresh empty one by default); throws on anything malformed. */
function normalizeDraft(raw = {}) {
  checkShape(raw);
  const variables = normalizeVariables(raw.variables);
  const draft = draftFields(raw, variables, secretNames(raw.secrets, variables));
  for (const key of PASSTHROUGH) if (raw[key] !== undefined) draft[key] = clone(raw[key]);
  for (const name of draft.secrets) draft.variables[name] = { secret: true };
  checkUniqueIds(draft.steps);
  return draft;
}

module.exports = { normalizeStep, normalizeDraft };
