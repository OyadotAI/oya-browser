/**
 * Normalization: turns an untrusted draft or step (from the renderer, a file
 * or the server) into the one shape the rest of the workflow code relies on,
 * or throws saying what is wrong.
 */
import { randomUUID } from 'node:crypto';
import { DRAFT, STEP } from './constants.ts';
import { LOCATOR_KINDS, candidates } from './locators.ts';
import { RESERVED, VARIABLE_NAME, isVariableName } from './rules.ts';
import type { Candidate, Draft, ElementFacts, RecordedElement, Step, Variable } from './types.ts';

/** An untrusted object: any field may hold anything. */
type Raw = Record<string, unknown>;

/** Step fields that are free text. */
const STRING_FIELDS = [
  'action',
  'url',
  'text',
  'option',
  'file',
  'key',
  'direction',
  'expected',
  'params',
  'captureIssue',
] as const;
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
  'host',
  'testIdRepeats',
  'hrefRepeats',
];
/** Draft fields copied through untouched when present. */
const PASSTHROUGH = ['repairedFrom', 'publishedAt', 'publishedRevision', 'run'] as const;

/** A deep copy, so a normalized draft never shares state with its input. */
const clone = <T>(value: T): T => structuredClone(value);

/** Whether `value` is a plain object (not null, not an array). */
const isObject = (value: unknown): value is Raw => !!value && typeof value === 'object' && !Array.isArray(value);

/** Copies the step's text fields, refusing any that is not a string or is too long. */
function copyStrings(raw: Raw, step: Partial<Step>): void {
  for (const key of STRING_FIELDS) {
    const value = raw[key];
    if (value === undefined) continue;
    if (typeof value !== 'string' || value.length > DRAFT.MAX_VALUE) throw new Error('Invalid step ' + key);
    step[key] = value;
  }
}

/** Whether a locator candidate is well formed: a known kind, a bounded value, a plain role. */
function validCandidate(c: unknown): c is Candidate {
  if (!isObject(c) || typeof c.kind !== 'string' || !LOCATOR_KINDS.includes(c.kind)) return false;
  if (typeof c.value !== 'string' || c.value.length > DRAFT.MAX_CANDIDATE_VALUE) return false;
  return c.kind !== 'role' || (typeof c.role === 'string' && /^[a-z]{1,40}$/.test(c.role));
}

/** One candidate reduced to its known fields. */
function normalizeCandidate(c: unknown): Candidate {
  if (!validCandidate(c)) throw new Error('Invalid locator candidate');
  return { kind: c.kind, value: c.value, ...(c.kind === 'role' ? { role: c.role } : {}) };
}

/**
 * The step's locators. Ones given (a saved or edited draft) must all be well
 * formed. Ones derived from a recorded element skip what a page made unusable,
 * such as a multi-word role or a huge href: losing one locator is better than
 * losing the step.
 */
function stepCandidates(raw: Raw): Candidate[] {
  if (Array.isArray(raw.candidates)) return raw.candidates.slice(0, DRAFT.MAX_CANDIDATES).map(normalizeCandidate);
  return candidates(raw.el as ElementFacts | undefined)
    .filter(validCandidate)
    .slice(0, DRAFT.MAX_CANDIDATES)
    .map(normalizeCandidate);
}

/** The recorded element's text fields, each capped. */
function stepElement(el: Raw): RecordedElement {
  const out: RecordedElement = {};
  for (const key of ELEMENT_FIELDS) {
    const value = el[key];
    if (typeof value === 'string') out[key] = value.slice(0, DRAFT.MAX_CANDIDATE_VALUE);
  }
  return out;
}

/** The frame selectors from the page down to the step's frame. */
function stepFrames(frames: unknown): string[] {
  const bad = (f: unknown) => typeof f !== 'string' || f.length > DRAFT.MAX_CANDIDATE_VALUE;
  if (frames && (!Array.isArray(frames) || frames.some(bad) || frames.length > DRAFT.MAX_FRAMES)) {
    throw new Error('Invalid frame path');
  }
  return (frames as string[] | undefined) || [];
}

/** The step's timeout, clamped to the allowed range. */
function stepTimeout(value: unknown): number {
  const timeout = Math.max(STEP.MIN_TIMEOUT, Math.min(Number(value) || STEP.DEFAULT_TIMEOUT, STEP.MAX_TIMEOUT));
  return Number.isFinite(timeout) ? timeout : STEP.DEFAULT_TIMEOUT;
}

/** Identity and switches: a safe id (or a fresh one), enabled, breakpoint. */
function stepIdentity(raw: Raw, step: Partial<Step>): void {
  step.id = typeof raw.id === 'string' && /^[\w:.-]{1,150}$/.test(raw.id) ? raw.id : randomUUID();
  step.enabled = raw.enabled !== false;
  step.breakpoint = raw.breakpoint === true;
}

/** Scroll distance and recording marks, kept only when present. */
function stepExtras(raw: Raw, step: Partial<Step>): void {
  if (raw.amount !== undefined)
    step.amount = Math.min(STEP.MAX_SCROLL, Math.abs(Number(raw.amount)) || STEP.DEFAULT_SCROLL);
  if (raw.t) step.t = Number(raw.t);
  if (raw.start) step.start = true;
}

