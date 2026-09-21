/** Validation of governance policies and project settings, before anything is stored. */
import { Status } from '../../../platform/http-status.ts';
import { fault } from './model.ts';
import { MAX_HOST_LENGTH, MAX_HOST_RULES, MAX_HOST_WILDCARDS, MAX_RETENTION_DAYS } from './constants.ts';

// Shaped like the hostname rule it replaces, deliberately no TLD anchor, which would
// reject `localhost`, IP literals and punycode TLDs that are valid today, plus `*`
// as a label character so mid-label rules like `*-aiplatform.googleapis.com` parse.
const HOST_RULE = /^(\*\.)?[a-z0-9*](?:[a-z0-9*.-]*[a-z0-9*])?$/;
// Each `*` compiles to an unbounded quantifier and the engine enumerates every
// partition on a failed match: on a 30-character host, 8 stars is 0.4s, 10 is 7.7s and
// 12 saturates around 150s. The policy check runs before any DNS, on the shared
// control plane, so one CONNECT would stall every tenant. Three stars against a
// 2000-character host is 0ms. The length cap is for cache memory, not backtracking,
// the pathological rules are only ~30 characters long.
const badHostRule = (h) =>
  typeof h !== 'string' ||
  h.length > MAX_HOST_LENGTH ||
  (h.match(/\*/g) || []).length > MAX_HOST_WILDCARDS ||
  !HOST_RULE.test(h);

/** Fields a policy may set. */
const POLICY_FIELDS = ['allowedHosts', 'humanHosts', 'region', 'redactRecording'];
/** Policy fields that are lists of host rules. */
const HOST_LISTS = ['allowedHosts', 'humanHosts'];
/** Settings a project may change. */
const SETTINGS = ['recordingDays', 'auditDays', 'budgetUsd', 'maxConcurrent', 'rates', 'policy'];

/** A refused policy. */
const invalidPolicy = (message) => fault('invalid_policy', message, Status.BAD_REQUEST);
/** Whether a value is an object that is not an array. */
const isRecord = (value) => !!value && typeof value === 'object' && !Array.isArray(value);
/** Whether a host list is empty, too long or holds a malformed rule. */
const badHostList = (list) =>
  !Array.isArray(list) || !list.length || list.length > MAX_HOST_RULES || list.some(badHostRule);

/** Reject a governance policy with unknown fields or malformed host rules, region or redaction flag. */
export function validatePolicy(policy) {
  if (!isRecord(policy) || Object.keys(policy).some((k) => !POLICY_FIELDS.includes(k)))
    throw invalidPolicy('Unknown policy field');
  for (const name of HOST_LISTS)
    if (name in policy && badHostList(policy[name]))
      throw invalidPolicy(`${name} must contain up to 100 lowercase hostname, *.domain or mid-label wildcard rules`);
  validateScalars(policy);
}

/** Reject a malformed region or redaction flag. */
function validateScalars(policy) {
  if ('region' in policy && (typeof policy.region !== 'string' || !/^[a-z0-9_-]{1,40}$/.test(policy.region)))
    throw invalidPolicy('Invalid region');
  if ('redactRecording' in policy && typeof policy.redactRecording !== 'boolean')
    throw invalidPolicy('redactRecording must be boolean');
}

/** Whether a retention is outside 1–3650 days. */
const badRetention = (days) => !Number.isInteger(days) || days < 1 || days > MAX_RETENTION_DAYS;
/** Whether a limit is not a positive number (a whole one for maxConcurrent). */
const badLimit = (k, value) =>
  !Number.isFinite(value) || value <= 0 || (k === 'maxConcurrent' && !Number.isInteger(value));
/** Whether a rate card is not a map of non-negative USD per browser hour. */
const badRates = (rates) => !isRecord(rates) || Object.values(rates).some((v: any) => !Number.isFinite(v) || v < 0);

/** Reject retention, limits and rates out of range. */
function validateRanges(changes) {
  for (const k of ['recordingDays', 'auditDays'])
    if (k in changes && badRetention(changes[k]))
      throw fault('invalid_retention', `${k} must be 1–3650 days`, Status.BAD_REQUEST);
  for (const k of ['budgetUsd', 'maxConcurrent'])
    if (k in changes && changes[k] !== null && badLimit(k, changes[k]))
      throw fault('invalid_limit', `${k} must be positive or null`, Status.BAD_REQUEST);
  if ('rates' in changes && badRates(changes.rates))
    throw fault('invalid_rates', 'Rates must be USD per browser hour', Status.BAD_REQUEST);
}

/** Reject unknown project settings and out-of-range retention, limits and rates. */
export function validateSettings(changes) {
  if (!changes || typeof changes !== 'object' || Object.keys(changes).some((k) => !SETTINGS.includes(k)))
    throw fault('invalid_settings', 'Unknown project setting', Status.BAD_REQUEST);
  validateRanges(changes);
  if ('policy' in changes) validatePolicy(changes.policy);
}
