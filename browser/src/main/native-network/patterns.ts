/** Bounded native request filters implement wildcard matching without regular-expression backtracking. */
import { NETWORK_LIMITS } from './constants.ts';
import type { RequestDetails } from './types.ts';
/** Shared native category mapping keeps filtering and emitted events consistent. */
const RESOURCES: Record<string, string> = {
  mainFrame: 'Document',
  subFrame: 'Document',
  xhr: 'XHR',
  script: 'Script',
  image: 'Image',
  stylesheet: 'Stylesheet',
  font: 'Font',
  media: 'Media',
  ping: 'Ping',
  cspReport: 'CSPViolationReport',
};
/** Only actual native resource categories are claimed; Fetch cannot be distinguished from xhr here. */
export function nativeResourceType(type: string): string {
  return Object.hasOwn(RESOURCES, type) ? RESOURCES[type] : 'Other';
}
/** Compiled literal/wildcard tokens distinguish escaped stars from match operators. */
type Token = {
  /** Token meaning, independent from its character spelling. */ kind: 'literal' | 'one' | 'many';
  /** Exact literal for nonwildcards. */ value?: string;
};
/** One native request-stage rule. */
interface Pattern {
  /** Compiled URL wildcard tokens. */
  tokens: Token[];
  /** Optional native resource category restriction. */
  resource?: string;
}
/** Compiled matching accepts native metadata only. */
export type RequestMatcher = (details: RequestDetails) => boolean;
/** Omitted rules intercept all; an explicitly empty list intercepts nothing. */
export function requestMatcher(value: unknown): RequestMatcher {
  const values = value === undefined ? [{}] : value;
  if (!Array.isArray(values) || values.length > NETWORK_LIMITS.patterns) throw Error('Invalid native request patterns');
  const patterns = values.map(compilePattern);
  return (d) => matchPatterns(patterns, d);
}
/** Every rule is validated before replacing any live interception policy. */
function compilePattern(value: unknown): Pattern {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid request pattern');
  const p = value as Record<string, unknown>;
  if (Object.keys(p).some((key) => !['urlPattern', 'resourceType', 'requestStage'].includes(key)))
    throw Error('Unsupported request pattern');
  if (p.requestStage !== undefined && p.requestStage !== 'Request')
    throw Error('Native interception supports Request stage only');
  const resource = resourceFilter(p.resourceType);
  return { tokens: tokenize(p.urlPattern === undefined ? '*' : p.urlPattern), resource };
}
/** Do not claim Fetch or worker/socket attribution when native metadata cannot provide it. */
function resourceFilter(value: unknown): string | undefined {
  if (value === undefined) return;
  if (typeof value !== 'string' || ![...Object.values(RESOURCES), 'Other'].includes(value))
    throw Error('Unsupported native resource type');
  return value;
}
/** Backslash escapes wildcard characters; a dangling escape is rejected explicitly. */
function tokenize(value: unknown): Token[] {
  const tokens: Token[] = [],
    characters = checkedCharacters(value);
  for (let i = 0; i < characters.length; i++) {
    const c = characters[i];
    if (c === '\\' && ++i === characters.length) throw Error('Dangling request pattern escape');
    tokens.push(c === '\\' ? { kind: 'literal', value: characters[i] } : token(c));
  }
  return tokens;
}
/** Literal characters are never interpreted as regex syntax. */
function token(c: string): Token {
  if (c === '*') return { kind: 'many' };
  if (c === '?') return { kind: 'one' };
  return { kind: 'literal', value: c };
}
/** A single budget bounds work across all rules; oversized input fails closed in the native callback. */
function matchPatterns(patterns: Pattern[], details: RequestDetails): boolean {
  patterns = patterns.filter((p) => matchesResource(p.resource, details.resourceType));
  if (!patterns.length) return false;
  if (details.url.length > NETWORK_LIMITS.url) throw Error('Native request URL filter limit exceeded');
  const budget = { remaining: NETWORK_LIMITS.matchSteps };
  return patterns.some((p) => matches(p.tokens, details.url, budget));
}
/** Iterative wildcard state machine: no recursive or regex backtracking on attacker-controlled URLs. */
function matches(tokens: Token[], text: string, budget: Budget): boolean {
  if (tokens.length === 1 && tokens[0].kind === 'many') return true;
  let states = closure(tokens, new Set([0]), budget);
  for (const c of text) {
    const next = advance(tokens, states, c, budget);
    if (!next.size) return false;
    states = closure(tokens, next, budget);
  }
  return states.has(tokens.length);
}
/** Shared fuel is consumed for every transition, not reset for each pattern or URL character. */
interface Budget {
  /** Remaining bounded state transitions. */ remaining: number;
}
/** State expansion admits empty matches for stars without recursion. */
function closure(tokens: Token[], states: Set<number>, budget: Budget): Set<number> {
  for (const index of states) {
    spend(budget);
    if (tokens[index]?.kind === 'many') states.add(index + 1);
  }
  return states;
}
/** Advance one URL character across literal, single-character and star states. */
function advance(tokens: Token[], states: Set<number>, c: string, budget: Budget): Set<number> {
  const next = new Set<number>();
  for (const index of states) {
    spend(budget);
    const t = tokens[index];
    if (t?.kind === 'many') next.add(index);
    else if (t?.kind === 'one' || t?.value === c) next.add(index + 1);
  }
  return next;
}
/** Exhaustion is an explicit fail-closed error, never a reason to silently let a request through. */
function spend(budget: Budget): void {
  if (--budget.remaining < 0) throw Error('Native request pattern work limit exceeded');
}

/** Validate before token allocation, preserving Unicode wildcard characters without splitting surrogate pairs. */
function checkedCharacters(value: unknown): string[] {
  if (typeof value !== 'string' || value.length > NETWORK_LIMITS.pattern) throw Error('Invalid native URL pattern');
  return [...value];
}

/** Other never becomes a catch-all for unsupported worker/socket category distinctions. */
function matchesResource(filter: string | undefined, actual: string): boolean {
  if (!filter) return true;
  if (filter === 'Other') return ['object', 'other'].includes(actual);
  return filter === nativeResourceType(actual);
}