/** Where the step acts: its locators, recorded element, tab, frames and timeout. */
function stepTarget(raw: Raw, step: Partial<Step>): void {
  step.candidates = stepCandidates(raw);
  if (raw.el && typeof raw.el === 'object') step.el = stepElement(raw.el as Raw);
  step.tab = typeof raw.tab === 'string' ? raw.tab.slice(0, DRAFT.MAX_TAB_NAME) : 'main';
  step.frames = stepFrames(raw.frames);
  step.timeout = stepTimeout(raw.timeout);
}

/** One step in its normalized shape; throws on anything malformed. */
export function normalizeStep(raw: unknown): Step {
  if (!isObject(raw)) throw new Error('Invalid step');
  const step: Partial<Step> = {};
  copyStrings(raw, step);
  stepIdentity(raw, step);
  stepTarget(raw, step);
  stepExtras(raw, step);
  return step as Step;
}

/** Refuses a draft recorded under another schema. */
function checkVersion(raw: Raw): void {
  if (raw.schemaVersion && raw.schemaVersion !== DRAFT.SCHEMA_VERSION) {
    throw new Error(`Unsupported recording version ${raw.schemaVersion}`);
  }
}

/** Refuses a draft from another schema, or with too many steps or variables. */
function checkShape(raw: Raw): void {
  checkVersion(raw);
  const steps = raw.steps || [];
  if (!Array.isArray(steps) || steps.length > DRAFT.MAX_STEPS) throw new Error('A draft supports at most 500 steps');
  const vars = raw.variables;
  if (vars && (typeof vars !== 'object' || Array.isArray(vars) || Object.keys(vars).length > DRAFT.MAX_VARIABLES)) {
    throw new Error('Invalid variables');
  }
}

/** A non-secret variable's default, checked; undefined when it has none. */
function variableDefault(config: Raw): string {
  if (typeof config.default !== 'string' || config.default.length > DRAFT.MAX_VALUE) {
    throw new Error('Invalid variable default');
  }
  return config.default;
}

/** One variable's settings; a secret never keeps a default. */
function normalizeVariable(name: string, config: unknown): Variable {
  if (!VARIABLE_NAME.test(name) || RESERVED.includes(name) || !config || typeof config !== 'object') {
    throw new Error('Invalid variable name');
  }
  const settings = config as Raw;
  const variable: Variable = { secret: settings.secret === true };
  if (!settings.secret && settings.default !== undefined) variable.default = variableDefault(settings);
  return variable;
}

/** Every variable, normalized. */
function normalizeVariables(raw: unknown): Record<string, Variable> {
  const variables: Record<string, Variable> = {};
  for (const [name, config] of Object.entries(raw || {})) variables[name] = normalizeVariable(name, config);
  return variables;
}

/** The secret names: the listed ones plus every variable marked secret. */
function secretNames(raw: unknown, variables: Record<string, Variable>): string[] {
  const listed = raw || [];
  if (!Array.isArray(listed) || listed.some((name) => !isVariableName(name)))
    throw new Error('Invalid secret variable');
  return [...new Set([...(listed as string[]), ...Object.keys(variables).filter((k) => variables[k].secret)])];
}

/** The draft's identity and description, with defaults for anything missing. */
function draftHeader(raw: Raw): Pick<Draft, 'schemaVersion' | 'id' | 'revision' | 'name' | 'description'> {
  return {
    schemaVersion: DRAFT.SCHEMA_VERSION,
    id: (raw.id as string) || randomUUID(),
    revision: Number(raw.revision) || 0,
    name: String(raw.name || 'Untitled workflow').slice(0, DRAFT.MAX_NAME),
    description: String(raw.description || '').slice(0, DRAFT.MAX_DESCRIPTION),
  };
}

/** The draft's own fields, with defaults for anything missing. */
function draftFields(raw: Raw, variables: Record<string, Variable>, secrets: string[]): Draft {
  const header = draftHeader(raw);
  const steps = ((raw.steps as unknown[]) || []).map(normalizeStep);
  const times = {
    createdAt: (raw.createdAt as number) || Date.now(),
    updatedAt: (raw.updatedAt as number) || Date.now(),
  };
  return { ...header, steps, variables, secrets, ...times, phase: raw.phase === 'recording' ? 'recording' : 'paused' };
}

/** Refuses two steps with one id: the editor and replay address steps by id. */
function checkUniqueIds(steps: Step[]): void {
  const ids = new Set<string>();
  for (const step of steps) {
    if (ids.has(step.id)) throw new Error('Step IDs must be unique');
    ids.add(step.id);
  }
}

/** A draft in its normalized shape (a fresh empty one by default); throws on anything malformed. */
export function normalizeDraft(input: unknown = {}): Draft {
  const raw = input as Raw;
  checkShape(raw);
  const variables = normalizeVariables(raw.variables);
  const draft = draftFields(raw, variables, secretNames(raw.secrets, variables));
  for (const key of PASSTHROUGH) if (raw[key] !== undefined) draft[key] = clone(raw[key]);
  for (const name of draft.secrets) draft.variables[name] = { secret: true };
  checkUniqueIds(draft.steps);
  return draft;
}
